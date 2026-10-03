// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the console view and the console tab (`docs/wiki/router-console.md`):
//! the webview in the main window (R8, R9), the result store and `console-changed` (R6),
//! `console_open` (R12), `console_status` (R13), and the shell side of the console tab
//! (R23 to R30). They use only the public interface and the mock runtime. Detection itself
//! (`detect_now`, the 10 s re-check) is tested in `net/console/tests.rs`, where the sockets
//! are. The pure side of the console tab is tested in `core/console_tab/tests.rs`.

use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{App, Listener, Manager, Url, Webview};

use crate::core::{ConsoleOp, EngineOp, View};
use crate::net::console::{ConsoleInfo, ConsoleKind, VerifiedConsole, probe_path};
use crate::net::testing::FakeConsole;
use crate::shell::console::{
    CONSOLE_LABEL, ConsoleWebview, RETRY_EVERY, RETRY_FOR, close, current, load_after_rules,
    navigation, new_window, page_load, run, set_console, title_changed,
};
use crate::shell::testing::{Mock, app, core, invoke, label, wait_for};

// ---------------------------------------------------------------- helpers

/// A fake console and its verified handle (keep the fake alive).
fn console(kind: ConsoleKind) -> (FakeConsole, VerifiedConsole) {
    let fake = FakeConsole::start(kind);
    let verified = fake.verified();
    (fake, verified)
}

fn view_open(app: &App<Mock>) -> bool {
    app.get_webview(CONSOLE_LABEL).is_some()
}

/// The console tab is in the tab strip.
fn tab_open(app: &App<Mock>) -> bool {
    core(app).console_tab().is_some()
}

fn open(app: &App<Mock>, console: &VerifiedConsole) {
    ConsoleWebview::open(app.handle(), console).unwrap();
}

/// The tabs, as the UI lists them.
fn tab_list(app: &App<Mock>) -> Vec<Value> {
    let list = invoke(app, "tab_list", json!({})).unwrap();
    list.as_array().cloned().unwrap_or_default()
}

