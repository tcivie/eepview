// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The only factory of remote (`tab-*`) webviews (ADR 0001 rule 1).
//!
//! A content webview needs a running [`Gatekeeper`], so none can exist before VERIFY passes.
//! Every one gets all engine-side layers:
//! - L2: `proxy_url` = the gatekeeper ([`Gatekeeper::url`]), and on Windows the same proxy in
//!   the browser arguments ([`windows_proxy_args`]).
//! - L3a: the page policy, added by the gatekeeper to every response (`net::rules`).
//! - L3b: the engine request filter (`WKContentRuleList` on macOS, a `WebResourceRequested`
//!   filter on Windows), attached through `eepview-platform` before the first load. The
//!   webview starts on `about:blank` and loads a page only once the filter is on. Until then
//!   a load only waits ([`super::arming`]); after a failed filter the webview never loads.
//! - L4: [`nav::guard`] on every navigation and new-window request.
//! - L5: WebRTC removed in every frame (`webrtc::webrtc_off`); `WebKitGTK` also turns it off in the
//!   engine settings.
//!
//! No IPC: the capabilities name only the `toolbar` and `internal` webviews.

use eepview_platform::{FailReason, Rules};
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, Url, Webview, WebviewBuilder,
    WebviewUrl, Window,
};

use super::apply::{self, with_core};
use super::state::{lock, now_ms, shared};
use super::webrtc::webrtc_off;
use crate::core::{Core, Load};
use crate::diag::{self, Code, ErrorKind, Field, OpKind, trace};
use crate::layout::Rect;
use crate::nav;
use crate::net::gatekeeper::Gatekeeper;
use crate::net::rules;

/// The first document of every content webview, until the engine filter is on.
const BLANK: &str = "about:blank";

/// The `code` of the load-failed page when the engine filter cannot be attached.
pub const FILTER_FAILED: &str = "eepview engine-filter";

/// `WebView2` features off: the out-of-process UI and `SmartScreen` calls home.
const WINDOWS_FEATURES_OFF: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// The `WebView2` arguments. They always carry `--proxy-server`: `wry` drops `proxy_url` when
/// custom arguments are set, so arguments without it would mean direct connections.
#[must_use]
pub fn windows_proxy_args(gatekeeper_url: &str) -> String {
    let proxy = gatekeeper_url.trim_end_matches('/');
    format!(
        "{WINDOWS_FEATURES_OFF} --proxy-server={proxy} --proxy-bypass-list=<-loopback> \
         --force-webrtc-ip-handling-policy=disable_non_proxied_udp"
    )
}

/// A remote webview for one tab.
pub struct ContentWebview;

impl ContentWebview {
    /// Builds the `tab-*` webview of `load.tab` in `window`, at `rect`, loading `load.url`.
    ///
    /// # Errors
    ///
    /// Fails when the URL is not an allowed I2P URL or the engine cannot build the webview.
    pub fn create<R: Runtime>(
        gatekeeper: &Gatekeeper,
        window: &Window<R>,
        label: &str,
        load: &Load,
        rect: Rect,
    ) -> tauri::Result<Webview<R>> {
        let url = Url::parse(&load.url)
            .ok()
            .filter(nav::guard)
            .ok_or(tauri::Error::InvalidWebviewUrl("not an I2P URL"))?;
        let proxy = Url::parse(&gatekeeper.url())
            .map_err(|_| tauri::Error::InvalidWebviewUrl("bad gatekeeper URL"))?;
        let blank = Url::parse(BLANK).map_err(|_| tauri::Error::InvalidWebviewUrl(BLANK))?;
        let mut builder = WebviewBuilder::new(label, WebviewUrl::External(blank))
            .proxy_url(proxy)
            .incognito(load.private)
            .background_color(super::surface::color(window.app_handle()))
            .initialization_script_for_all_frames(webrtc_off());
        if !load.js {
            builder = builder.disable_javascript();
        }
        builder = windows_engine(builder, &gatekeeper.url(), window.app_handle());
        builder = hooks(builder, window.app_handle(), load.tab);
        let webview = window.add_child(
            builder,
            LogicalPosition::new(rect.x, rect.y),
            LogicalSize::new(rect.w, rect.h),
        )?;
        webview.set_zoom(load.zoom)?;
        arm(&webview, load.tab, url)?;
        Ok(webview)
    }
}

