// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The only factory of the `console` webview: the router's own console pages
//! (`docs/wiki/router-console.md`).
//!
//! It is not a web tab. It is built only from a [`VerifiedConsole`], in a window of its own,
//! and loads only that console origin (`http://127.0.0.1:<port>`):
//! - no `proxy_url`: the console is on loopback, and the gatekeeper never sees it;
//! - no IPC: no capability names the `console` webview;
//! - an engine rule list that allows only the console origin, attached before the first
//!   load: the view starts on `about:blank` and stays there if the list cannot be attached;
//! - a navigation guard: the console origin stays, an I2P link opens in a normal tab
//!   through the tab guard, anything else is cancelled;
//! - WebRTC off in every frame, downloads refused, incognito.
//!
//! The `tab-*` webviews keep every layer of ADR 0001 and still cannot reach loopback.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use eepview_platform::Rules;
use tauri::webview::NewWindowResponse;
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, Emitter, EventTarget, LogicalPosition, LogicalSize, Manager, Runtime, Url, Webview,
    WebviewBuilder, WebviewUrl,
};

use super::apply::{self, LISTENERS, with_core};
use super::log;
use super::state::{lock, shared};
use super::webrtc::webrtc_off;
use crate::net::console::{
    ConsoleInfo, ConsoleNav, ConsolePage, VerifiedConsole, after_recheck, detect_here,
};

/// The label of the console webview.
pub const CONSOLE_LABEL: &str = "console";
/// The label of the window that holds it.
pub const CONSOLE_WINDOW: &str = "console-window";
/// The window title.
const TITLE: &str = "Router console";
/// The first window size.
const SIZE: (f64, f64) = (1100.0, 760.0);
/// The first document of the console view, until the rule list is on.
const BLANK: &str = "about:blank";
/// How often a known console is checked again, and how often a miss is retried.
pub const RETRY_EVERY: Duration = Duration::from_secs(10);
/// How long a miss is retried after a trigger.
pub const RETRY_FOR: Duration = Duration::from_mins(2);

/// The router console view.
pub struct ConsoleWebview;

impl ConsoleWebview {
    /// Opens the console window, or reuses the open one, and loads `page` of `console`.
    ///
    /// # Errors
    ///
    /// Fails when this router has no such page, or the window or webview cannot be built.
    pub fn open<R: Runtime>(
        app: &AppHandle<R>,
        console: &VerifiedConsole,
        page: ConsolePage,
    ) -> tauri::Result<Webview<R>> {
        let url = console
            .url(page)
            .ok_or(tauri::Error::InvalidWebviewUrl("no such console page"))?;
        if let Some(webview) = app.get_webview(CONSOLE_LABEL) {
            reload(app, &webview, console, url)?;
            focus(app);
            return Ok(webview);
        }
        let webview = build(app, console)?;
        arm(&webview, console, url)?;
        focus(app);
        Ok(webview)
    }
}

/// Loads `url` in the open view: directly once the rule list is on, else after it.
fn reload<R: Runtime>(
    app: &AppHandle<R>,
    webview: &Webview<R>,
    console: &VerifiedConsole,
    url: Url,
) -> tauri::Result<()> {
    if shared(app).console_armed.load(Ordering::SeqCst) {
        webview.navigate(url)
    } else {
        arm(webview, console, url)
    }
}

/// Builds the console window and its one webview.
fn build<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole) -> tauri::Result<Webview<R>> {
    shared(app).console_armed.store(false, Ordering::SeqCst);
    let blank = Url::parse(BLANK).map_err(|_| tauri::Error::InvalidWebviewUrl(BLANK))?;
    let window = WindowBuilder::new(app, CONSOLE_WINDOW)
        .title(TITLE)
        .inner_size(SIZE.0, SIZE.1)
        .build()?;
    let builder = WebviewBuilder::new(CONSOLE_LABEL, WebviewUrl::External(blank))
        .incognito(true)
        .auto_resize()
        .initialization_script_for_all_frames(webrtc_off());
    let builder = hooks(builder, app, console);
    window.add_child(
        builder,
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(SIZE.0, SIZE.1),
    )
}