fn tab_kinds(app: &App<Mock>) -> Vec<String> {
    tab_list(app)
        .iter()
        .map(|t| t["kind"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// The `tab-*` webviews of the app.
fn tab_webviews(app: &App<Mock>) -> usize {
    app.webviews()
        .keys()
        .filter(|l| l.starts_with("tab-"))
        .count()
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

/// R19: the first load runs through the rule-list callback. On the mock runtime no list is
/// attached, so a fresh view stays on about:blank until the callback runs.
fn first_load(webview: &Webview<Mock>, console: &VerifiedConsole) {
    assert_eq!(webview.url().unwrap().as_str(), "about:blank");
    let url = console.home();
    load_after_rules(webview.clone(), url.clone())(Ok(()));
    assert!(wait_for(|| webview.url().is_ok_and(|u| u == url)), "{url}");
}

#[test]
fn r8_the_label_is_console() {
    // R8: the webview label is exactly `console`.
    assert_eq!(CONSOLE_LABEL, "console");
}

#[test]
fn r8_open_builds_the_console_webview_as_a_child_of_the_main_window() {
    // R8: a child of the `main` window, like the tab webviews.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    let webview = ConsoleWebview::open(app.handle(), &console).unwrap();
    assert_eq!(webview.label(), "console");
    assert_eq!(webview.window().label(), "main");
    assert!(app.get_webview("console").is_some());
}

#[test]
fn r8_there_is_no_other_window() {
    // R8: there is no other console window: the main window is the only window.
    let app = app();
    let (_fake, console) = console(ConsoleKind::I2pd);
    open(&app, &console);
    thread::sleep(Duration::from_millis(300));
    let mut windows: Vec<String> = app.windows().keys().cloned().collect();
    windows.sort();
    assert_eq!(windows, ["main"]);
}

#[test]
fn r8_the_console_is_never_a_tab_webview() {
    // R8: never a tab-* webview, never in the tab-webview label map.
    let app = app();
    let (_fake, console) = console(ConsoleKind::I2pd);
    let webview = ConsoleWebview::open(app.handle(), &console).unwrap();
    assert!(!webview.label().starts_with("tab-"));
    assert_eq!(
        tab_webviews(&app),
        0,
        "opening the console adds no tab webview"
    );
    let id = core(&app).console_tab().expect("the console tab");
    assert_eq!(label(&app, id), None, "the console tab has no tab webview");
}

#[test]
fn r8_opening_twice_reuses_the_one_webview() {
    // R8 and R24: the second open builds no second webview and no second tab.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let (views, tabs) = (app.webviews().len(), core(&app).tabs().len());
    open(&app, &console);
    assert_eq!(app.webviews().len(), views);
    assert_eq!(core(&app).tabs().len(), tabs);
    let consoles = app.webviews().keys().filter(|l| *l == "console").count();
    assert_eq!(consoles, 1);
}

// ---------------------------------------------------------------- R9

#[test]
fn r9_the_view_loads_the_home_page_on_the_detected_origin() {
    // R7 and R9: scheme http, host 127.0.0.1, the detected port, the home page of the router.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let app = app();
        let (fake, console) = console(kind);
        let webview = ConsoleWebview::open(app.handle(), &console).unwrap();
        first_load(&webview, &console);
        let url = webview.url().unwrap();
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port_or_known_default(), Some(fake.port()));
        assert_eq!(url.path(), probe_path(kind), "{kind:?}");
        assert_eq!(url.query(), None, "{kind:?}");
    }
}

#[test]
fn r19_when_the_rule_list_cannot_be_attached_nothing_loads() {
    // R19: fail closed. An Err from the rule list leaves the view on about:blank.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    let webview = ConsoleWebview::open(app.handle(), &console).unwrap();
    assert_eq!(webview.url().unwrap().as_str(), "about:blank");
    load_after_rules(webview.clone(), console.home())(Err("no rule list".to_owned()));
    thread::sleep(Duration::from_millis(500));
    assert_eq!(webview.url().unwrap().as_str(), "about:blank");
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

// ---------------------------------------------------------------- R6, R30: the console tab closes

#[test]
fn r6_the_console_tab_closes_when_the_console_goes_away() {
    // R6 and R30: the console goes away, the console tab closes and the webview is destroyed.
    let app = app();
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found.clone()));
    open(&app, &found);
    assert!(view_open(&app) && tab_open(&app));
    set_console(app.handle(), None);
    assert!(wait_for(|| !view_open(&app)));
    assert!(wait_for(|| !tab_open(&app)));
    assert!(tab_kinds(&app).iter().all(|k| k != "console"));
}

#[test]
fn r6_the_console_tab_closes_when_the_console_moves_to_another_port() {
    // R6 and R30: the console moves, the console tab closes.
    let app = app();
    let (_a, first) = console(ConsoleKind::Java);
    let (_b, second) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(first.clone()));
    open(&app, &first);
    set_console(app.handle(), Some(second));
    assert!(wait_for(|| !view_open(&app)));
    assert!(wait_for(|| !tab_open(&app)));
}

#[test]
fn r6_the_same_console_again_keeps_the_console_tab_open() {
    // R6 and R30: no change, no close.
    let app = app();
    let (_fake, found) = console(ConsoleKind::I2pd);
    set_console(app.handle(), Some(found.clone()));
    open(&app, &found);
    set_console(app.handle(), Some(found));
    thread::sleep(Duration::from_millis(500));
    assert!(view_open(&app));
    assert!(tab_open(&app));
}

#[test]
fn r6_close_closes_the_console_tab_and_is_safe_when_none_is_open() {
    // R30: close() closes the console tab (if open) and destroys the webview (if any).
    let app = app();
    close(app.handle());
    assert_eq!(
        core(&app).tabs().len(),
        1,
        "close with none changes nothing"
    );
    let (_fake, found) = console(ConsoleKind::Java);
    open(&app, &found);
    close(app.handle());
    assert!(wait_for(|| !view_open(&app)));
    assert!(wait_for(|| !tab_open(&app)));
    assert_eq!(core(&app).tabs().len(), 1);
}

// ---------------------------------------------------------------- R12

#[test]
fn r12_with_no_console_it_answers_no_console() {
    // R12: {ok: false, reason: "no-console"}; no tab and no webview are made.
    let app = app();
    let got = invoke(&app, "console_open", json!({})).unwrap();
    assert_eq!(got["ok"], json!(false));
    assert_eq!(got["reason"], json!("no-console"));
    assert!(!view_open(&app));
    assert!(!tab_open(&app));
    assert_eq!(core(&app).tabs().len(), 1);
    assert_eq!(tab_kinds(&app), ["internal"]);
}

