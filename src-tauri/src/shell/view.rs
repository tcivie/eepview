//! Places the webviews and shows the one the active tab needs.

use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Rect as TauriRect, Runtime, Url, Webview,
    Window,
};

use super::state::{lock, shared};
use crate::core::View;
use crate::hover::HoverText;
use crate::layout::{self, Rect};
use crate::nav::internal_file;

/// The main window size in logical pixels.
fn window_size<R: Runtime>(window: &Window<R>) -> (f64, f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    window.inner_size().map_or((1200.0, 800.0), |s| {
        let logical = s.to_logical::<f64>(scale);
        (logical.width, logical.height)
    })
}

fn rects<R: Runtime>(app: &AppHandle<R>) -> Option<(Rect, Rect)> {
    let window = app.get_window("main")?;
    let (w, h) = window_size(&window);
    let state = shared(app);
    let core = lock(&state.core);
    let bar = layout::toolbar_height(core.find_open(), core.toolbar_request());
    Some(layout::split(w, h, bar, core.find_open()))
}

/// The content area.
pub fn content_rect<R: Runtime>(app: &AppHandle<R>) -> Rect {
    rects(app).map_or(
        Rect {
            x: 0.0,
            y: layout::TOOLBAR,
            w: 1200.0,
            h: 716.0,
        },
        |(_, c)| c,
    )
}

fn place<R: Runtime>(webview: &Webview<R>, rect: Rect) {
    let bounds = TauriRect {
        position: LogicalPosition::new(rect.x, rect.y).into(),
        size: LogicalSize::new(rect.w, rect.h).into(),
    };
    let _ = webview.set_bounds(bounds);
}

/// Lays out every webview and shows the right content webview.
pub fn sync<R: Runtime>(app: &AppHandle<R>) {
    let Some((bar, content)) = rects(app) else {
        return;
    };
    if let Some(toolbar) = app.get_webview("toolbar") {
        place(&toolbar, bar);
    }
    let view = lock(&shared(app).core).view();
    let active = match &view {
        View::Web(tab) => lock(&shared(app).labels).get(tab).cloned(),
        View::Internal(_) => None,
    };
    let labels: Vec<String> = lock(&shared(app).labels).values().cloned().collect();
    for label in labels.iter().filter(|l| Some(*l) != active.as_ref()) {
        hide(app, label);
    }
    super::log::view(&format!("{view:?} active={active:?} labels={labels:?}"));
    match (view, active) {
        (View::Web(_), Some(label)) => show(app, &label, content),
        (View::Internal(page), _) => show_internal(app, &page, content),
        (View::Web(_), None) => hide(app, "internal"),
    }
    sync_stop_item(app);
}

fn hide<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Some(webview) = app.get_webview(label) {
        let _ = webview.hide();
    }
}

fn show<R: Runtime>(app: &AppHandle<R>, label: &str, rect: Rect) {
    hide(app, "internal");
    if let Some(webview) = app.get_webview(label) {
        place(&webview, rect);
        let _ = webview.show();
    }
}

fn show_internal<R: Runtime>(app: &AppHandle<R>, page: &str, rect: Rect) {
    let Some(webview) = app.get_webview("internal") else {
        return;
    };
    let base = lock(&shared(app).base).clone();
    let target = base
        .zip(internal_file(page))
        .and_then(|(b, f)| b.join(&f).ok());
    if let Some(url) = target.filter(|u| webview.url().ok().as_ref() != Some(u)) {
        let _ = webview.navigate(url);
    }
    place(&webview, rect);
    let _ = webview.show();
}

/// Shows the status bubble with `text`, or hides it.
pub fn status<R: Runtime>(app: &AppHandle<R>, text: Option<&HoverText>) {
    let Some(webview) = app.get_webview("status") else {
        return;
    };
    match text {
        Some(t) => {
            place(
                &webview,
                layout::status(content_rect(app), t.text.chars().count()),
            );
            let _ = webview.show();
        }
        None => {
            let _ = webview.hide();
        }
    }
}

/// Puts the toolbar and the status bubble above the tab webviews again: a new child webview
/// lands on top, and the toolbar must overlap the content while a popup is open.
pub fn raise_chrome<R: Runtime>(app: &AppHandle<R>, window: &Window<R>) {
    for label in ["toolbar", "status"] {
        if let Some(webview) = app.get_webview(label) {
            let _ = webview.reparent(window);
        }
    }
}

/// Enables the Stop item (Esc) only while the active tab loads.
pub fn sync_stop_item<R: Runtime>(app: &AppHandle<R>) {
    let loading = lock(&shared(app).core)
        .tabs()
        .active()
        .is_some_and(|t| t.loading);
    if let Some(item) = lock(&shared(app).stop_item).as_ref() {
        let _ = item.set_enabled(loading);
    }
}

/// The origin of the bundled pages, read from the toolbar webview.
pub fn remember_base<R: Runtime>(app: &AppHandle<R>, toolbar: &Webview<R>) {
    if let Ok(url) = toolbar.url() {
        let base: Option<Url> = url.join("/").ok();
        *lock(&shared(app).base) = base;
    }
}

/// Sends `fullscreen-changed` when the window enters or leaves full screen (the UI drops the
/// traffic-light inset in full screen).
pub fn check_fullscreen<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_window("main") else {
        return;
    };
    let now = window.is_fullscreen().unwrap_or(false);
    let before = shared(app)
        .fullscreen
        .swap(now, std::sync::atomic::Ordering::SeqCst);
    if now != before {
        for label in ["toolbar", "internal"] {
            let _ = tauri::Emitter::emit_to(app, label, "fullscreen-changed", now);
        }
    }
}
