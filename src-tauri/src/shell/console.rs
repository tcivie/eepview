// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The only factory of the `console` webview: the router's own console pages
//! (`docs/wiki/router-console.md`).
//!
//! It is not a web tab webview. It is built only from a [`VerifiedConsole`], as a child of
//! the main window that the console tab shows, and loads only that console origin
//! (`http://127.0.0.1:<port>`):
//! - no `proxy_url`: the console is on loopback, and the gatekeeper never sees it;
//! - no IPC: no capability names the `console` webview;
//! - an engine rule list that allows only the console origin, attached before the first
//!   load: the view starts on `about:blank` and stays there if the list cannot be attached;
//! - a navigation guard: the console origin stays, an I2P link opens in a new normal tab
//!   through the tab guard, anything else is cancelled;
//! - WebRTC off in every frame, downloads refused, incognito.
//!
//! The `tab-*` webviews keep every layer of ADR 0001 and still cannot reach loopback.

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use eepview_platform::Rules;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{
    AppHandle, Emitter, EventTarget, LogicalPosition, LogicalSize, Manager, Runtime, Url, Webview,
    WebviewBuilder, WebviewUrl,
};

use super::apply::{self, LISTENERS, with_core};
use super::state::{lock, shared};
use super::webrtc::webrtc_off;
use super::{engine, view};
use crate::core::{ConsoleOp, Core, EngineOp};
use crate::diag::{self, Code, ErrorKind, Field, OpKind};
use crate::net::console::{ConsoleInfo, ConsoleNav, VerifiedConsole, after_recheck, detect_here};

/// The label of the console webview.
pub const CONSOLE_LABEL: &str = "console";
/// The first document of the console view, until the rule list is on.
const BLANK: &str = "about:blank";
/// How often a known console is checked again, and how often a miss is retried.
pub const RETRY_EVERY: Duration = Duration::from_secs(10);
/// How long a miss is retried after a trigger.
pub const RETRY_FOR: Duration = Duration::from_mins(2);

/// The router console view.
pub struct ConsoleWebview;

impl ConsoleWebview {
    /// Builds the console webview in the main window, or reuses it, loads the home page of
    /// `console`, and opens or selects the console tab (R8, R24).
    ///
    /// # Errors
    ///
    /// Fails when there is no main window or the webview cannot be built.
    pub fn open<R: Runtime>(
        app: &AppHandle<R>,
        console: &VerifiedConsole,
    ) -> tauri::Result<Webview<R>> {
        let url = console.home();
        let live = app.get_webview(CONSOLE_LABEL);
        let webview = match live {
            Some(webview) => webview,
            None => build(app, console)?,
        };
        let fx = lock(&shared(app).core).console_open(url.as_str());
        apply::later(app, fx);
        if shared(app).console_armed.load(Ordering::SeqCst) {
            webview.navigate(url)?;
        } else {
            arm(&webview, console, url)?;
        }
        Ok(webview)
    }
}

/// Builds the console webview, hidden, in the main window at the content rect.
fn build<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole) -> tauri::Result<Webview<R>> {
    shared(app).console_armed.store(false, Ordering::SeqCst);
    let window = app.get_window("main").ok_or(tauri::Error::WindowNotFound)?;
    let blank = Url::parse(BLANK).map_err(|_| tauri::Error::InvalidWebviewUrl(BLANK))?;
    let builder = WebviewBuilder::new(CONSOLE_LABEL, WebviewUrl::External(blank))
        .incognito(true)
        .background_color(super::surface::color(app))
        .initialization_script_for_all_frames(webrtc_off());
    let builder = hooks(builder, app, console);
    let rect = view::content_rect(app);
    let webview = window.add_child(
        builder,
        LogicalPosition::new(rect.x, rect.y),
        LogicalSize::new(rect.w, rect.h),
    )?;
    let _ = webview.hide();
    view::raise_chrome(app, &window);
    Ok(webview)
}

/// Attaches the console rule list, then loads `url`. When the list cannot be attached, the
/// view stays on `about:blank` (fail closed).
fn arm<R: Runtime>(webview: &Webview<R>, console: &VerifiedConsole, url: Url) -> tauri::Result<()> {
    let live = webview.clone();
    let json = console.rule_list().to_string();
    let id = format!("eepview-console-{}", console.port());
    let allow = console.clone();
    let allow: Box<dyn Fn(&str) -> bool + Send> = Box::new(move |url| allow.engine_allows(url));
    webview.with_webview(move |platform| {
        let list = Rules {
            id: &id,
            json: &json,
            allow,
        };
        eepview_platform::attach_rules(&platform, list, load_after_rules(live, url));
    })
}

/// The first load of the console view, after the rule list is attached. On `Ok` it marks
/// the view armed and loads `url`, off the engine callback; on `Err` nothing loads.
pub fn load_after_rules<R: Runtime>(
    webview: Webview<R>,
    url: Url,
) -> Box<dyn FnOnce(Result<(), String>)> {
    Box::new(move |result| {
        if result.is_err() {
            failed(OpKind::EngineFilter, ErrorKind::Platform);
            with_core(webview.app_handle(), Core::console_failed);
            return;
        }
        shared(webview.app_handle())
            .console_armed
            .store(true, Ordering::SeqCst);
        apply::outside(move || navigate(&webview, url));
    })
}