#[test]
fn r12_with_a_console_it_opens_the_console_tab_and_answers_ok() {
    // R12: takes no argument, opens the console tab, loads the home page, answers {ok: true}.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let app = app();
        let (fake, found) = console(kind);
        set_console(app.handle(), Some(found));
        let got = invoke(&app, "console_open", json!({})).unwrap();
        assert_eq!(got["ok"], json!(true), "{kind:?}");
        assert!(got.get("reason").is_none_or(Value::is_null), "{got}");
        assert!(wait_for(|| view_open(&app)));
        let id = core(&app).console_tab().expect("the console tab");
        assert_eq!(core(&app).tabs().active_id(), id);
        assert_eq!(core(&app).view(), View::Console(id));
        let view = app.get_webview("console").expect("console webview");
        let found = current(app.handle()).expect("stored console");
        first_load(&view, &found);
        let want = format!("http://127.0.0.1:{}{}", fake.port(), probe_path(kind));
        assert_eq!(view.url().unwrap().as_str(), want, "{kind:?}");
    }
}

#[test]
fn r12_it_never_answers_no_page() {
    // R12: there is no "no-page" answer; every router opens its home page.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let app = app();
        let (_fake, found) = console(kind);
        set_console(app.handle(), Some(found));
        let got = invoke(&app, "console_open", json!({})).unwrap();
        assert_ne!(got["reason"], json!("no-page"), "{kind:?}");
        assert_eq!(got["ok"], json!(true), "{kind:?}");
    }
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
    open(&app, &found);
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

// ---------------------------------------------------------------- R10: navigation guard

/// An app with a console view that finished its first load on the Home page.
fn armed_view(kind: ConsoleKind) -> (App<Mock>, FakeConsole, VerifiedConsole, Webview<Mock>, Url) {
    let app = app();
    let (fake, console) = console(kind);
    let webview = ConsoleWebview::open(app.handle(), &console).unwrap();
    first_load(&webview, &console);
    let home = webview.url().unwrap();
    (app, fake, console, webview, home)
}

fn tab_count(app: &App<Mock>) -> usize {
    core(app).tabs().len()
}

fn url(text: &str) -> Url {
    Url::parse(text).unwrap()
}

fn denied<R: tauri::Runtime>(response: &NewWindowResponse<R>) -> bool {
    matches!(response, NewWindowResponse::Deny)
}

#[test]
fn r10_the_first_about_blank_is_allowed() {
    // R10 and R19: the view starts on about:blank, so the guard lets it through.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    assert!(navigation(app.handle(), &console, &url("about:blank")));
    assert_eq!(tab_count(&app), 1);
}

#[test]
fn r10_same_origin_navigation_stays_in_the_console_view() {
    // R10: a navigation on the console origin is allowed and opens no tab.
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let (app, _fake, console, _webview, _home) = armed_view(kind);
        let origin = console.origin();
        let tabs = tab_count(&app);
        for path in ["/", "/logs", "/config?x=1#top", "/?page=commands"] {
            let target = url(&format!("{origin}{path}"));
            assert!(
                navigation(app.handle(), &console, &target),
                "{kind:?} {target}"
            );
        }
        thread::sleep(Duration::from_millis(300));
        assert_eq!(
            tab_count(&app),
            tabs,
            "{kind:?}: no tab for the same origin"
        );
    }
}

#[test]
fn r10_a_same_origin_new_window_loads_in_the_console_view() {
    // R10: a new-window request on the console origin answers Deny and loads the URL in
    // the console view (through the async apply step).
    let (app, _fake, console, webview, _home) = armed_view(ConsoleKind::Java);
    let tabs = tab_count(&app);
    let target = url(&format!("{}/logs", console.origin()));
    let response = new_window(app.handle(), &console, &target);
    assert!(denied(&response), "the engine never opens a window");
    assert!(
        wait_for(|| webview.url().is_ok_and(|u| u == target)),
        "the console view loads {target}"
    );
    assert_eq!(tab_count(&app), tabs, "no tab for the same origin");
}