/// Turns on the engine filter (L3b) and the native hooks. `url` waits for the filter and
/// loads only when the filter is on. When the filter cannot be attached, the webview stays on
/// `about:blank` (fail closed).
fn arm<R: Runtime>(webview: &Webview<R>, tab: u32, url: Url) -> tauri::Result<()> {
    lock(&shared(webview.app_handle()).arming).start(webview.label(), url);
    let live = webview.clone();
    let app = webview.app_handle().clone();
    let label = webview.label().to_owned();
    webview.with_webview(move |platform| {
        super::engine::native_hooks(&platform, &app, tab, &label);
        let json = rules::content_rule_list().to_string();
        let rules = Rules {
            id: rules::CONTENT_RULES_ID,
            json: &json,
            allow: Box::new(rules::engine_allows),
        };
        eepview_platform::attach_rules(&platform, rules, load_after_rules(live, tab));
    })
}

/// The callback of the engine filter of the webview of `tab`. On `Ok` the newest URL that
/// waits loads, in a task on the main thread: no other load of the tab can run between the
/// state change and this load. On `Err` the webview never loads a page.
fn load_after_rules<R: Runtime>(
    webview: Webview<R>,
    tab: u32,
) -> Box<dyn FnOnce(Result<(), String>)> {
    Box::new(move |result| match result {
        // Linux, Windows and a cached macOS rule list call this inside `with_webview`.
        Ok(()) => apply::outside(move || {
            let main = webview.clone();
            let queued = webview.run_on_main_thread(move || load_waiting(&main));
            if let Err(e) = queued {
                diag::event(
                    Code::ThreadFailed,
                    &[
                        Field::Op(OpKind::MainThread),
                        Field::Error(ErrorKind::from(&e)),
                    ],
                );
            }
        }),
        Err(e) => filter_failed(&webview, tab, &e),
    })
}

/// The filter of `webview` is on: loads the newest URL that waited for it. Call on the main
/// thread.
fn load_waiting<R: Runtime>(webview: &Webview<R>) {
    let waiting = lock(&shared(webview.app_handle()).arming).armed(webview.label());
    let Some(url) = waiting else {
        return;
    };
    trace::load(&format!("{} navigate {url}", webview.label()));
    let sent = webview.navigate(url);
    first_load_sent(webview.label(), sent);
}

/// The filter of `webview` could not be attached: the webview stays on `about:blank` and
/// never loads a page. The tab shows the load-failed page for the URL that waited.
fn filter_failed<R: Runtime>(webview: &Webview<R>, tab: u32, error: &str) {
    trace::load(&format!("engine filter failed, page not loaded: {error}"));
    diag::event(
        Code::EngineCallFailed,
        &[
            Field::Op(OpKind::EngineFilter),
            Field::Error(ErrorKind::Platform),
        ],
    );
    let app = webview.app_handle();
    let waiting = lock(&shared(app).arming).failed(webview.label());
    let url = waiting.map(String::from);
    let reason = FailReason::Blocked.as_str();
    with_core(app, |core| {
        core.load_failed(tab, url.as_deref(), reason, FILTER_FAILED)
    });
}

/// The outcome of the first load call of the tab webview `label`.
fn first_load_sent(label: &str, sent: tauri::Result<()>) {
    match sent {
        Ok(()) => trace::load(&format!("{label} navigate returned")),
        Err(e) => {
            trace::load(&format!("{label} navigate failed: {e}"));
            diag::event(
                Code::EngineCallFailed,
                &[
                    Field::Op(OpKind::FirstLoad),
                    Field::Error(ErrorKind::from(&e)),
                ],
            );
        }
    }
}

