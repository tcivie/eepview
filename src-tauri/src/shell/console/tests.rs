// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the console view (`docs/wiki/router-console.md`): the window and
//! the webview (R8, R9), the result store and `console-changed` (R6), `console_open` (R12)
//! and `console_status` (R13). They use only the public interface and the mock runtime.
//! Detection itself (`detect_now`, the 10 s re-check) is tested in `net/console/tests.rs`,
//! where the sockets are.

use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{App, Listener, Manager};

use crate::net::console::{ConsoleInfo, ConsoleKind, ConsolePage, VerifiedConsole};
use crate::net::testing::FakeConsole;
use crate::shell::console::{
    CONSOLE_LABEL, CONSOLE_WINDOW, ConsoleWebview, RETRY_EVERY, RETRY_FOR, close, current,
    set_console,
};
use crate::shell::testing::{Mock, app, invoke, wait_for};

// ---------------------------------------------------------------- helpers

/// A fake console and its verified handle (keep the fake alive).
fn console(kind: ConsoleKind) -> (FakeConsole, VerifiedConsole) {
    let fake = FakeConsole::start(kind);
    let verified = fake.verified();
    (fake, verified)
}

fn window_open(app: &App<Mock>) -> bool {
    app.get_window(CONSOLE_WINDOW).is_some()
}

fn open(app: &App<Mock>, console: &VerifiedConsole, page: ConsolePage) {
    ConsoleWebview::open(app.handle(), console, page).unwrap();
}

/// The `console-changed` payloads, as JSON.
fn changes(app: &App<Mock>) -> Receiver<Value> {
    let (tx, rx) = channel();
    app.listen("console-changed", move |event| {
        let value: Value = serde_json::from_str(event.payload()).unwrap();
        let _ = tx.send(value);
    });
    rx
}

fn info_json(info: &ConsoleInfo) -> Value {
    serde_json::to_value(info).unwrap()
}

fn quiet(rx: &Receiver<Value>) -> bool {
    rx.recv_timeout(Duration::from_millis(400)).is_err()
}

// ---------------------------------------------------------------- R8

#[test]
fn r8_the_labels_are_console_and_console_window() {
    // R8: the webview label is exactly `console`, the window `console-window`.
    assert_eq!(CONSOLE_LABEL, "console");
    assert_eq!(CONSOLE_WINDOW, "console-window");
}

#[test]
fn r8_open_builds_the_console_webview_in_its_own_window() {
    // R8: label `console` in a window labelled `console-window`.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    let webview = ConsoleWebview::open(app.handle(), &console, ConsolePage::Home).unwrap();
    assert_eq!(webview.label(), "console");
    assert_eq!(webview.window().label(), "console-window");
    assert!(app.get_webview("console").is_some());
    assert!(window_open(&app));
}

#[test]
fn r8_the_console_is_never_a_tab_webview_and_not_in_the_main_window() {
    // R8: never a tab-* webview.
    let app = app();
    let (_fake, console) = console(ConsoleKind::I2pd);
    let webview = ConsoleWebview::open(app.handle(), &console, ConsolePage::Home).unwrap();
    assert!(!webview.label().starts_with("tab-"));
    assert_ne!(webview.window().label(), "main");
    let tabs = app
        .webviews()
        .keys()
        .filter(|l| l.starts_with("tab-"))
        .count();
    assert_eq!(tabs, 0, "opening the console adds no tab webview");
}

#[test]
fn r8_opening_twice_reuses_the_one_window() {
    // R12: reuses the open window.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console, ConsolePage::Home);
    let before = app.webviews().len();
    open(&app, &console, ConsolePage::Tunnels);
    assert_eq!(app.webviews().len(), before);
    let consoles = app.webviews().keys().filter(|l| *l == "console").count();
    assert_eq!(consoles, 1);
}

// ---------------------------------------------------------------- R9

#[test]
fn r9_the_view_loads_the_page_on_the_detected_origin() {
    // R9: scheme http, host 127.0.0.1, the detected port.
    let app = app();
    let (fake, console) = console(ConsoleKind::Java);
    let webview = ConsoleWebview::open(app.handle(), &console, ConsolePage::Config).unwrap();
    let url = webview.url().unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert_eq!(url.port_or_known_default(), Some(fake.port()));
    assert_eq!(url.path(), "/config");
}

