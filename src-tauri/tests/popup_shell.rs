// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the shell side of the toolbar popups (`docs/wiki/browser-shell.md`,
//! "Toolbar popups"), on the Tauri mock runtime. The mock runtime does not keep a webview's
//! place, visibility or focus, so the tests read what the shell sends: `popup-show` to open a
//! popup and `popup-closed` when one ends. Each test names its rule.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::sleep;
use std::time::{Duration, Instant};

use eepview_lib::core::Core;
use eepview_lib::layout::Rect;
use eepview_lib::popup::Kind;
use eepview_lib::shell::{chrome, popup, state::Shared};
use serde_json::{Value, json};
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use tauri::{App, Listener, Manager};

type Log = Arc<Mutex<Vec<(String, Value)>>>;

/// The log, also after a failed test thread poisoned it.
fn entries(log: &Log) -> MutexGuard<'_, Vec<(String, Value)>> {
    log.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Adds an event to the log. `popup-closed` goes to the toolbar and to the popup page: one
/// entry for both.
fn record(log: &Log, name: &str, payload: &str) {
    let payload = serde_json::from_str(payload).unwrap_or(Value::Null);
    let entry = (name.to_owned(), payload);
    let mut log = entries(log);
    if log.last() != Some(&entry) {
        log.push(entry);
    }
}

/// An app with the shared state and the bundled webviews, and a log of the popup events. An
/// event sent to two webviews (`popup-closed`) shows once.
fn app() -> (App<MockRuntime>, Log) {
    let built = mock_builder().build(mock_context(noop_assets()));
    let Ok(mut app) = built else {
        unreachable!("the mock app builds");
    };
    app.manage(Shared::<MockRuntime>::new(Core::new(
        None,
        "127.0.0.1:4444",
        0,
    )));
    assert!(chrome::build(&mut app).is_ok(), "the chrome builds");
    let log: Log = Arc::default();
    for name in ["popup-show", "popup-closed"] {
        let log = Arc::clone(&log);
        app.listen_any(name, move |event| record(&log, name, event.payload()));
    }
    (app, log)
}

fn button() -> Rect {
    Rect {
        x: 700.0,
        y: 44.0,
        w: 32.0,
        h: 32.0,
    }
}

fn names(log: &Log) -> Vec<String> {
    entries(log).iter().map(|(n, _)| n.clone()).collect()
}

/// Waits up to 5 s for the log to hold `count` events.
fn wait_for(log: &Log, count: usize) -> Vec<(String, Value)> {
    let end = Instant::now() + Duration::from_secs(5);
    while entries(log).len() < count && Instant::now() < end {
        sleep(Duration::from_millis(10));
    }
    entries(log).clone()
}

/// Gives the shell time to send events that must not come.
fn settle() {
    sleep(Duration::from_millis(300));
}

fn open(app: &App<MockRuntime>, kind: Kind) -> u64 {
    popup::open(app.handle(), kind, button(), &json!({}))
}

#[test]
fn rule_6_open_asks_the_popup_page_to_render_with_a_new_id() {
    // Rule 6 (interface): ids start at 1 and only go up. The page gets `popup-show`.
    let (app, log) = app();
    let first = open(&app, Kind::Menu);
    let events = wait_for(&log, 1);
    assert_eq!(first, 1);
    assert_eq!(events[0].0, "popup-show");
    assert_eq!(events[0].1["id"], json!(1));
    assert_eq!(events[0].1["kind"], json!("menu"));
}

#[test]
fn rule_6_opening_another_kind_closes_the_open_one_first() {
    // Rule 6: Opening a popup of another kind closes the open one first.
    let (app, log) = app();
    let menu = open(&app, Kind::Menu);
    let router = open(&app, Kind::Router);
    let events = wait_for(&log, 4);
    assert!(router > menu);
    assert_eq!(names(&log), ["popup-show", "popup-closed", "popup-show"]);
    assert_eq!(events[1].1["id"], json!(menu));
    assert_eq!(events[1].1["kind"], json!("menu"));
    assert_eq!(events[2].1["id"], json!(router));
}

#[test]
fn rule_6_opening_the_same_kind_closes_nothing() {
    // Rule 6: Opening the same kind again updates it in place.
    let (app, log) = app();
    open(&app, Kind::Suggestions);
    open(&app, Kind::Suggestions);
    wait_for(&log, 2);
    settle();
    assert!(
        !names(&log).contains(&"popup-closed".to_owned()),
        "{:?}",
        names(&log)
    );
}

