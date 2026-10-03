// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The main window and the bundled webviews: `toolbar`, `internal` and `status`. They load
//! only bundled pages (`WebviewUrl::App`); remote pages live in `content.rs`.

use tauri::webview::PageLoadEvent;
use tauri::window::WindowBuilder;
use tauri::{
    App, AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, Url, WebviewBuilder,
    WebviewUrl, Window,
};

use super::apply::{later, with_core};
use super::state::{lock, shared};
use super::view;
use crate::layout;

/// The toolbar page.
pub const TOOLBAR_PAGE: &str = "src/ui/toolbar.html";
/// The first internal page.
pub const HOME_PAGE: &str = "src/ui/home.html";
/// The status bubble page.
pub const STATUS_PAGE: &str = "src/ui/status.html";

/// Builds the window and the three bundled webviews.
///
/// # Errors
///
/// Fails when the window or a webview cannot be built.
pub fn build<R: Runtime>(app: &mut App<R>) -> tauri::Result<Window<R>> {
    let window = window_builder(app)?.build()?;
    let (w, h) = (1200.0, 800.0);
    let (bar, content) = layout::split(w, h, layout::TOOLBAR, false);
    let toolbar = WebviewBuilder::new("toolbar", WebviewUrl::App(TOOLBAR_PAGE.into()));
    let toolbar = window.add_child(
        toolbar,
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(bar.w, bar.h),
    )?;
    view::remember_base(app.handle(), &toolbar);
    let internal = internal_builder(app.handle());
    window.add_child(
        internal,
        LogicalPosition::new(content.x, content.y),
        LogicalSize::new(content.w, content.h),
    )?;
    let status = WebviewBuilder::new("status", WebviewUrl::App(STATUS_PAGE.into()));
    let status = window.add_child(
        status,
        LogicalPosition::new(0.0, h - 24.0),
        LogicalSize::new(200.0, 24.0),
    )?;
    status.hide()?;
    view::raise_chrome(app.handle(), &window);
    Ok(window)
}

/// The main window from `tauri.conf.json` (`create: false`): on macOS the title bar is an
/// overlay and the traffic lights sit in the 44 px tab row. Windows and Linux keep their
/// native title bar (the macOS keys do nothing there).
fn window_builder<R: Runtime>(app: &App<R>) -> tauri::Result<WindowBuilder<'_, R, App<R>>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .cloned()
        .unwrap_or_default();
    WindowBuilder::from_config(app, &config)
}

/// The `internal` webview: bundled pages only. A link to an I2P site opens in the active tab;
/// anything else is refused.
fn internal_builder<R: Runtime>(app: &AppHandle<R>) -> WebviewBuilder<R> {
    let nav_app = app.clone();
    WebviewBuilder::new("internal", WebviewUrl::App(HOME_PAGE.into()))
        .on_navigation(move |url| internal_navigation(&nav_app, url))
        .on_page_load(|webview, payload| {
            internal_page_load(webview.app_handle(), payload.event(), payload.url());
        })
}

/// A finished load of a bundled page tells the core which page shows.
fn internal_page_load<R: Runtime>(app: &AppHandle<R>, event: PageLoadEvent, url: &Url) {
    if matches!(event, PageLoadEvent::Finished) {
        with_core(app, |core| core.internal_loaded(url));
    }
}

fn internal_navigation<R: Runtime>(app: &AppHandle<R>, url: &Url) -> bool {
    let base = lock(&shared(app).base).clone();
    if base.is_some_and(|b| b.origin() == url.origin()) || url.scheme() == "tauri" {
        return true;
    }
    let target = url.to_string();
    let fx = lock(&shared(app).core).navigate(&target).1;
    later(app, fx);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::testing::{app, bare, core};

    fn url(text: &str) -> Url {
        Url::parse(text).unwrap()
    }

    #[test]
    fn build_adds_the_three_bundled_webviews() {
        let app = app();
        for label in ["toolbar", "internal", "status"] {
            assert!(app.get_webview(label).is_some(), "{label}");
        }
        assert!(lock(&shared(app.handle()).base).is_some());
    }

    #[test]
    fn bundled_pages_stay_in_the_internal_webview() {
        let app = bare();
        assert!(internal_navigation(
            app.handle(),
            &url("tauri://localhost/x.html")
        ));
        *lock(&shared(app.handle()).base) = Some(url("http://localhost:1420/"));
        assert!(internal_navigation(
            app.handle(),
            &url("http://localhost:1420/a.html")
        ));
        assert!(internal_navigation(
            app.handle(),
            &url("tauri://localhost/x.html")
        ));
    }

    #[test]
    fn links_to_sites_open_in_the_active_tab() {
        let app = bare();
        assert!(!internal_navigation(app.handle(), &url("http://a.i2p/")));
        assert_eq!(core(&app).tabs().active().unwrap().url, "http://a.i2p/");
        assert!(!internal_navigation(
            app.handle(),
            &url("https://example.com/")
        ));
    }

    #[test]
    fn finished_loads_tell_the_core() {
        let app = bare();
        let page = url("tauri://localhost/src/ui/history.html");
        internal_page_load(app.handle(), PageLoadEvent::Started, &page);
        internal_page_load(app.handle(), PageLoadEvent::Finished, &page);
    }
}