/// Windows: the proxy in the browser arguments, and a data folder of its own, so the
/// arguments never clash with the chrome webviews in one WebView2 environment.
#[cfg(windows)]
fn windows_engine<R: Runtime>(
    builder: WebviewBuilder<R>,
    gatekeeper_url: &str,
    app: &AppHandle<R>,
) -> WebviewBuilder<R> {
    use tauri::Manager;
    let builder = builder.additional_browser_args(&windows_proxy_args(gatekeeper_url));
    match app.path().app_local_data_dir() {
        Ok(dir) => builder.data_directory(dir.join("content-webview")),
        Err(_) => builder,
    }
}

#[cfg(not(windows))]
fn windows_engine<R: Runtime>(
    builder: WebviewBuilder<R>,
    _gatekeeper_url: &str,
    _app: &AppHandle<R>,
) -> WebviewBuilder<R> {
    builder
}

/// The engine callbacks: navigation guard, new windows, downloads, loads, titles.
fn hooks<R: Runtime>(
    builder: WebviewBuilder<R>,
    app: &AppHandle<R>,
    tab: u32,
) -> WebviewBuilder<R> {
    let (nav_app, win_app, dl_app) = (app.clone(), app.clone(), app.clone());
    builder
        .on_navigation(move |url| navigation(&nav_app, tab, url))
        .on_new_window(move |url, _features| new_window(&win_app, tab, &url))
        .on_download(move |_webview, event| download(&dl_app, &event))
        .on_page_load(move |webview, payload| {
            page_load(webview.app_handle(), tab, payload.event(), payload.url());
            first_page_done(&webview, payload.event(), payload.url());
        })
        .on_document_title_changed(move |webview, title| {
            with_core(webview.app_handle(), |core| core.title_changed(tab, &title));
        })
}

/// A navigation of a tab: allowed only when L4 and the core both allow it.
fn navigation<R: Runtime>(app: &AppHandle<R>, tab: u32, url: &Url) -> bool {
    // L4 first, before any state: nothing but an I2P URL passes this line.
    let allowed = nav::guard(url);
    let (core_ok, fx) = lock(&shared(app).core).tab_navigation(tab, url);
    apply::later(app, fx);
    allowed && core_ok
}

/// A page asked for a new window: the core may open a tab, the engine never opens one.
fn new_window<R: Runtime>(app: &AppHandle<R>, tab: u32, url: &Url) -> NewWindowResponse<R> {
    super::input::new_window(app, super::input::Source::Tab(tab), url);
    NewWindowResponse::Deny
}

/// Downloads are refused, with a toast.
fn download<R: Runtime>(app: &AppHandle<R>, event: &DownloadEvent<'_>) -> bool {
    if let DownloadEvent::Requested { url, .. } = event {
        apply::later(app, Core::download_refused(url.as_str()));
    }
    false
}

/// The first real page of a tab is up: the webview goes back to the engine default
/// background, so a page with no background of its own stays readable in dark mode.
fn first_page_done<R: Runtime>(webview: &Webview<R>, event: PageLoadEvent, url: &Url) {
    if matches!(event, PageLoadEvent::Finished) && url.as_str() != BLANK {
        let _ = webview.set_background_color(None);
    }
}

fn page_load<R: Runtime>(app: &AppHandle<R>, tab: u32, event: PageLoadEvent, url: &Url) {
    let url = url.to_string();
    match event {
        PageLoadEvent::Started => with_core(app, |core| core.page_started(tab, &url)),
        PageLoadEvent::Finished => {
            let failed = failed_page(app, &url);
            with_core(app, |core| finished(core, tab, &url, failed));
        }
    }
}

/// True when the gatekeeper saw an error answer for `url` (a 5xx, or a refusal).
fn failed_page<R: Runtime>(app: &AppHandle<R>, url: &str) -> bool {
    let gate = lock(&shared(app).gate).clone();
    gate.is_some_and(|g| g.take_failure(url))
}