#[test]
fn r10_an_i2p_navigation_is_cancelled_and_opens_a_new_tab() {
    // R10: http(s)://*.i2p is cancelled in the console view and opens one new normal tab.
    let (app, _fake, console, webview, home) = armed_view(ConsoleKind::Java);
    for text in i2p_urls() {
        let text = text.as_str();
        let tabs = tab_count(&app);
        assert!(
            !navigation(app.handle(), &console, &url(text)),
            "{text}: cancelled in the console view"
        );
        assert!(
            wait_for(|| tab_count(&app) == tabs + 1),
            "{text}: one new tab"
        );
        thread::sleep(Duration::from_millis(300));
        assert_eq!(tab_count(&app), tabs + 1, "{text}: exactly one tab");
        assert_eq!(webview.url().unwrap(), home, "{text}: the console stays");
    }
}

#[test]
fn r10_an_i2p_new_window_opens_a_new_tab_and_is_denied() {
    // R10: a new-window request for *.i2p opens a new tab; the engine opens no window.
    let (app, _fake, console, webview, home) = armed_view(ConsoleKind::I2pd);
    for text in i2p_urls() {
        let text = text.as_str();
        let tabs = tab_count(&app);
        let response = new_window(app.handle(), &console, &url(text));
        assert!(denied(&response), "{text}");
        assert!(
            wait_for(|| tab_count(&app) == tabs + 1),
            "{text}: one new tab"
        );
        thread::sleep(Duration::from_millis(300));
        assert_eq!(tab_count(&app), tabs + 1, "{text}: exactly one tab");
        assert_eq!(webview.url().unwrap(), home, "{text}: the console stays");
    }
}

/// `*.i2p` URLs the tab guard accepts: a name, and a full 52 character b32 address (the
/// guard refuses a short `.b32.i2p` label such as `y.b32.i2p`, so the table uses a valid one).
fn i2p_urls() -> Vec<String> {
    vec![
        "http://x.i2p/".to_owned(),
        format!("https://{}.b32.i2p/p", "a".repeat(52)),
    ]
}

/// URLs the guard drops: not the console origin and not `*.i2p`.
fn dropped_urls(console: &VerifiedConsole, other: &VerifiedConsole) -> Vec<String> {
    let port = console.port();
    vec![
        format!("http://127.0.0.1:{}/", port.wrapping_add(1)),
        format!("http://localhost:{port}/"),
        format!("https://127.0.0.1:{port}/"),
        format!("{}/", other.origin()),
        "https://example.com/".to_owned(),
        "http://x.i2p.example.com/".to_owned(),
        "ftp://x.i2p/".to_owned(),
        "file:///etc/hosts".to_owned(),
        "data:text/html,hi".to_owned(),
        "javascript:alert(1)".to_owned(),
        "eepview://home".to_owned(),
    ]
}

#[test]
fn r10_anything_else_is_cancelled_and_opens_nothing() {
    // R10: other ports and hosts on loopback, the other console, clearnet and every other
    // scheme: cancelled, no new tab, the console view stays where it is.
    let (app, _fake, console, webview, home) = armed_view(ConsoleKind::Java);
    let (_other_fake, other) = self::console(ConsoleKind::I2pd);
    let tabs = tab_count(&app);
    for text in dropped_urls(&console, &other) {
        assert!(
            !navigation(app.handle(), &console, &url(&text)),
            "{text}: navigation is cancelled"
        );
    }
    thread::sleep(Duration::from_millis(500));
    assert_eq!(tab_count(&app), tabs, "no tab opened");
    assert_eq!(webview.url().unwrap(), home, "the console stays");
}

#[test]
fn r10_a_new_window_for_anything_else_is_dropped() {
    // R10: the same table for new-window requests: Deny, no tab, the console unchanged.
    let (app, _fake, console, webview, home) = armed_view(ConsoleKind::Java);
    let (_other_fake, other) = self::console(ConsoleKind::I2pd);
    let tabs = tab_count(&app);
    for text in dropped_urls(&console, &other) {
        let response = new_window(app.handle(), &console, &url(&text));
        assert!(denied(&response), "{text}");
    }
    thread::sleep(Duration::from_millis(500));
    assert_eq!(tab_count(&app), tabs, "no tab opened");
    assert_eq!(webview.url().unwrap(), home, "the console stays");
}