/// The engine callbacks: navigation guard, new windows, downloads, loads, titles.
fn hooks<R: Runtime>(
    builder: WebviewBuilder<R>,
    app: &AppHandle<R>,
    console: &VerifiedConsole,
) -> WebviewBuilder<R> {
    let (nav_app, win_app) = (app.clone(), app.clone());
    let (nav_console, win_console, load_console) =
        (console.clone(), console.clone(), console.clone());
    builder
        .on_navigation(move |url| navigation(&nav_app, &nav_console, url))
        .on_new_window(move |url, _features| new_window(&win_app, &win_console, &url))
        .on_download(|_webview, _event| false)
        .on_page_load(move |webview, payload| {
            page_load(
                webview.app_handle(),
                &load_console,
                payload.event(),
                payload.url(),
            );
        })
        .on_document_title_changed(|webview, title| title_changed(webview.app_handle(), &title))
}

/// R10: a navigation of the console view. The console origin (and the first
/// `about:blank`) stays; an I2P site opens in a new normal tab instead; anything else is
/// cancelled.
pub fn navigation<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole, url: &Url) -> bool {
    if url.as_str() == BLANK {
        return true;
    }
    match console.route(url) {
        ConsoleNav::Stay => true,
        ConsoleNav::OpenTab => {
            open_tab(app, url);
            false
        }
        ConsoleNav::Cancel => false,
    }
}

/// R10: a console page asked for a new window. Never an engine window: the console origin
/// loads in the console view, an I2P site opens in a new normal tab, anything else is
/// dropped.
pub fn new_window<R: Runtime>(
    app: &AppHandle<R>,
    console: &VerifiedConsole,
    url: &Url,
) -> NewWindowResponse<R> {
    match console.route(url) {
        ConsoleNav::Stay => load_here(app, url.clone()),
        ConsoleNav::OpenTab => open_tab(app, url),
        ConsoleNav::Cancel => {}
    }
    NewWindowResponse::Deny
}

/// R25: a main-frame load of the console view. Only a URL on the console origin reaches
/// the core.
pub fn page_load<R: Runtime>(
    app: &AppHandle<R>,
    console: &VerifiedConsole,
    event: PageLoadEvent,
    url: &Url,
) {
    if console.route(url) != ConsoleNav::Stay {
        return;
    }
    let url = url.to_string();
    match event {
        PageLoadEvent::Started => with_core(app, |core| core.console_started(&url)),
        PageLoadEvent::Finished => with_core(app, |core| core.console_finished(&url)),
    }
}

/// R25: the document title of the console page changed.
pub fn title_changed<R: Runtime>(app: &AppHandle<R>, title: &str) {
    with_core(app, |core| core.console_title(title));
}

/// R27, R28: carries out a [`ConsoleOp`] on the console webview, when one exists.
pub fn run<R: Runtime>(app: &AppHandle<R>, op: &ConsoleOp) {
    let Some(engine_op) = engine_op(op) else {
        close_webview(app);
        return;
    };
    let tab = lock(&shared(app).core).console_tab().unwrap_or_default();
    if let Some(webview) = app.get_webview(CONSOLE_LABEL) {
        engine::run(&webview, tab, &engine_op);
    }
}

/// The engine call of a [`ConsoleOp`]. `Close` is not an engine call.
fn engine_op(op: &ConsoleOp) -> Option<EngineOp> {
    match op {
        ConsoleOp::Back => Some(EngineOp::Back),
        ConsoleOp::Forward => Some(EngineOp::Forward),
        ConsoleOp::Reload => Some(EngineOp::Reload),
        ConsoleOp::HardReload => Some(EngineOp::HardReload),
        ConsoleOp::Stop => Some(EngineOp::Stop),
        ConsoleOp::Close => None,
    }
}

/// Loads a console URL in the console view, off the engine callback.
fn load_here<R: Runtime>(app: &AppHandle<R>, url: Url) {
    if let Some(webview) = app.get_webview(CONSOLE_LABEL) {
        apply::outside(move || navigate(&webview, url));
    }
}

fn navigate<R: Runtime>(webview: &Webview<R>, url: Url) {
    if let Err(e) = webview.navigate(url) {
        failed(OpKind::ConsoleLoad, ErrorKind::from(&e));
    }
}

/// Opens an I2P URL in a new normal tab, through the tab guard of the core.
fn open_tab<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    with_core(app, |core| core.new_window(url));
}

/// Records a failed engine call of the console view.
fn failed(op: OpKind, error: ErrorKind) {
    diag::event(
        Code::EngineCallFailed,
        &[Field::Op(op), Field::Error(error)],
    );
}

/// Destroys the console webview, if any.
fn close_webview<R: Runtime>(app: &AppHandle<R>) {
    shared(app).console_armed.store(false, Ordering::SeqCst);
    if let Some(webview) = app.get_webview(CONSOLE_LABEL)
        && let Err(e) = webview.close()
    {
        failed(OpKind::ConsoleClose, ErrorKind::from(&e));
    }
}