/// A page finished: first mark it failed when it was an error page, then close the load.
fn finished(core: &mut Core, tab: u32, url: &str, failed: bool) -> Vec<crate::core::Effect> {
    let mut fx = if failed {
        core.page_failed(tab, url)
    } else {
        Vec::new()
    };
    fx.extend(core.page_finished(tab, url, now_ms()));
    fx
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    use tauri::App;

    use super::*;
    use crate::core::{Effect, WebOp};
    use crate::net::testing::FakeRouter;
    use crate::shell::testing::{Mock, app, bare, core, label, load, open_gate, wait_for};

    /// Long enough for a load that a callback sends from another thread.
    const SETTLE: Duration = Duration::from_millis(200);

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn navigation_needs_the_guard_and_the_core() {
        let app = bare();
        let tab = core(&app).tabs().active().unwrap().id;
        assert!(!navigation(app.handle(), tab, &url("https://example.com/")));
        assert!(!navigation(app.handle(), tab, &url("file:///etc/passwd")));
        navigation(app.handle(), tab, &url("http://a.i2p/"));
    }

    #[test]
    fn new_windows_are_always_denied() {
        let app = bare();
        let response = new_window(app.handle(), 1, &url("http://a.i2p/"));
        assert!(matches!(response, NewWindowResponse::Deny));
    }

    #[test]
    fn downloads_are_refused() {
        let app = bare();
        let mut destination = PathBuf::from("/tmp/x");
        let requested = DownloadEvent::Requested {
            url: url("http://a.i2p/f.zip"),
            destination: &mut destination,
        };
        assert!(!download(app.handle(), &requested));
        let finished = DownloadEvent::Finished {
            url: url("http://a.i2p/f.zip"),
            path: None,
            success: false,
        };
        assert!(!download(app.handle(), &finished));
    }

    #[test]
    fn page_loads_reach_the_core() {
        let app = bare();
        let tab = core(&app).tabs().active().unwrap().id;
        page_load(
            app.handle(),
            tab,
            PageLoadEvent::Started,
            &url("http://a.i2p/"),
        );
        page_load(
            app.handle(),
            tab,
            PageLoadEvent::Finished,
            &url("http://a.i2p/"),
        );
    }

    #[test]
    fn the_first_load_waits_for_the_filter() {
        let app = app();
        let webview = app.get_webview("status").unwrap();
        lock(&shared(app.handle()).arming).start("status", url("http://a.i2p/"));
        assert_ne!(webview.url().unwrap().as_str(), "http://a.i2p/");
        load_after_rules(webview.clone(), 1)(Ok(()));
        assert!(wait_for(
            || webview.url().unwrap().as_str() == "http://a.i2p/"
        ));
    }

    /// The URL that the mock engine shows in `webview`, or an empty text.
    fn shown(webview: &Webview<Mock>) -> String {
        webview.url().map(|u| u.to_string()).unwrap_or_default()
    }

    /// A load of `url` in tab `tab`, as the core asks for it.
    fn web_load(tab: u32, url: &str) -> Vec<Effect> {
        vec![Effect::Web(WebOp::Load(load(tab, url)))]
    }

    /// An app with a gatekeeper and the webview of tab 1, built for `first`. The mock engine
    /// never calls the filter callback, so the filter stays off until a test calls it.
    fn tab_app(first: &str) -> (App<Mock>, FakeRouter, Webview<Mock>) {
        let app = app();
        let router = FakeRouter::start();
        open_gate(&app, &router);
        apply::apply(app.handle(), web_load(1, first));
        let webview = apply::tab_webview(app.handle(), 1).unwrap();
        (app, router, webview)
    }

    #[test]
    fn a_load_before_the_filter_is_on_does_not_navigate() {
        let (app, _router, webview) = tab_app("http://a.i2p/");
        apply::apply(app.handle(), web_load(1, "http://b.i2p/"));
        thread::sleep(SETTLE);
        assert_eq!(shown(&webview), BLANK);
    }

    #[test]
    fn the_filter_callback_loads_the_newest_url() {
        let (app, _router, webview) = tab_app("http://a.i2p/");
        apply::apply(app.handle(), web_load(1, "http://b.i2p/"));
        load_after_rules(webview.clone(), 1)(Ok(()));
        assert!(wait_for(|| shown(&webview) == "http://b.i2p/"));
        thread::sleep(SETTLE);
        assert_eq!(shown(&webview), "http://b.i2p/");
    }

    #[test]
    fn a_tab_whose_filter_failed_never_navigates() {
        let (app, _router, webview) = tab_app("http://a.i2p/");
        let first = label(&app, 1);
        load_after_rules(webview.clone(), 1)(Err("no filter".into()));
        apply::apply(app.handle(), web_load(1, "http://c.i2p/"));
        thread::sleep(SETTLE);
        assert_ne!(shown(&webview), "http://c.i2p/");
        assert_ne!(
            label(&app, 1),
            first,
            "a new webview tries the filter again"
        );
    }

    #[test]
    fn a_failed_filter_shows_the_load_failed_page() {
        let app = app();
        let router = FakeRouter::start();
        open_gate(&app, &router);
        let fx = {
            let mut c = core(&app);
            let verdict = crate::net::verify::verify(router.addr);
            c.router_changed(crate::core::router::status_of(
                &verdict,
                "127.0.0.1:4444",
                true,
            ));
            c.navigate("http://a.i2p/").1
        };
        apply::apply(app.handle(), fx);
        let tab = core(&app).tabs().active_id();
        let webview = apply::tab_webview(app.handle(), tab).unwrap();
        load_after_rules(webview.clone(), tab)(Err("no filter".into()));
        let url_of = || core(&app).tab_info(tab).map(|t| t.url).unwrap_or_default();
        assert!(wait_for(|| url_of().starts_with("eepview://load-failed")));
        let page = Url::parse(&url_of()).unwrap();
        let pairs: Vec<(String, String)> = page.query_pairs().into_owned().collect();
        assert!(pairs.contains(&("url".into(), "http://a.i2p/".into())));
        assert!(pairs.contains(&("reason".into(), "blocked".into())));
        assert_ne!(shown(&webview), "http://a.i2p/");
    }

    #[test]
    fn a_failed_page_is_closed_without_a_history_entry() {
        let app = bare();
        let tab = core(&app).tabs().active().unwrap().id;
        let mut c = core(&app);
        c.navigate("a.i2p");
        c.page_started(tab, "http://a.i2p/");
        finished(&mut c, tab, "http://a.i2p/", true);
        assert!(!c.tab_info(tab).unwrap().nav.loading);
        assert!(
            c.history_query(&crate::types::HistoryQuery::default())
                .is_empty()
        );
        finished(&mut c, tab, "http://a.i2p/", false);
        assert!(
            c.history_query(&crate::types::HistoryQuery::default())
                .is_empty()
        );
    }

    #[test]
    fn the_gatekeeper_failure_flag_reaches_the_shell() {
        let app = bare();
        assert!(!failed_page(app.handle(), "http://a.i2p/"));
        let router = crate::net::testing::FakeRouter::start();
        crate::shell::testing::open_gate(&app, &router);
        assert!(!failed_page(app.handle(), "http://a.i2p/"));
    }

    #[test]
    fn the_first_real_page_resets_the_background() {
        let app = app();
        let webview = app.get_webview("status").unwrap();
        first_page_done(&webview, PageLoadEvent::Finished, &url(BLANK));
        first_page_done(&webview, PageLoadEvent::Finished, &url("http://a.i2p/"));
        first_page_done(&webview, PageLoadEvent::Started, &url("http://a.i2p/"));
    }

    #[test]
    fn windows_args_always_carry_the_proxy() {
        let args = windows_proxy_args("http://127.0.0.1:5555/");
        assert!(args.contains("--proxy-server=http://127.0.0.1:5555 "));
        assert!(args.contains("--proxy-bypass-list=<-loopback>"));
        assert!(args.contains("--force-webrtc-ip-handling-policy=disable_non_proxied_udp"));
        assert!(args.starts_with(WINDOWS_FEATURES_OFF));
    }

    #[test]
    fn webrtc_script_removes_peer_connections() {
        assert!(webrtc_off().contains("RTCPeerConnection"));
        assert!(webrtc_off().contains("mediaDevices"));
    }
}
