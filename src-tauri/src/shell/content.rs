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
//!   webview starts on `about:blank` and loads the page only once the filter is on.
//! - L4: [`nav::guard`] on every navigation and new-window request.
//! - L5: WebRTC removed in every frame ([`webrtc_off`]); `WebKitGTK` also turns it off in the
//!   engine settings.
//!
//! No IPC: the capabilities name only the `toolbar` and `internal` webviews.

use eepview_platform::Rules;
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, PageLoadPayload};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, Url, Webview, WebviewBuilder,
    WebviewUrl, Window,
};

use super::apply::{self, with_core};
use super::state::{lock, now_ms, shared};
use crate::core::{Core, Load};
use crate::layout::Rect;
use crate::nav;
use crate::net::gatekeeper::Gatekeeper;
use crate::net::rules;

/// The first document of every content webview, until the engine filter is on.
const BLANK: &str = "about:blank";

/// Removes the WebRTC constructors in every frame, before any page script runs.
pub const WEBRTC_OFF_SCRIPT: &str = r"
for (const name of ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel',
                    'RTCSessionDescription', 'RTCIceCandidate', 'RTCRtpSender',
                    'RTCRtpReceiver', 'RTCRtpTransceiver']) {
  try { Object.defineProperty(window, name, { value: undefined, writable: false, configurable: false }); }
  catch (e) {}
}
try {
  if (navigator.mediaDevices) {
    Object.defineProperty(navigator, 'mediaDevices', { value: undefined, writable: false, configurable: false });
  }
} catch (e) {}
";

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

/// The init script that removes WebRTC in every frame.
#[must_use]
pub fn webrtc_off() -> &'static str {
    WEBRTC_OFF_SCRIPT
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

/// Turns on the engine filter (L3b) and the native hooks, then loads `url`. When the filter
/// cannot be attached, the webview stays on `about:blank` (fail closed).
fn arm<R: Runtime>(webview: &Webview<R>, tab: u32, url: Url) -> tauri::Result<()> {
    let live = webview.clone();
    let app = webview.app_handle().clone();
    webview.with_webview(move |platform| {
        super::engine::native_hooks(&platform, &app, tab);
        let json = rules::content_rule_list().to_string();
        let rules = Rules {
            json: &json,
            allow: rules::engine_allows,
        };
        eepview_platform::attach_rules(&platform, rules, load_after_rules(live, url));
    })
}

/// The load that waits for the engine filter.
fn load_after_rules<R: Runtime>(
    webview: Webview<R>,
    url: Url,
) -> Box<dyn FnOnce(Result<(), String>)> {
    Box::new(move |result| match result {
        Ok(()) => {
            if let Err(e) = webview.navigate(url) {
                super::log::error("first load", &e.to_string());
            }
        }
        Err(e) => super::log::error("engine filter, page not loaded", &e),
    })
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
        .on_navigation(move |url| {
            // L4 first, before any state: nothing but an I2P URL passes this line.
            let allowed = nav::guard(url);
            let (core_ok, fx) = lock(&shared(&nav_app).core).tab_navigation(tab, url);
            apply::later(&nav_app, fx);
            allowed && core_ok
        })
        .on_new_window(move |url, _features| {
            with_core(&win_app, |core| core.new_window(&url));
            NewWindowResponse::Deny
        })
        .on_download(move |_webview, event| {
            if let DownloadEvent::Requested { url, .. } = event {
                apply::later(&dl_app, Core::download_refused(url.as_str()));
            }
            false
        })
        .on_page_load(move |webview, payload| page_load(&webview, &payload, tab))
        .on_document_title_changed(move |webview, title| {
            with_core(webview.app_handle(), |core| core.title_changed(tab, &title));
        })
}

fn page_load<R: Runtime>(webview: &Webview<R>, payload: &PageLoadPayload<'_>, tab: u32) {
    let url = payload.url().to_string();
    let event = payload.event();
    super::log::page(tab, matches!(event, PageLoadEvent::Started), &url);
    with_core(webview.app_handle(), |core| match event {
        PageLoadEvent::Started => core.page_started(tab, &url),
        PageLoadEvent::Finished => core.page_finished(tab, &url, now_ms()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
