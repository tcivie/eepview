// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the popup commands and their events (`docs/wiki/browser-shell.md`,
//! "Toolbar popups"), on the Tauri mock runtime: `popup_open`, `popup_size` and `popup_close`
//! as the toolbar and the popup page call them, and the events the shell sends back. The mock
//! runtime keeps no webview place, visibility or focus, so the tests read the events and the
//! one-popup state. Each test names its rule.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::sleep;
use std::time::{Duration, Instant};

use eepview_lib::core::Core;
use eepview_lib::popup::{Closed, Kind};
use eepview_lib::shell::popup::Anchor;
use eepview_lib::shell::state::{Shared, lock, shared};
use eepview_lib::shell::{chrome, commands, popup, view};
use serde_json::{Value, json};
use tauri::async_runtime::block_on;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use tauri::{App, AppHandle, Emitter, Listener, Manager, WindowBuilder};

type Log = Arc<Mutex<Vec<(String, Value)>>>;

const KINDS: [Kind; 4] = [Kind::Suggestions, Kind::Menu, Kind::Router, Kind::Hint];

fn entries(log: &Log) -> MutexGuard<'_, Vec<(String, Value)>> {
    log.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A mock app with the shared state and no window.
fn bare() -> App<MockRuntime> {
    let built = mock_builder().build(mock_context(noop_assets()));
    let Ok(app) = built else {
        unreachable!("the mock app builds");
    };
    app.manage(Shared::<MockRuntime>::new(Core::new(
        None,
        "127.0.0.1:4444",
        0,
    )));
    app
}

/// Logs the popup events and `status-side`.
fn watch(app: &App<MockRuntime>) -> Log {
    let log: Log = Arc::default();
    for name in ["popup-show", "popup-closed", "status-side"] {
        let log = Arc::clone(&log);
        app.listen_any(name, move |event| {
            let payload = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
            entries(&log).push((name.to_owned(), payload));
        });
    }
    log
}

/// An app with the bundled webviews.
fn app() -> (App<MockRuntime>, Log) {
    let mut app = bare();
    assert!(chrome::build(&mut app).is_ok(), "the chrome builds");
    let log = watch(&app);
    (app, log)
}

fn anchor(x: f64, y: f64, width: f64, height: f64) -> Anchor {
    Anchor {
        x,
        y,
        width,
        height,
    }
}

fn button() -> Anchor {
    anchor(700.0, 44.0, 32.0, 32.0)
}

fn open(app: &AppHandle<MockRuntime>, kind: Kind, data: Option<Value>) -> u64 {
    match block_on(commands::popup_open(app.clone(), kind, button(), data)) {
        Ok(id) => id,
        Err(e) => unreachable!("popup_open failed: {e}"),
    }
}

fn current(app: &AppHandle<MockRuntime>) -> Option<Closed> {
    lock(&shared(app).popups).current()
}

fn wait_for(log: &Log, count: usize) -> Vec<(String, Value)> {
    let end = Instant::now() + Duration::from_secs(5);
    while entries(log).len() < count && Instant::now() < end {
        sleep(Duration::from_millis(10));
    }
    entries(log).clone()
}

fn settle() {
    sleep(Duration::from_millis(300));
}

#[test]
fn rule_3_the_anchor_is_a_rectangle_when_it_is_finite_and_not_negative() {
    // Rule 3: a popup opens under its anchor, so the anchor must be a real rectangle.
    let good = anchor(10.0, 20.0, 30.0, 40.0).rect();
    assert_eq!(
        good.map(|r| (r.x, r.y, r.w, r.h)),
        Some((10.0, 20.0, 30.0, 40.0))
    );
    assert!(
        anchor(0.0, 0.0, 0.0, 0.0).rect().is_some(),
        "an empty anchor is fine"
    );
    for bad in [
        anchor(f64::NAN, 0.0, 1.0, 1.0),
        anchor(0.0, f64::INFINITY, 1.0, 1.0),
        anchor(0.0, 0.0, f64::NEG_INFINITY, 1.0),
        anchor(0.0, 0.0, 1.0, f64::NAN),
        anchor(0.0, 0.0, -1.0, 1.0),
        anchor(0.0, 0.0, 1.0, -1.0),
    ] {
        assert!(bad.rect().is_none(), "{bad:?}");
    }
}

#[test]
fn rule_3_the_anchor_comes_from_json_as_x_y_width_height() {
    // Rule 3 (IPC contract): `anchor: {x, y, width, height}`.
    let parsed: Anchor = serde_json::from_value(json!({ "x": 1, "y": 2, "width": 3, "height": 4 }))
        .expect("the anchor parses");
    assert_eq!(
        parsed.rect().map(|r| (r.x, r.y, r.w, r.h)),
        Some((1.0, 2.0, 3.0, 4.0))
    );
}

