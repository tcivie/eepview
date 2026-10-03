// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Places the webviews and shows the one the active tab needs.

use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Rect as TauriRect, Runtime, Url, Webview,
    Window,
};

use std::sync::atomic::Ordering;

use super::state::{lock, shared};
use crate::core::View;
use crate::hover::HoverText;
use crate::layout::{self, Rect};
use crate::nav::internal_file;
use crate::types::ChromeInsets;

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

/// Hides the status bubble at once when the text goes. New text shows once the status page
/// has measured its pill ([`status_sized`]), so the webview always fits the pill.
pub fn status<R: Runtime>(app: &AppHandle<R>, text: Option<&HoverText>) {
    if text.is_none() {
        hide(app, "status");
    }
}

/// The status page measured its pill: fit the webview to it and show it, while there is
/// text to show.
pub fn status_sized<R: Runtime>(app: &AppHandle<R>, size: (f64, f64)) {
    let Some(webview) = app.get_webview("status") else {
        return;
    };
    if lock(&shared(app).core).hover_text().is_none() {
        return;
    }
    let (rect, corner) = layout::status(content_rect(app), size, cursor(app));
    let _ = tauri::Emitter::emit_to(app, "status", "status-side", corner.name());
    place(&webview, rect);
    let _ = webview.show();
}

/// The mouse in window points, when the window knows it.
fn cursor<R: Runtime>(app: &AppHandle<R>) -> Option<(f64, f64)> {
    let window = app.get_window("main")?;
    let at = window.cursor_position().ok()?;
    let origin = window.inner_position().ok()?;
    let scale = window.scale_factor().unwrap_or(1.0);
    Some((
        (at.x - f64::from(origin.x)) / scale,
        (at.y - f64::from(origin.y)) / scale,
    ))
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

/// Sends `fullscreen-changed` when the window enters or leaves full screen, and
/// `chrome-insets-changed` when the button inset moves with it.
pub fn check_fullscreen<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_window("main") else {
        return;
    };
    let insets = chrome_insets(app);
    let now = window.is_fullscreen().unwrap_or(false);
    let before = shared(app).fullscreen.swap(now, Ordering::SeqCst);
    if now != before {
        emit_chrome(app, "fullscreen-changed", now);
    }
    emit_insets_if_moved(app, insets);
}

/// The current `chrome_insets()` answer.
pub fn chrome_insets<R: Runtime>(app: &AppHandle<R>) -> ChromeInsets {
    let state = shared(app);
    let buttons = *lock(&state.buttons);
    ChromeInsets {
        left: layout::chrome_inset(buttons, state.fullscreen.load(Ordering::SeqCst)),
    }
}

/// Puts the macOS window buttons on the tab row center, then measures them. `AppKit` lays the
/// title bar out again on resizes and focus changes, so this runs after each of them.
pub fn place_buttons<R: Runtime>(app: &AppHandle<R>) {
    let Some(toolbar) = app.get_webview("toolbar") else {
        return;
    };
    let handle = app.clone();
    let _ = toolbar.with_webview(move |platform| {
        let buttons = eepview_platform::place_window_buttons(&platform, layout::TAB_ROW_CENTER);
        let edges = buttons.map(|b| (b.left, b.right));
        super::apply::outside(move || store_buttons(&handle, edges));
    });
}

/// Records measured button edges and tells the UI when the inset moved.
pub fn store_buttons<R: Runtime>(app: &AppHandle<R>, buttons: Option<(f64, f64)>) {
    let insets = chrome_insets(app);
    *lock(&shared(app).buttons) = buttons;
    emit_insets_if_moved(app, insets);
}

fn emit_insets_if_moved<R: Runtime>(app: &AppHandle<R>, before: ChromeInsets) {
    let now = chrome_insets(app);
    if now != before {
        emit_chrome(app, "chrome-insets-changed", now);
    }
}