#[test]
fn r10_an_i2p_link_opens_a_new_normal_tab_and_the_console_tab_stays() {
    // R10: the .i2p URL opens a NEW normal tab; the console tab is neither replaced nor closed.
    let (app, _fake, console, webview, home) = armed_view(ConsoleKind::Java);
    let console_tab = core(&app).console_tab().expect("the console tab");
    assert!(!navigation(app.handle(), &console, &url("http://x.i2p/")));
    assert!(wait_for(|| tab_kinds(&app).iter().any(|k| k == "web")));
    assert_eq!(core(&app).console_tab(), Some(console_tab));
    let kinds = tab_kinds(&app);
    assert_eq!(
        kinds.iter().filter(|k| *k == "console").count(),
        1,
        "{kinds:?}"
    );
    assert_eq!(kinds.iter().filter(|k| *k == "web").count(), 1, "{kinds:?}");
    assert_eq!(webview.url().unwrap(), home, "the console stays");
}

// ---------------------------------------------------------------- R23

#[test]
fn r23_the_console_tab_is_listed_with_kind_console_and_fixed_marks() {
    // R23: kind "console", zoom 1, jsOn true, bookmarked false, icon null.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let list = tab_list(&app);
    let tab = list
        .iter()
        .find(|t| t["kind"] == json!("console"))
        .expect("a console tab in tab_list");
    assert_eq!(tab["zoom"], json!(1.0));
    assert_eq!(tab["jsOn"], json!(true));
    assert_eq!(tab["bookmarked"], json!(false));
    assert_eq!(tab["icon"], json!(null));
    assert_eq!(tab["active"], json!(true));
}

#[test]
fn r23_the_content_area_follows_the_active_tab() {
    // R23: the console view shows only while the console tab is the active tab.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let id = core(&app).console_tab().expect("the console tab");
    assert_eq!(core(&app).view(), View::Console(id));
    let home = core(&app).tabs().iter().next().expect("home tab").id;
    invoke(&app, "tab_select", json!({"id": home})).unwrap();
    assert_ne!(core(&app).view(), View::Console(id));
    assert!(view_open(&app), "the view is hidden, not destroyed");
    invoke(&app, "tab_select", json!({"id": id})).unwrap();
    assert_eq!(core(&app).view(), View::Console(id));
}

// ---------------------------------------------------------------- R24

#[test]
fn r24_open_makes_the_tab_right_after_the_active_tab() {
    // R24: with none open, it makes one right after the active tab and selects it.
    let app = app();
    invoke(&app, "tab_new", json!({})).unwrap();
    invoke(&app, "tab_new", json!({})).unwrap();
    let first = core(&app).tabs().iter().next().expect("tab").id;
    invoke(&app, "tab_select", json!({"id": first})).unwrap();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    assert_eq!(
        tab_kinds(&app),
        ["internal", "console", "internal", "internal"]
    );
    let console_tab = core(&app).console_tab();
    let active = core(&app).tabs().active_id();
    assert_eq!(Some(active), console_tab);
}

#[test]
fn r24_a_second_open_selects_the_tab_and_loads_the_home_page_again() {
    // R24 and R12: with a console tab open, console_open selects it and loads the home page.
    let app = app();
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found.clone()));
    invoke(&app, "console_open", json!({})).unwrap();
    let view = app.get_webview("console").expect("console webview");
    first_load(&view, &found);
    let elsewhere = url(&format!("{}/logs", found.origin()));
    view.navigate(elsewhere.clone()).unwrap();
    assert!(wait_for(|| view.url().is_ok_and(|u| u == elsewhere)));
    invoke(&app, "tab_new", json!({})).unwrap();
    let id = core(&app).console_tab().expect("the console tab");
    assert_ne!(core(&app).tabs().active_id(), id);
    let tabs = core(&app).tabs().len();
    let got = invoke(&app, "console_open", json!({})).unwrap();
    assert_eq!(got["ok"], json!(true));
    assert_eq!(core(&app).tabs().active_id(), id, "selected");
    assert_eq!(core(&app).tabs().len(), tabs, "no second tab");
    assert!(
        wait_for(|| view.url().is_ok_and(|u| u == found.home())),
        "the home page loads again"
    );
    let consoles = app.webviews().keys().filter(|l| *l == "console").count();
    assert_eq!(consoles, 1, "never a second console webview");
}

// ---------------------------------------------------------------- R25

#[test]
fn r25_the_tab_title_is_router_console_until_the_page_title_arrives() {
    // R25: until the first title arrives, it is `Router console`; then the document title.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let title = |app: &App<Mock>| {
        tab_list(app)
            .iter()
            .find(|t| t["kind"] == json!("console"))
            .and_then(|t| t["title"].as_str().map(str::to_owned))
    };
    assert_eq!(title(&app).as_deref(), Some("Router console"));
    title_changed(app.handle(), "I2P Router Console - Home");
    assert!(wait_for(
        || title(&app).as_deref() == Some("I2P Router Console - Home")
    ));
}