#[test]
fn r9_i2pd_pages_load_their_own_paths() {
    // R7 and R9: the i2pd Config page is /?page=commands.
    let app = app();
    let (fake, console) = console(ConsoleKind::I2pd);
    let webview = ConsoleWebview::open(app.handle(), &console, ConsolePage::Config).unwrap();
    let want = format!("http://127.0.0.1:{}/?page=commands", fake.port());
    assert_eq!(webview.url().unwrap().as_str(), want);
}

#[test]
fn r12_opening_again_loads_the_new_page_in_the_same_view() {
    // R12: reuses the open one and loads the page.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console, ConsolePage::Home);
    let webview = ConsoleWebview::open(app.handle(), &console, ConsolePage::Logs).unwrap();
    assert!(wait_for(|| webview
        .url()
        .is_ok_and(|u| u.path() == "/logs")));
}

// ---------------------------------------------------------------- R6: store and event

#[test]
fn r6_set_console_stores_the_result_and_current_gives_it_back() {
    // R6: the stored detection result.
    let app = app();
    assert!(current(app.handle()).is_none());
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found.clone()));
    assert_eq!(current(app.handle()), Some(found));
    set_console(app.handle(), None);
    assert!(current(app.handle()).is_none());
}

#[test]
fn r6_a_new_console_emits_console_changed_with_its_info() {
    // R6: when the result changes, eepview emits console-changed.
    let app = app();
    let events = changes(&app);
    let (_fake, found) = console(ConsoleKind::I2pd);
    set_console(app.handle(), Some(found.clone()));
    let sent = events
        .recv_timeout(Duration::from_secs(10))
        .expect("console-changed");
    assert_eq!(sent, info_json(&found.info()));
}

#[test]
fn r6_a_console_that_goes_away_emits_found_false() {
    // R6: the console goes away, the UI hears it.
    let app = app();
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found));
    let events = changes(&app);
    set_console(app.handle(), None);
    let sent = events
        .recv_timeout(Duration::from_secs(10))
        .expect("console-changed");
    assert_eq!(sent, info_json(&ConsoleInfo::none()));
}

#[test]
fn r6_the_same_result_again_emits_nothing() {
    // R6: only a change emits.
    let app = app();
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), None);
    let events = changes(&app);
    assert!(quiet(&events), "none to none is no change");
    set_console(app.handle(), Some(found.clone()));
    events
        .recv_timeout(Duration::from_secs(10))
        .expect("first change");
    set_console(app.handle(), Some(found));
    assert!(quiet(&events), "same console again is no change");
}

#[test]
fn r6_a_console_on_another_port_emits_a_change() {
    // R6: a console that moves to another port is a change.
    let app = app();
    let (_a, first) = console(ConsoleKind::Java);
    let (_b, second) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(first));
    let events = changes(&app);
    set_console(app.handle(), Some(second.clone()));
    let sent = events
        .recv_timeout(Duration::from_secs(10))
        .expect("console-changed");
    assert_eq!(sent, info_json(&second.info()));
}

// ---------------------------------------------------------------- R6: the window closes

#[test]
fn r6_the_window_closes_when_the_console_goes_away() {
    // R6: the console goes away, the console window closes.
    let app = app();
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found.clone()));
    open(&app, &found, ConsolePage::Home);
    assert!(window_open(&app));
    set_console(app.handle(), None);
    assert!(wait_for(|| app.get_webview(CONSOLE_LABEL).is_none()));
}

#[test]
fn r6_the_window_closes_when_the_console_moves_to_another_port() {
    // R6: the console moves, the console window closes.
    let app = app();
    let (_a, first) = console(ConsoleKind::Java);
    let (_b, second) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(first.clone()));
    open(&app, &first, ConsolePage::Home);
    set_console(app.handle(), Some(second));
    assert!(wait_for(|| app.get_webview(CONSOLE_LABEL).is_none()));
}

#[test]
fn r6_the_same_console_again_keeps_the_window_open() {
    // R6: no change, no close.
    let app = app();
    let (_fake, found) = console(ConsoleKind::I2pd);
    set_console(app.handle(), Some(found.clone()));
    open(&app, &found, ConsolePage::Home);
    set_console(app.handle(), Some(found));
    thread::sleep(Duration::from_millis(500));
    assert!(window_open(&app));
}

#[test]
fn r6_close_closes_the_window_and_is_safe_when_none_is_open() {
    // R6: close() closes the console window, if open.
    let app = app();
    close(app.handle());
    let (_fake, found) = console(ConsoleKind::Java);
    open(&app, &found, ConsolePage::Home);
    close(app.handle());
    assert!(wait_for(|| app.get_webview(CONSOLE_LABEL).is_none()));
}

// ---------------------------------------------------------------- R12

