// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Effect tests on the Tauri mock runtime: events, tab webviews and engine calls.

use std::time::Duration;

use super::*;
use crate::core::EngineOp;
use crate::hover::HoverText;
use crate::net::testing::FakeRouter;
use crate::shell::testing::{app, bare, core, label, load, open_gate};
use crate::types::{FindResult, Toast};

fn events() -> Vec<Event> {
    vec![
        Event::TabsChanged,
        Event::TabUpdated(1),
        Event::Find(FindResult {
            query: "x".into(),
            matches: Some(1),
            active: Some(1),
        }),
        Event::Router,
        Event::Bookmarks,
        Event::History,
        Event::Settings,
        Event::Shortcut("focus-address"),
        Event::Toast(Toast {
            kind: "info",
            text: "hi".into(),
        }),
        Event::Hover(Some(HoverText {
            text: "http://a.i2p/".into(),
            blocked: false,
        })),
        Event::Hover(None),
    ]
}

#[test]
fn every_event_has_its_contract_name() {
    let core = Core::new(None, "127.0.0.1:4444", 0);
    let names: Vec<&str> = events().iter().map(|e| payload(&core, e).0).collect();
    assert_eq!(
        names,
        [
            "tabs-changed",
            "tab-updated",
            "find-result",
            "router-status",
            "bookmarks-changed",
            "history-changed",
            "settings-changed",
            "shortcut",
            "toast",
            "link-hover",
            "link-hover",
        ]
    );
    let hidden = payload(&core, &Event::Hover(None)).1;
    assert_eq!(hidden, json!({ "text": "", "blocked": false }));
    let shortcut = payload(&core, &Event::Shortcut("x")).1;
    assert_eq!(shortcut, json!({ "action": "x" }));
}

#[test]
fn events_reach_the_bundled_webviews() {
    for app in [app(), bare()] {
        apply(
            app.handle(),
            events().into_iter().map(Effect::Emit).collect(),
        );
        apply(app.handle(), vec![Effect::FocusToolbar, Effect::Layout]);
    }
}

#[test]
fn with_core_runs_the_effects() {
    let app = app();
    with_core(app.handle(), |core| {
        core.tab_new(None, crate::tabs::Place::End).1
    });
    later(app.handle(), Vec::new());
    assert_eq!(core(&app).tabs().len(), 2);
}

#[test]
fn hover_shows_after_the_delay() {
    let app = app();
    apply(app.handle(), vec![Effect::HoverLater(0)]);
    thread::sleep(Duration::from_millis(SHOW_DELAY_MS * 3));
}

#[test]
fn no_tab_webview_without_a_gatekeeper() {
    let app = app();
    apply(
        app.handle(),
        vec![Effect::Web(WebOp::Load(load(1, "http://a.i2p/")))],
    );
    assert_eq!(label(&app, 1), None);
}

#[test]
fn no_tab_webview_without_the_window() {
    let app = bare();
    let router = FakeRouter::start();
    open_gate(&app, &router);
    apply(
        app.handle(),
        vec![Effect::Web(WebOp::Load(load(1, "http://a.i2p/")))],
    );
    assert_eq!(label(&app, 1), None);
}

#[test]
fn tab_webviews_are_built_reused_and_rebuilt() {
    let app = app();
    let router = FakeRouter::start();
    open_gate(&app, &router);
    let web = |l| apply(app.handle(), vec![Effect::Web(WebOp::Load(l))]);
    web(load(1, "http://a.i2p/"));
    let first = label(&app, 1).unwrap();
    assert!(tab_webview(app.handle(), 1).is_some());
    web(load(1, "http://b.i2p/"));
    web(load(1, "not a url"));
    assert_eq!(label(&app, 1).as_ref(), Some(&first));
    let mut no_js = load(1, "http://b.i2p/");
    no_js.rebuild = true;
    no_js.js = false;
    web(no_js);
    let second = label(&app, 1).unwrap();
    assert_ne!(first, second);
    assert!(app.get_webview(&second).is_some());
}

#[test]
fn a_url_that_is_not_i2p_gets_no_webview() {
    let app = app();
    let router = FakeRouter::start();
    open_gate(&app, &router);
    let mut bad = load(1, "https://example.com/");
    bad.rebuild = true;
    apply(app.handle(), vec![Effect::Web(WebOp::Load(bad))]);
    assert_eq!(label(&app, 1), None);
}

#[test]
fn engine_calls_and_destroy() {
    let app = app();
    let router = FakeRouter::start();
    open_gate(&app, &router);
    let ops = vec![
        Effect::Web(WebOp::Load(load(1, "http://a.i2p/"))),
        Effect::Web(WebOp::Engine(1, EngineOp::Reload)),
        Effect::Web(WebOp::Engine(9, EngineOp::Reload)),
        Effect::Web(WebOp::Destroy(1)),
        Effect::Web(WebOp::Destroy(9)),
    ];
    apply(app.handle(), ops);
    assert_eq!(label(&app, 1), None);
    assert!(tab_webview(app.handle(), 1).is_none());
}