/// R30: closes the console tab, if open, and destroys the console webview, if any.
pub fn close<R: Runtime>(app: &AppHandle<R>) {
    with_core(app, Core::console_gone);
    close_webview(app);
}

/// The stored detection result.
#[must_use]
pub fn current<R: Runtime>(app: &AppHandle<R>) -> Option<VerifiedConsole> {
    lock(&shared(app).console).clone()
}

/// The contract `ConsoleInfo` of a detection result.
#[must_use]
pub fn info_of(console: Option<&VerifiedConsole>) -> ConsoleInfo {
    console.map_or_else(ConsoleInfo::none, VerifiedConsole::info)
}

/// Stores a detection result. A change emits `console-changed`; a console that went away
/// or moved closes the console tab (R30).
pub fn set_console<R: Runtime>(app: &AppHandle<R>, console: Option<VerifiedConsole>) {
    let info = info_of(console.as_ref());
    let previous = std::mem::replace(&mut *lock(&shared(app).console), console);
    let before = info_of(previous.as_ref());
    if before == info {
        return;
    }
    if before.origin.is_some() && before.origin != info.origin {
        close(app);
    }
    let _ = app.emit_to(EventTarget::App, "console-changed", info.clone());
    for label in LISTENERS {
        if app.get_webview(label).is_some() {
            let _ = app.emit_to(label, "console-changed", info.clone());
        }
    }
}

/// Probes now (blocking), stores the result and answers its info. A found console is
/// re-checked every 10 s (misses count, R21); a miss here clears the stored console at once
/// and is retried every 10 s for 2 minutes.
pub fn detect_now<R: Runtime>(app: &AppHandle<R>) -> ConsoleInfo {
    let found = detect_here();
    let info = info_of(found.as_ref());
    let hit = found.is_some();
    set_console(app, found);
    if hit {
        recheck(app);
    } else {
        retry(app);
    }
    info
}

/// R22: ends every re-check and retry loop at its next tick. After it, no loop opens a
/// connection; a later [`detect_now`] starts the loops again.
pub fn stop<R: Runtime>(app: &AppHandle<R>) {
    shared(app).console_epoch.fetch_add(1, Ordering::SeqCst);
}

/// True while the loop of epoch `epoch` may still run: no [`stop`] since it started.
fn live<R: Runtime>(app: &AppHandle<R>, epoch: u64) -> bool {
    shared(app).console_epoch.load(Ordering::SeqCst) == epoch
}

/// Starts a named loop thread unless one of the current epoch runs. `slot` holds the epoch
/// of the running loop (0: none); the thread clears it when it ends.
fn spawn_once<R: Runtime>(
    app: &AppHandle<R>,
    name: &str,
    slot: fn(&AppHandle<R>) -> &AtomicU64,
    body: fn(&AppHandle<R>, u64),
) {
    let epoch = shared(app).console_epoch.load(Ordering::SeqCst);
    if slot(app).swap(epoch, Ordering::SeqCst) == epoch {
        return;
    }
    let handle = app.clone();
    let spawned = thread::Builder::new().name(name.into()).spawn(move || {
        body(&handle, epoch);
        let _ = slot(&handle).compare_exchange(epoch, 0, Ordering::SeqCst, Ordering::SeqCst);
    });
    if let Err(e) = spawned {
        slot(app).store(0, Ordering::SeqCst);
        diag::event(
            Code::ThreadFailed,
            &[Field::Op(OpKind::Spawn), Field::Error(ErrorKind::from(&e))],
        );
    }
}

/// Retries a miss every 10 s for 2 minutes, unless a retry runs.
fn retry<R: Runtime>(app: &AppHandle<R>) {
    spawn_once(
        app,
        "console-retry",
        |a| &shared(a).inner().console_retry,
        retry_loop,
    );
}

fn retry_loop<R: Runtime>(app: &AppHandle<R>, epoch: u64) {
    let rounds = RETRY_FOR.as_secs() / RETRY_EVERY.as_secs();
    for _ in 0..rounds {
        thread::sleep(RETRY_EVERY);
        if !live(app, epoch) || current(app).is_some() {
            return;
        }
        if let Some(found) = detect_here() {
            set_console(app, Some(found));
            recheck(app);
            return;
        }
    }
}

/// Re-checks the known console every 10 s, unless a re-check runs.
fn recheck<R: Runtime>(app: &AppHandle<R>) {
    spawn_once(
        app,
        "console-watch",
        |a| &shared(a).inner().console_watch,
        watch_loop,
    );
}

/// Checks the known console every 10 s until it is cleared ([`after_recheck`]) or the
/// loops stop.
fn watch_loop<R: Runtime>(app: &AppHandle<R>, epoch: u64) {
    let mut misses = 0;
    loop {
        thread::sleep(RETRY_EVERY);
        let Some(known) = current(app).filter(|_| live(app, epoch)) else {
            return;
        };
        let (next, count) = after_recheck(Some(&known), misses, detect_here());
        misses = count;
        set_console(app, next);
    }
}

#[cfg(test)]
mod tests;