#[test]
fn r25_a_title_with_no_console_tab_does_nothing() {
    // R25: no console tab, no state to change.
    let app = app();
    title_changed(app.handle(), "x");
    assert_eq!(tab_kinds(&app), ["internal"]);
}

#[test]
fn r25_main_frame_page_loads_of_the_console_origin_update_the_tab() {
    // R25: the tab URL is the URL the console view shows; loading follows its page loads.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let id = core(&app).console_tab().expect("the console tab");
    let page = url(&format!("{}/tunnels", console.origin()));
    page_load(app.handle(), &console, PageLoadEvent::Started, &page);
    assert!(wait_for(|| {
        core(&app)
            .tab_info(id)
            .is_some_and(|t| t.url == page.as_str() && t.nav.loading)
    }));
    page_load(app.handle(), &console, PageLoadEvent::Finished, &page);
    assert!(wait_for(|| {
        core(&app)
            .tab_info(id)
            .is_some_and(|t| t.url == page.as_str() && !t.nav.loading)
    }));
}

#[test]
fn r25_a_page_load_off_the_console_origin_never_reaches_the_core() {
    // R25: only a URL on the console origin reaches the core; any other URL is ignored.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let id = core(&app).console_tab().expect("the console tab");
    let before = core(&app).tab_info(id).expect("info");
    let port = console.port().wrapping_add(1);
    for text in [
        format!("http://127.0.0.1:{port}/"),
        "http://example.com/".to_owned(),
        "http://stats.i2p/".to_owned(),
        "https://localhost/".to_owned(),
    ] {
        for event in [PageLoadEvent::Started, PageLoadEvent::Finished] {
            page_load(app.handle(), &console, event, &url(&text));
        }
    }
    thread::sleep(Duration::from_millis(400));
    let info = core(&app).tab_info(id).expect("info");
    assert_eq!(info.url, before.url);
    assert_eq!(info.nav.loading, before.nav.loading);
    assert_eq!(tab_webviews(&app), 0);
}

#[test]
fn r25_a_console_page_never_enters_history() {
    // R25: a console page never enters history.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let page = url(&format!("{}/config", console.origin()));
    page_load(app.handle(), &console, PageLoadEvent::Started, &page);
    title_changed(app.handle(), "Config");
    page_load(app.handle(), &console, PageLoadEvent::Finished, &page);
    thread::sleep(Duration::from_millis(400));
    let got = invoke(&app, "history_query", json!({"query": {}})).unwrap();
    assert_eq!(got, json!([]));
}

// ---------------------------------------------------------------- R26

#[test]
fn r26_typing_an_address_makes_a_normal_tab_and_destroys_the_console_view() {
    // R26: the console tab becomes a normal tab, and the `console` webview is destroyed.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let got = invoke(&app, "navigate", json!({"input": "eepview://settings"})).unwrap();
    assert_eq!(got["ok"], json!(true));
    assert!(wait_for(|| !view_open(&app)));
    assert!(!tab_open(&app));
    assert!(tab_kinds(&app).iter().all(|k| k != "console"));
    assert_eq!(tab_kinds(&app).len(), 2, "the tab stays, as a normal tab");
}

#[test]
fn r26_a_typed_console_address_is_refused_and_never_reaches_a_tab_webview() {
    // R26 and R29: refused with `not-i2p`; only console_open() loads the console.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let got = invoke(&app, "navigate", json!({"input": console.home().as_str()})).unwrap();
    assert_eq!(got["ok"], json!(false));
    assert_eq!(got["reason"], json!("not-i2p"));
    assert_eq!(
        tab_webviews(&app),
        0,
        "no tab webview for a loopback address"
    );
}

// ---------------------------------------------------------------- R27

#[test]
fn r27_back_forward_reload_and_stop_keep_the_console_view() {
    // R27: the commands act on the `console` webview and make no tab webview.
    let (app, _fake, _console, webview, _home) = armed_view(ConsoleKind::Java);
    for (cmd, args) in [
        ("go_back", json!({})),
        ("go_forward", json!({})),
        ("reload", json!({})),
        ("reload", json!({"hard": true})),
        ("stop", json!({})),
    ] {
        invoke(&app, cmd, args).unwrap_or_else(|e| panic!("{cmd}: {e}"));
    }
    thread::sleep(Duration::from_millis(300));
    assert!(view_open(&app));
    assert!(webview.url().is_ok());
    assert_eq!(tab_webviews(&app), 0);
    assert!(tab_open(&app));
}

