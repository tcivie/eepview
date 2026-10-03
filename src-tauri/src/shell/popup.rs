// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Shows the toolbar popups in the `popup` webview: placed at the popup's own rectangle
//! over the page, so the toolbar never grows and the page never moves.

use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::state::{lock, shared};
use super::view;
use crate::layout::Rect;
use crate::popup::{Closed, Kind};

/// The anchor of a popup in window points (JSON `{x, y, width, height}`).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Anchor {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Anchor {
    /// The anchor as a rectangle; `None` when a value is not finite or a side is negative.
    #[must_use]
    pub fn rect(self) -> Option<Rect> {
        let values = [self.x, self.y, self.width, self.height];
        let ok = values.iter().all(|v| v.is_finite()) && self.width >= 0.0 && self.height >= 0.0;
        ok.then_some(Rect {
            x: self.x,
            y: self.y,
            w: self.width,
            h: self.height,
        })
    }
}

/// Opens a popup: closes one of another kind, then asks the page to render and measure
/// this one (`popup-show`). The webview shows once the page reports its size.
pub fn open<R: Runtime>(app: &AppHandle<R>, kind: Kind, anchor: Rect, data: &Value) -> u64 {
    let (id, replaced) = lock(&shared(app).popups).open(kind, anchor);
    if let Some(old) = replaced {
        hide(app);
        notify(app, old, false);
    }
    let body = json!({ "id": id, "kind": kind, "anchorWidth": anchor.w, "data": data });
    let _ = app.emit_to("popup", "popup-show", body);
    id
}

/// The page measured popup `id`: place the webview at the popup's rectangle and show it.
pub fn sized<R: Runtime>(app: &AppHandle<R>, id: u64, size: (f64, f64)) {
    let Some(window) = app.get_window("main") else {
        return;
    };
    let area = view::window_size(&window);
    let state = shared(app);
    let placed = {
        let mut popups = lock(&state.popups);
        popups.size(id, size, area).zip(popups.current())
    };
    let Some((rect, open)) = placed else {
        return;
    };
    let Some(webview) = app.get_webview("popup") else {
        return;
    };
    view::place(&webview, rect);
    let _ = webview.show();
    if open.kind.takes_focus() {
        let _ = webview.set_focus();
    }
}

/// Closes popup `id` (any popup for `None`). `refocus` gives the focus back to the toolbar.
pub fn close<R: Runtime>(app: &AppHandle<R>, id: Option<u64>, refocus: bool) {
    let closed = lock(&shared(app).popups).close(id);
    let Some(closed) = closed else {
        return;
    };
    hide(app);
    if let Some(toolbar) = app.get_webview("toolbar").filter(|_| refocus) {
        let _ = toolbar.set_focus();
    }
    notify(app, closed, refocus);
}

fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(webview) = app.get_webview("popup") {
        let _ = webview.hide();
    }
}

/// Sends `popup-closed` to the toolbar and the popup page.
fn notify<R: Runtime>(app: &AppHandle<R>, closed: Closed, refocus: bool) {
    let body = json!({ "id": closed.id, "kind": closed.kind, "refocus": refocus });
    for label in ["toolbar", "popup"] {
        let _ = app.emit_to(label, "popup-closed", body.clone());
    }
}
