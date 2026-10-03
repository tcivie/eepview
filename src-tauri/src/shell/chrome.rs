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
            if matches!(payload.event(), PageLoadEvent::Finished) {
                let url = payload.url().clone();
                with_core(webview.app_handle(), |core| core.internal_loaded(&url));
            }
        })
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