#[test]
fn rule_6_popup_open_returns_the_id_and_sends_popup_show_for_each_kind() {
    // Rule 6: each kind opens as the one popup; the page gets `popup-show` with the data.
    for kind in KINDS {
        let (app, log) = app();
        let data = json!({ "title": "T", "text": "x" });
        let id = open(app.handle(), kind, Some(data.clone()));
        let events = wait_for(&log, 1);
        assert_eq!(id, 1, "{kind:?}");
        assert_eq!(events[0].0, "popup-show");
        assert_eq!(events[0].1["id"], json!(id));
        assert_eq!(events[0].1["anchorWidth"], json!(32.0));
        assert_eq!(
            events[0].1["data"], data,
            "{kind:?}: the data goes through unchanged"
        );
        assert_eq!(current(app.handle()), Some(Closed { id, kind }));
    }
}

#[test]
fn rule_6_popup_open_without_data_sends_null() {
    // Rule 6: `data` is optional.
    let (app, log) = app();
    open(app.handle(), Kind::Menu, None);
    assert_eq!(wait_for(&log, 1)[0].1["data"], Value::Null);
}

#[test]
fn rule_3_popup_open_refuses_an_anchor_that_is_not_a_rectangle() {
    // Rule 3: a bad anchor opens nothing.
    let (app, log) = app();
    let bad = anchor(f64::NAN, 0.0, 1.0, 1.0);
    let result = block_on(commands::popup_open(
        app.handle().clone(),
        Kind::Menu,
        bad,
        None,
    ));
    assert!(result.is_err(), "a NaN anchor opened a popup");
    let negative = anchor(0.0, 0.0, -5.0, 1.0);
    let result = block_on(commands::popup_open(
        app.handle().clone(),
        Kind::Menu,
        negative,
        None,
    ));
    assert!(result.is_err(), "a negative width opened a popup");
    settle();
    assert!(entries(&log).is_empty(), "an event went out");
    assert_eq!(current(app.handle()), None);
}

#[test]
fn rule_5_a_size_report_for_the_open_popup_keeps_it_open() {
    // Rule 5: the popup page reports its size and the shell places and shows the popup.
    for kind in KINDS {
        let (app, log) = app();
        let id = open(app.handle(), kind, None);
        let sized = block_on(commands::popup_size(app.handle().clone(), id, 320.0, 200.0));
        assert!(sized.is_ok(), "{kind:?}");
        settle();
        assert_eq!(current(app.handle()), Some(Closed { id, kind }), "{kind:?}");
        assert_eq!(entries(&log).len(), 1, "{kind:?}: only popup-show went out");
    }
}

#[test]
fn rule_5_a_second_size_report_updates_the_popup_in_place() {
    // Rule 15: a list that grows while open reports its new height; the popup stays.
    let (app, _log) = app();
    let id = open(app.handle(), Kind::Suggestions, None);
    for height in [80.0, 160.0, 5000.0] {
        popup::sized(app.handle(), id, (400.0, height));
        assert_eq!(current(app.handle()).map(|c| c.id), Some(id));
    }
}

#[test]
fn rule_12_a_size_report_with_a_bad_size_or_id_does_nothing() {
    // Rule 12: a size report for a popup that is not open, or a size that is not finite and
    // positive, does nothing.
    let (app, log) = app();
    let before = block_on(commands::popup_size(app.handle().clone(), 1, 100.0, 100.0));
    assert!(
        before.is_ok(),
        "a size report before any open is not an error"
    );
    let id = open(app.handle(), Kind::Menu, None);
    for (w, h) in [
        (0.0, 10.0),
        (10.0, -1.0),
        (f64::NAN, 10.0),
        (10.0, f64::INFINITY),
    ] {
        popup::sized(app.handle(), id, (w, h));
    }
    popup::sized(app.handle(), id + 99, (100.0, 100.0));
    settle();
    assert_eq!(
        current(app.handle()),
        Some(Closed {
            id,
            kind: Kind::Menu
        })
    );
    assert_eq!(entries(&log).len(), 1, "only popup-show went out");
}

#[test]
fn rule_12_a_size_report_without_a_window_or_a_popup_webview_does_nothing() {
    // Rule 12: no window, or no popup webview: nothing to place, and no panic.
    let app = bare();
    let id = open(app.handle(), Kind::Router, None);
    popup::sized(app.handle(), id, (100.0, 100.0));
    assert_eq!(current(app.handle()).map(|c| c.id), Some(id));
    let built = WindowBuilder::new(&app, "main").build();
    assert!(built.is_ok(), "a window without webviews builds");
    popup::sized(app.handle(), id, (100.0, 100.0));
    assert_eq!(current(app.handle()).map(|c| c.id), Some(id));
}