#[test]
fn r27_run_carries_out_the_engine_ops_on_the_console_view() {
    // R27: `run` does a ConsoleOp::Engine on the console webview; it is safe with none.
    let app = app();
    for op in [
        EngineOp::Back,
        EngineOp::Forward,
        EngineOp::Reload,
        EngineOp::HardReload,
        EngineOp::Stop,
    ] {
        run(app.handle(), &ConsoleOp::Engine(op));
    }
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    for op in [
        EngineOp::Back,
        EngineOp::Forward,
        EngineOp::Reload,
        EngineOp::HardReload,
        EngineOp::Stop,
    ] {
        run(app.handle(), &ConsoleOp::Engine(op));
    }
    thread::sleep(Duration::from_millis(300));
    assert!(view_open(&app), "an engine op never destroys the view");
}

// ---------------------------------------------------------------- R28, R30

#[test]
fn r28_closing_the_console_tab_destroys_the_console_webview() {
    // R28: closing the console tab destroys the `console` webview.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let id = core(&app).console_tab().expect("the console tab");
    invoke(&app, "tab_close", json!({"id": id})).unwrap();
    assert!(wait_for(|| !view_open(&app)));
    assert!(!tab_open(&app));
    assert!(tab_kinds(&app).iter().all(|k| k != "console"));
}

#[test]
fn r28_closing_another_tab_keeps_the_console_webview() {
    // R28: only closing the console tab destroys it.
    let app = app();
    let (_fake, console) = console(ConsoleKind::Java);
    open(&app, &console);
    let other = invoke(&app, "tab_new", json!({})).unwrap()["id"]
        .as_u64()
        .unwrap_or(0);
    invoke(&app, "tab_close", json!({"id": other})).unwrap();
    thread::sleep(Duration::from_millis(300));
    assert!(view_open(&app));
    assert!(tab_open(&app));
}

#[test]
fn r28_run_close_destroys_the_console_webview_and_is_safe_with_none() {
    // R28 and R30: ConsoleOp::Close destroys the `console` webview, if any.
    let app = app();
    run(app.handle(), &ConsoleOp::Close);
    let (_fake, console) = console(ConsoleKind::I2pd);
    open(&app, &console);
    assert!(view_open(&app));
    run(app.handle(), &ConsoleOp::Close);
    assert!(wait_for(|| !view_open(&app)));
}

#[test]
fn r28_after_the_console_tab_closed_open_makes_a_fresh_one() {
    // R24 and R28: a closed console tab leaves nothing behind; open builds a fresh view.
    let app = app();
    let (_fake, found) = console(ConsoleKind::Java);
    set_console(app.handle(), Some(found));
    invoke(&app, "console_open", json!({})).unwrap();
    let id = core(&app).console_tab().expect("the console tab");
    invoke(&app, "tab_close", json!({"id": id})).unwrap();
    assert!(wait_for(|| !view_open(&app)));
    let got = invoke(&app, "console_open", json!({})).unwrap();
    assert_eq!(got["ok"], json!(true));
    assert!(wait_for(|| view_open(&app)));
    assert!(tab_open(&app));
    let consoles = tab_kinds(&app).iter().filter(|k| *k == "console").count();
    assert_eq!(consoles, 1);
}

// ---------------------------------------------------------------- R29

#[test]
fn r29_the_console_never_becomes_a_tab_webview() {
    // R29: a console page URL never reaches a tab-* webview, whatever the tab does.
    let (app, _fake, console, _webview, _home) = armed_view(ConsoleKind::Java);
    invoke(&app, "tab_new", json!({})).unwrap();
    invoke(&app, "navigate", json!({"input": console.origin()})).unwrap();
    assert!(!navigation(
        app.handle(),
        &console,
        &url("http://example.com/")
    ));
    assert_eq!(tab_webviews(&app), 0);
    let ids: Vec<u32> = core(&app).tabs().iter().map(|t| t.id).collect();
    for id in ids {
        assert_eq!(label(&app, id), None, "tab {id}");
    }
}