fn emit_chrome<R: Runtime, S: serde::Serialize + Clone>(app: &AppHandle<R>, name: &str, body: S) {
    for label in ["toolbar", "internal"] {
        let _ = tauri::Emitter::emit_to(app, label, name, body.clone());
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::core::router::status_of;
    use crate::net::testing::FakeRouter;
    use crate::net::verify::verify;
    use crate::shell::apply::apply;
    use crate::shell::testing::{Mock, app, bare, core, label, open_gate};

    /// Verifies a fake router, then opens `url` in the active tab.
    fn browse(app: &tauri::App<Mock>, gate: bool, url: &str) {
        let router = FakeRouter::start();
        if gate {
            open_gate(app, &router);
        }
        let status = status_of(&verify(router.addr), "127.0.0.1:4444", gate);
        let mut fx = core(app).router_changed(status);
        fx.extend(core(app).navigate(url).1);
        apply(app.handle(), fx);
    }

    fn internal_url(app: &tauri::App<Mock>) -> String {
        app.get_webview("internal")
            .unwrap()
            .url()
            .unwrap()
            .to_string()
    }

    #[test]
    fn insets_follow_buttons_and_fullscreen() {
        let app = bare();
        let handle = app.handle();
        assert!(chrome_insets(handle).left.abs() < f64::EPSILON);
        store_buttons(handle, Some((14.0, 66.0)));
        assert!((chrome_insets(handle).left - 80.0).abs() < f64::EPSILON);
        shared(handle).fullscreen.store(true, Ordering::SeqCst);
        assert!(chrome_insets(handle).left.abs() < f64::EPSILON);
        place_buttons(handle);
    }

    #[test]
    fn nothing_to_place_without_the_window() {
        let app = bare();
        sync(app.handle());
        status(app.handle(), None);
        check_fullscreen(app.handle());
        let rect = content_rect(app.handle());
        assert!((rect.y - layout::TOOLBAR).abs() < f64::EPSILON);
        assert!((rect.w - 1200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn internal_pages_load_in_the_internal_webview() {
        let app = app();
        sync(app.handle());
        assert!(
            internal_url(&app).ends_with("src/ui/home.html"),
            "{}",
            internal_url(&app)
        );
        core(&app).navigate("eepview://bookmarks");
        sync(app.handle());
        assert!(internal_url(&app).ends_with("src/ui/bookmarks.html"));
    }

    #[test]
    fn web_tabs_show_their_webview() {
        let app = app();
        browse(&app, true, "http://a.i2p/");
        let tab = core(&app).tabs().active().unwrap().id;
        assert_eq!(core(&app).view(), View::Web(tab));
        assert!(label(&app, tab).is_some());
        sync(app.handle());
        core(&app).tab_new(None, crate::tabs::Place::End);
        sync(app.handle());
    }

    #[test]
    fn a_closed_last_tab_comes_back_with_its_webview() {
        let app = app();
        browse(&app, true, "http://a.i2p/");
        let first = core(&app).tabs().active().unwrap().id;
        let fx = core(&app).tab_close(first);
        apply(app.handle(), fx);
        assert_eq!(label(&app, first), None);
        let fx = core(&app).tab_reopen();
        apply(app.handle(), fx);
        let tab = core(&app).tabs().active().unwrap().id;
        assert_eq!(core(&app).tabs().active().unwrap().url, "http://a.i2p/");
        assert_eq!(core(&app).view(), View::Web(tab));
        assert!(label(&app, tab).is_some());
    }

    #[test]
    fn a_web_tab_without_a_webview_hides_the_internal_page() {
        let app = app();
        browse(&app, false, "http://a.i2p/");
        sync(app.handle());
        let tab = core(&app).tabs().active().unwrap().id;
        assert_eq!(label(&app, tab), None);
    }

    #[test]
    fn the_status_bubble_shows_and_hides() {
        let app = app();
        let text = HoverText {
            text: "http://a.i2p/".into(),
            blocked: false,
        };
        status(app.handle(), Some(&text));
        status_sized(app.handle(), (90.0, 20.0));
        core(&app).hover_link(1, Some("http://a.i2p/"));
        status(app.handle(), None);
        let window = app.get_window("main").unwrap();
        raise_chrome(app.handle(), &window);
        sync_stop_item(app.handle());
    }

    #[test]
    fn fullscreen_changes_are_sent_once() {
        let app = app();
        let state = shared(app.handle());
        state.fullscreen.store(true, Ordering::SeqCst);
        check_fullscreen(app.handle());
        assert!(!state.fullscreen.load(Ordering::SeqCst));
        check_fullscreen(app.handle());
        assert!(!state.fullscreen.load(Ordering::SeqCst));
    }
}