/// Attaches the console rule list, then loads `url`. When the list cannot be attached, the
/// view stays on `about:blank` (fail closed).
fn arm<R: Runtime>(webview: &Webview<R>, console: &VerifiedConsole, url: Url) -> tauri::Result<()> {
    let live = webview.clone();
    let rules = console.clone();
    webview.with_webview(move |platform| {
        let json = rules.rule_list().to_string();
        let id = format!("eepview-console-{}", rules.port());
        let allow = rules.clone();
        let list = Rules {
            id: &id,
            json: &json,
            allow: Box::new(move |url| allow.engine_allows(url)),
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
    Box::new(move |result| match result {
        Ok(()) => {
            shared(webview.app_handle())
                .console_armed
                .store(true, Ordering::SeqCst);
            apply::outside(move || navigate(&webview, url));
        }
        Err(e) => log::error("console rule list, page not loaded", &e),
    })
}

/// The engine callbacks: navigation guard, new windows, downloads.
fn hooks<R: Runtime>(
    builder: WebviewBuilder<R>,
    app: &AppHandle<R>,
    console: &VerifiedConsole,
) -> WebviewBuilder<R> {
    let (nav_app, win_app) = (app.clone(), app.clone());
    let (nav_console, win_console) = (console.clone(), console.clone());
    builder
        .on_navigation(move |url| navigation(&nav_app, &nav_console, url))
        .on_new_window(move |url, _features| new_window(&win_app, &win_console, &url))
        .on_download(|_webview, _event| false)
}

/// A navigation of the console view: the console origin stays; an I2P site opens in a tab.
fn navigation<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole, url: &Url) -> bool {
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

/// A console page asked for a new window: never an engine window.
fn new_window<R: Runtime>(
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

/// Loads a console URL in the console view, off the engine callback.
fn load_here<R: Runtime>(app: &AppHandle<R>, url: Url) {
    if let Some(webview) = app.get_webview(CONSOLE_LABEL) {
        apply::outside(move || navigate(&webview, url));
    }
}

fn navigate<R: Runtime>(webview: &Webview<R>, url: Url) {
    if let Err(e) = webview.navigate(url) {
        log::error("console load", &e.to_string());
    }
}

/// Opens an I2P URL in a new normal tab, through the tab guard of the core.
fn open_tab<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    with_core(app, |core| core.new_window(url));
    if let Some(main) = app.get_window("main") {
        let _ = main.set_focus();
    }
}

fn focus<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_window(CONSOLE_WINDOW) {
        let _ = window.set_focus();
    }
}

/// Closes the console webview and its window, if open.
pub fn close<R: Runtime>(app: &AppHandle<R>) {
    if let Some(webview) = app.get_webview(CONSOLE_LABEL)
        && let Err(e) = webview.close()
    {
        log::error("console close", &e.to_string());
    }
    if let Some(window) = app.get_window(CONSOLE_WINDOW)
        && let Err(e) = window.destroy()
    {
        log::error("console close", &e.to_string());
    }
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
/// or moved closes the console window.
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
/// re-checked every 10 s; a miss is retried every 10 s for 2 minutes.
pub fn detect_now<R: Runtime>(app: &AppHandle<R>) -> ConsoleInfo {
    let found = detect_here();
    let info = info_of(found.as_ref());
    if found.is_some() {
        set_console(app, found);
        recheck(app);
    } else if current(app).is_none() {
        retry(app);
    }
    info
}

/// Starts a named thread unless `flag` says one runs; the thread clears it when it ends.
fn spawn_once<R: Runtime>(
    app: &AppHandle<R>,
    name: &str,
    flag: fn(&AppHandle<R>) -> &std::sync::atomic::AtomicBool,
    body: fn(&AppHandle<R>),
) {
    if flag(app).swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    let spawned = thread::Builder::new().name(name.into()).spawn(move || {
        body(&handle);
        flag(&handle).store(false, Ordering::SeqCst);
    });
    if let Err(e) = spawned {
        flag(app).store(false, Ordering::SeqCst);
        log::error(name, &e.to_string());
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

fn retry_loop<R: Runtime>(app: &AppHandle<R>) {
    let rounds = RETRY_FOR.as_secs() / RETRY_EVERY.as_secs();
    for _ in 0..rounds {
        thread::sleep(RETRY_EVERY);
        if current(app).is_some() {
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

/// Checks the known console every 10 s until it is cleared ([`after_recheck`]).
fn watch_loop<R: Runtime>(app: &AppHandle<R>) {
    let mut misses = 0;
    while let Some(known) = current(app) {
        thread::sleep(RETRY_EVERY);
        let (next, count) = after_recheck(Some(&known), misses, detect_here());
        misses = count;
        set_console(app, next);
    }
}

#[cfg(test)]
mod tests;
