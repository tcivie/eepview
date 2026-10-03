// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The only factory of the `console` webview: the router's own console pages
//! (`docs/wiki/router-console.md`).
//!
//! It is not a web tab. It is built only from a [`VerifiedConsole`], in a window of its own,
//! and loads only that console origin (`http://127.0.0.1:<port>`):
//! - no `proxy_url`: the console is on loopback, and the gatekeeper never sees it;
//! - no IPC: no capability names the `console` webview;
//! - a navigation guard: the console origin stays, an I2P link opens in a normal tab
//!   through the tab guard, anything else is cancelled;
//! - WebRTC off in every frame, downloads refused, incognito.
//!
//! The `tab-*` webviews keep every layer of ADR 0001 and still cannot reach loopback.

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use tauri::webview::NewWindowResponse;
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, Emitter, EventTarget, LogicalPosition, LogicalSize, Manager, Runtime, Url, Webview,
    WebviewBuilder, WebviewUrl,
};

use super::apply::{self, LISTENERS, with_core};
use super::content::webrtc_off;
use super::log;
use super::state::{lock, shared};
use crate::net::console::{ConsoleInfo, ConsoleNav, ConsolePage, VerifiedConsole, detect_here};

/// The label of the console webview.
pub const CONSOLE_LABEL: &str = "console";
/// The label of the window that holds it.
pub const CONSOLE_WINDOW: &str = "console-window";
/// The window title.
const TITLE: &str = "Router console";
/// The first window size.
const SIZE: (f64, f64) = (1100.0, 760.0);
/// How often a known console is checked again.
const RECHECK: Duration = Duration::from_secs(10);

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
            webview.navigate(url)?;
            focus(app);
            return Ok(webview);
        }
        let webview = build(app, console, url)?;
        focus(app);
        Ok(webview)
    }
}

/// Builds the console window and its one webview.
fn build<R: Runtime>(
    app: &AppHandle<R>,
    console: &VerifiedConsole,
    url: Url,
) -> tauri::Result<Webview<R>> {
    let window = WindowBuilder::new(app, CONSOLE_WINDOW)
        .title(TITLE)
        .inner_size(SIZE.0, SIZE.1)
        .build()?;
    let builder = WebviewBuilder::new(CONSOLE_LABEL, WebviewUrl::External(url))
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

/// Closes the console window, if open.
pub fn close<R: Runtime>(app: &AppHandle<R>) {
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

/// Probes now (blocking), stores the result and answers its info. While a console is
/// known, a thread checks it again every 10 s.
pub fn detect_now<R: Runtime>(app: &AppHandle<R>) -> ConsoleInfo {
    let found = detect_here();
    set_console(app, found.clone());
    if found.is_some() {
        recheck(app);
    }
    info_of(found.as_ref())
}

/// Starts the re-check thread, unless one runs. It ends when no console is known.
fn recheck<R: Runtime>(app: &AppHandle<R>) {
    if shared(app).console_watch.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    let spawned = thread::Builder::new()
        .name("console-watch".into())
        .spawn(move || watch_loop(&handle));
    if let Err(e) = spawned {
        shared(app).console_watch.store(false, Ordering::SeqCst);
        log::error("console watch", &e.to_string());
    }
}

/// Checks the known console every 10 s until none is known.
fn watch_loop<R: Runtime>(app: &AppHandle<R>) {
    while current(app).is_some() {
        thread::sleep(RECHECK);
        set_console(app, detect_here());
    }
    shared(app).console_watch.store(false, Ordering::SeqCst);
    // A detection may have found a console between the last check and the store.
    if current(app).is_some() {
        recheck(app);
    }
}

#[cfg(test)]
mod tests;