#[test]
fn rule_8_popup_close_closes_by_id_with_or_without_refocus() {
    // Rule 8 and rule 12: the toolbar and the popup page close a popup by its id.
    for (refocus, flag) in [(Some(true), true), (Some(false), false), (None, false)] {
        let (app, log) = app();
        let id = open(app.handle(), Kind::Router, None);
        let closed = block_on(commands::popup_close(app.handle().clone(), id, refocus));
        assert!(closed.is_ok());
        let events = wait_for(&log, 2);
        assert_eq!(
            events[1].1,
            json!({ "id": id, "kind": "router", "refocus": flag })
        );
        assert_eq!(current(app.handle()), None);
    }
}

#[test]
fn rule_12_popup_close_with_a_stale_or_unknown_id_does_nothing() {
    // Rule 12: a close for a popup that is no longer open does nothing.
    let (app, log) = app();
    let old = open(app.handle(), Kind::Suggestions, None);
    let new = open(app.handle(), Kind::Menu, None);
    for id in [old, new + 7, 0] {
        assert!(block_on(commands::popup_close(app.handle().clone(), id, Some(true))).is_ok());
    }
    settle();
    assert_eq!(
        current(app.handle()),
        Some(Closed {
            id: new,
            kind: Kind::Menu
        })
    );
    let names: Vec<String> = entries(&log).iter().map(|(n, _)| n.clone()).collect();
    assert_eq!(
        names,
        ["popup-show", "popup-closed", "popup-closed", "popup-show"]
    );
}

#[test]
fn rule_6_a_second_open_replaces_the_first_and_tells_the_pages() {
    // Rule 6: opening a popup of another kind closes the open one first. The toolbar and the
    // popup page each get `popup-closed` for the first.
    let (app, log) = app();
    let first = open(app.handle(), Kind::Menu, None);
    let second = open(app.handle(), Kind::Router, None);
    let events = wait_for(&log, 4);
    assert_eq!(
        events[1],
        (
            "popup-closed".to_owned(),
            json!({ "id": first, "kind": "menu", "refocus": false })
        )
    );
    assert_eq!(
        events[2], events[1],
        "the toolbar and the page both hear it"
    );
    assert_eq!(events[3].1["id"], json!(second));
    assert_eq!(
        current(app.handle()),
        Some(Closed {
            id: second,
            kind: Kind::Router
        })
    );
}

#[test]
fn rule_11_a_resize_closes_the_popup_by_the_public_close_path() {
    // Rule 11: A window resize closes the open popup (the shell closes with no id).
    let (app, log) = app();
    let id = open(app.handle(), Kind::Menu, None);
    popup::sized(app.handle(), id, (200.0, 100.0));
    popup::close(app.handle(), None, false);
    let events = wait_for(&log, 2);
    assert_eq!(events[1].1["id"], json!(id));
    assert_eq!(current(app.handle()), None);
    popup::close(app.handle(), None, false);
    settle();
    assert_eq!(
        entries(&log).len(),
        3,
        "popup-show, then popup-closed to two pages"
    );
}

#[test]
fn rule_2_the_layout_commands_answer_with_the_window() {
    // Rule 2: the window reports its insets and its full-screen state to the toolbar.
    let (app, _log) = app();
    let fullscreen = block_on(commands::window_fullscreen(app.handle().clone()));
    assert_eq!(fullscreen, Ok(false));
    let insets = block_on(commands::chrome_insets(app.handle().clone()));
    assert!(insets.is_ok_and(|i| i.left >= 0.0 && i.left.is_finite()));
}

/// Makes the core show a link under the mouse, as the `link-hover` flow does.
fn hover_a_link(app: &App<MockRuntime>) {
    let state = shared(app.handle());
    let mut core = lock(&state.core);
    let tab = core.tabs().active().map_or(1, |t| t.id);
    core.hover_link(tab, Some("http://a.i2p/"));
    for generation in 0..8 {
        core.hover_expire(generation);
    }
    assert!(
        core.hover_text().is_some(),
        "the link shows after the delay"
    );
}

#[test]
fn rule_16_the_status_bubble_is_placed_when_its_page_reports_a_size() {
    // The IPC contract: `status-size` fits the bubble while there is text, and `status-side`
    // names its corner.
    let (app, log) = app();
    view::status_sized(app.handle(), (120.0, 24.0));
    settle();
    assert!(entries(&log).is_empty(), "no text: no bubble");
    hover_a_link(&app);
    view::status_sized(app.handle(), (120.0, 24.0));
    let events = wait_for(&log, 1);
    assert_eq!(events[0].0, "status-side");
    assert!(["left", "right"].contains(&events[0].1.as_str().unwrap_or("")));
    assert!(
        app.emit("status-size", json!({ "width": 90.0, "height": 24.0 }))
            .is_ok()
    );
    assert_eq!(
        wait_for(&log, 2).len(),
        2,
        "the page's report placed the bubble"
    );
}