#[test]
fn r12_with_no_console_it_answers_no_console() {
    // R12: {ok: false, reason: "no-console"}.
    let app = app();
    let got = invoke(&app, "console_open", json!({"page": "home"})).unwrap();
    assert_eq!(got["ok"], json!(false));
    assert_eq!(got["reason"], json!("no-console"));
    assert!(!window_open(&app));
}

#[test]
fn r12_a_page_the_router_does_not_have_answers_no_page() {
    // R12: i2pd has no address book and no log page.
    let app = app();
    let (_fake, found) = console(ConsoleKind::I2pd);
    set_console(app.handle(), Some(found));
    for page in ["addressbook", "logs"] {
        let got = invoke(&app, "console_open", json!({"page": page})).unwrap();
        assert_eq!(got["ok"], json!(false), "{page}");
        assert_eq!(got["reason"], json!("no-page"), "{page}");
    }
    assert!(!window_open(&app), "no-page opens no window");
}

#[test]
fn r12_an_unknown_page_key_is_an_error() {
    // R12: for an unknown page key: an error.
    let app = app();
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found));
    assert!(invoke(&app, "console_open", json!({"page": "bogus"})).is_err());
    assert!(invoke(&app, "console_open", json!({"page": "/home"})).is_err());
    assert!(!window_open(&app));
}

#[test]
fn r12_a_known_page_opens_the_window_and_answers_ok() {
    // R12: opens the console window, loads the page and answers {ok: true}.
    let app = app();
    let (fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found));
    let got = invoke(&app, "console_open", json!({"page": "tunnels"})).unwrap();
    assert_eq!(got["ok"], json!(true));
    assert!(got.get("reason").is_none_or(Value::is_null));
    assert!(wait_for(|| window_open(&app)));
    let view = app.get_webview("console").expect("console webview");
    assert_eq!(
        view.url().unwrap().as_str(),
        format!("http://127.0.0.1:{}/tunnels", fake.port())
    );
}

#[test]
fn r12_every_page_of_the_router_opens() {
    // R7 and R12: each listed page answers ok.
    let app = app();
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found.clone()));
    for page in found.pages() {
        let got = invoke(&app, "console_open", json!({"page": page.key()})).unwrap();
        assert_eq!(got["ok"], json!(true), "{page:?}");
    }
    let consoles = app.webviews().keys().filter(|l| *l == "console").count();
    assert_eq!(consoles, 1, "one window is reused");
}

// ---------------------------------------------------------------- R13

#[test]
fn r13_status_with_no_console_is_the_none_info() {
    // R13: the current ConsoleInfo.
    let app = app();
    let got = invoke(&app, "console_status", json!({})).unwrap();
    assert_eq!(got, info_json(&ConsoleInfo::none()));
}

#[test]
fn r13_status_answers_the_stored_console() {
    // R13: the current ConsoleInfo.
    let app = app();
    let (_fake, found) = console(ConsoleKind::I2pd);
    set_console(app.handle(), Some(found.clone()));
    let got = invoke(&app, "console_status", json!({})).unwrap();
    assert_eq!(got, info_json(&found.info()));
}

#[test]
fn r13_status_does_not_probe() {
    // R13 and R6: console_status never probes, not even the stored console.
    let app = app();
    let (fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found));
    let before = fake.requests().len();
    for _ in 0..3 {
        invoke(&app, "console_status", json!({})).unwrap();
    }
    assert_eq!(fake.requests().len(), before);
}

#[test]
fn r6_storing_and_opening_a_console_probe_nothing() {
    // R6: a stored console is not probed by opening the window or by reading the store.
    let app = app();
    let (fake, found) = console(ConsoleKind::Java);
    let before = fake.requests().len();
    set_console(app.handle(), Some(found.clone()));
    open(&app, &found, ConsolePage::Home);
    let _ = current(app.handle());
    assert_eq!(fake.requests().len(), before);
}

// ---------------------------------------------------------------- R20, R21

#[test]
fn r20_retry_every_10_seconds_for_2_minutes() {
    // R20: retries every 10 s for 2 minutes (12 retries).
    assert_eq!(RETRY_EVERY, Duration::from_secs(10));
    assert_eq!(RETRY_FOR, Duration::from_mins(2));
    assert_eq!(RETRY_FOR.as_secs() / RETRY_EVERY.as_secs(), 12);
}

#[test]
fn r21_the_re_check_period_is_the_same_10_seconds() {
    // R21: a re-check runs every 10 s (RETRY_EVERY, R20 and R21).
    assert_eq!(RETRY_EVERY.as_secs(), 10);
}