#[test]
fn rule_8_close_by_id_sends_popup_closed_with_the_refocus_flag() {
    // Rule 8: Esc closes the panel and focus goes back to the dot (`refocus`).
    let (app, log) = app();
    let id = open(&app, Kind::Router);
    popup::close(app.handle(), Some(id), true);
    let events = wait_for(&log, 2);
    assert_eq!(events[1].0, "popup-closed");
    assert_eq!(
        events[1].1,
        json!({ "id": id, "kind": "router", "refocus": true })
    );
}

#[test]
fn rule_9_close_without_refocus_says_so() {
    // Rule 9: a menu item that opens a page closes the menu, without moving the focus.
    let (app, log) = app();
    let id = open(&app, Kind::Menu);
    popup::close(app.handle(), Some(id), false);
    let events = wait_for(&log, 2);
    assert_eq!(
        events[1].1,
        json!({ "id": id, "kind": "menu", "refocus": false })
    );
}

#[test]
fn rule_11_a_window_resize_closes_whatever_popup_is_open() {
    // Rule 11: A window resize closes the open popup. The shell closes it with no id.
    for kind in [Kind::Suggestions, Kind::Menu, Kind::Router, Kind::Hint] {
        let (app, log) = app();
        let id = open(&app, kind);
        popup::close(app.handle(), None, false);
        let events = wait_for(&log, 2);
        assert_eq!(events[1].0, "popup-closed", "{kind:?}");
        assert_eq!(events[1].1["id"], json!(id), "{kind:?}");
    }
}

#[test]
fn rule_12_a_close_for_a_popup_that_is_no_longer_open_does_nothing() {
    // Rule 12: A close for a popup that is no longer open does nothing.
    let (app, log) = app();
    let old = open(&app, Kind::Suggestions);
    let new = open(&app, Kind::Menu);
    wait_for(&log, 4);
    popup::close(app.handle(), Some(old), true);
    settle();
    assert_eq!(names(&log), ["popup-show", "popup-closed", "popup-show"]);
    popup::close(app.handle(), Some(new), false);
    let events = wait_for(&log, 4);
    assert_eq!(events[3].1["id"], json!(new));
}

#[test]
fn rule_12_closing_twice_sends_one_popup_closed() {
    // Rule 12: the second close for the same id is stale.
    let (app, log) = app();
    let id = open(&app, Kind::Router);
    popup::close(app.handle(), Some(id), false);
    popup::close(app.handle(), Some(id), false);
    popup::close(app.handle(), None, false);
    wait_for(&log, 2);
    settle();
    assert_eq!(names(&log), ["popup-show", "popup-closed"]);
}

#[test]
fn rule_12_a_size_report_for_a_closed_popup_does_nothing() {
    // Rule 12: A size report for a popup that is no longer open does nothing: it sends no event
    // and does not open the popup again.
    let (app, log) = app();
    let id = open(&app, Kind::Menu);
    popup::close(app.handle(), Some(id), false);
    wait_for(&log, 2);
    popup::sized(app.handle(), id, (200.0, 100.0));
    popup::sized(app.handle(), id + 50, (200.0, 100.0));
    settle();
    assert_eq!(names(&log), ["popup-show", "popup-closed"]);
    popup::close(app.handle(), None, false);
    settle();
    assert_eq!(names(&log).len(), 2, "the size report reopened the popup");
}

#[test]
fn rule_12_a_close_with_nothing_open_does_nothing() {
    // Rule 12: no popup is open, so no event goes out.
    let (app, log) = app();
    popup::close(app.handle(), None, false);
    popup::close(app.handle(), Some(1), true);
    settle();
    assert!(names(&log).is_empty(), "{:?}", names(&log));
}

#[test]
fn rule_7_the_hint_opens_and_closes_like_any_popup() {
    // Rule 7: The router hint shows while the mouse is over the dot and hides when it leaves.
    let (app, log) = app();
    let id = popup::open(
        app.handle(),
        Kind::Hint,
        button(),
        &json!({ "title": "Router", "text": "Connected." }),
    );
    let events = wait_for(&log, 1);
    assert_eq!(events[0].1["kind"], json!("hint"));
    assert_eq!(
        events[0].1["data"],
        json!({ "title": "Router", "text": "Connected." })
    );
    popup::close(app.handle(), Some(id), false);
    assert_eq!(wait_for(&log, 2)[1].1["kind"], json!("hint"));
}

#[test]
fn rule_3_the_page_is_told_the_anchor_width() {
    // Rule 3: The suggestions take the width of the address field, so the page gets it.
    let (app, log) = app();
    let field = Rect {
        x: 120.0,
        y: 48.0,
        w: 500.0,
        h: 28.0,
    };
    popup::open(
        app.handle(),
        Kind::Suggestions,
        field,
        &json!({ "items": [], "index": 0 }),
    );
    let events = wait_for(&log, 1);
    assert_eq!(events[0].1["anchorWidth"], json!(500.0));
}
