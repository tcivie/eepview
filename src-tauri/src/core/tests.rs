// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Core state-machine tests: commands and engine events in, effects out.

use super::find::Zoom;
use super::*;
use crate::session::Step;
use crate::shortcuts::Action;
use crate::store::testdir;
use crate::tabs::Place;
use crate::types::{HistoryQuery, NavResult, NewBookmark};
use serde_json::json;
use tauri::Url;

const STATS: &str = "http://stats.i2p/";
const REG: &str = "http://reg.i2p/";

fn ok_status() -> RouterStatus {
    RouterStatus {
        state: "ok",
        proxy: "127.0.0.1:4444".into(),
        version: None,
        detail: None,
        paused: false,
        managed: false,
    }
}

fn down_status() -> RouterStatus {
    RouterStatus {
        state: "down",
        ..ok_status()
    }
}

fn core() -> Core {
    let mut c = Core::new(None, "127.0.0.1:4444", 0);
    c.router_changed(ok_status());
    c
}

fn loads(fx: &[Effect]) -> Vec<&Load> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Web(WebOp::Load(l)) => Some(l),
            _ => None,
        })
        .collect()
}

fn engine_ops(fx: &[Effect]) -> Vec<&EngineOp> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Web(WebOp::Engine(_, op)) => Some(op),
            _ => None,
        })
        .collect()
}

fn has(fx: &[Effect], want: &Effect) -> bool {
    fx.contains(want)
}

/// Navigates the active tab and plays the engine load events.
fn visit(c: &mut Core, url: &str) {
    c.navigate(url);
    let id = c.tabs().active_id();
    c.page_started(id, url);
    c.page_finished(id, url, 1);
}

#[test]
fn starts_with_a_home_tab_and_no_webview() {
    let c = Core::new(None, "p", 0);
    assert_eq!(c.tab_infos().len(), 1);
    assert_eq!(c.view(), View::Internal("eepview://home".into()));
    assert_eq!(c.router().state, "verifying");
    assert_eq!(c.home_url(), "eepview://home");
    assert!(c.settings().js_default);
    assert_eq!(internal_title("eepview://router-down?x"), "Router down");
    assert_eq!(internal_title(""), "");
}

#[test]
fn web_navigation_waits_for_the_router() {
    let mut c = Core::new(None, "p", 0);
    let (res, fx) = c.navigate("stats.i2p");
    assert_eq!(res, NavResult::refused("router-down"));
    assert!(loads(&fx).is_empty());
    assert!(matches!(c.view(), View::Internal(u) if u.starts_with("eepview://router-down?url=")));
    let fx = c.router_changed(ok_status());
    let l = loads(&fx);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].url, STATS);
    assert!(l[0].js && l[0].private && !l[0].rebuild);
    assert_eq!(c.view(), View::Web(1));
}

#[test]
fn navigate_classifies_input() {
    let mut c = core();
    assert_eq!(c.navigate("stats.i2p").0, NavResult::ok());
    assert_eq!(
        c.navigate("http://example.com").0,
        NavResult::refused("not-i2p")
    );
    assert!(
        c.tabs()
            .active()
            .unwrap()
            .url
            .starts_with("eepview://blocked?url=http")
    );
    assert_eq!(c.navigate("   ").0, NavResult::refused("invalid"));
    c.navigate("forum search");
    assert_eq!(
        c.tabs().active().unwrap().url,
        "eepview://history?q=forum+search"
    );
    c.navigate("eepview://settings");
    assert_eq!(c.tab_info(1).unwrap().title, "Settings");
    assert_eq!(c.tab_info(1).unwrap().kind, "internal");
}

#[test]
fn page_events_update_tab_and_history() {
    let mut c = core();
    visit(&mut c, STATS);
    c.title_changed(1, "Stats");
    let info = c.tab_info(1).unwrap();
    assert_eq!(info.title, "Stats");
    assert!(!info.nav.loading);
    assert!(info.nav.can_back);
    let h = c.history_query(&HistoryQuery::default());
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].title, "Stats");
    assert!(c.page_started(1, "about:blank").is_empty());
    assert!(c.page_finished(1, "about:blank", 2).is_empty());
    assert!(c.page_started(99, STATS).is_empty());
    assert!(c.title_changed(99, "x").is_empty());
}

#[test]
fn history_off_records_nothing() {
    let mut c = core();
    c.settings_set(&json!({"history": {"enabled": false}}))
        .unwrap();
    visit(&mut c, STATS);
    assert!(c.history_query(&HistoryQuery::default()).is_empty());
}

#[test]
fn back_and_forward() {
    let mut c = core();
    visit(&mut c, STATS);
    visit(&mut c, REG);
    let fx = c.step(Step::Back);
    assert_eq!(engine_ops(&fx), [&EngineOp::Back]);
    c.page_started(1, STATS);
    assert_eq!(c.tabs().active().unwrap().url, STATS);
    let fx = c.step(Step::Forward);
    assert_eq!(engine_ops(&fx), [&EngineOp::Forward]);
    c.page_started(1, REG);
    // Back twice: the second step lands on the home page, shown directly.
    c.step(Step::Back);
    c.page_started(1, STATS);
    let fx = c.step(Step::Back);
    assert!(has(&fx, &Effect::Layout));
    assert_eq!(c.view(), View::Internal("eepview://home".into()));
    // Forward again reveals the live webview.
    c.step(Step::Forward);
    assert_eq!(c.view(), View::Web(1));
    assert_eq!(c.tab_info(1).unwrap().url, STATS);
}

#[test]
fn reload_stop_home() {
    let mut c = core();
    assert_eq!(c.reload(false), vec![Effect::Layout]);
    assert!(c.stop().is_empty());
    visit(&mut c, STATS);
    assert_eq!(engine_ops(&c.reload(true)), [&EngineOp::HardReload]);
    assert_eq!(engine_ops(&c.stop()), [&EngineOp::Stop]);
    assert!(c.stop().is_empty());
    c.home();
    assert_eq!(c.tabs().active().unwrap().url, "eepview://home");
}

#[test]
fn tabs_open_close_and_lazy_load() {
    let mut c = core();
    let (info, fx) = c.tab_new(Some(STATS), Place::End);
    let info = info.unwrap();
    assert_eq!(info.id, 2);
    assert!(info.marks.active);
    assert_eq!(loads(&fx).len(), 1);
    c.tab_select(1);
    assert!(c.tab_select(1).is_empty());
    assert!(c.tab_select(42).is_empty());
}

#[test]
fn tabs_close_reopen_and_order() {
    let mut c = core();
    c.tab_new(Some(STATS), Place::End);
    c.tab_select(1);
    let fx = c.tab_close(2);
    assert!(has(&fx, &Effect::Web(WebOp::Destroy(2))));
    assert!(c.tab_close(2).is_empty());
    let fx = c.tab_reopen();
    assert_eq!(loads(&fx).len(), 1);
    assert!(c.tab_move(1, 5).contains(&Effect::Emit(Event::TabsChanged)));
    assert!(c.tab_move(77, 0).is_empty());
    c.tab_cycle(true);
    c.tab_number(1);
    assert_eq!(c.tabs().active_id(), c.tabs().iter().next().unwrap().id);
    c.tab_close(1);
    c.tab_close(3);
    assert_eq!(c.tab_infos().len(), 1);
    assert_eq!(c.tab_infos()[0].url, "eepview://home");
    assert!(c.tab_reopen().iter().any(|e| matches!(e, Effect::Layout)));
}

#[test]
fn router_down_destroys_tab_webviews() {
    let mut c = core();
    visit(&mut c, STATS);
    let fx = c.router_changed(down_status());
    assert!(has(&fx, &Effect::Web(WebOp::Destroy(1))));
    assert!(matches!(c.view(), View::Internal(u) if u.contains("state=down")));
    assert!(c.reload(false).is_empty());
    let fx = c.router_changed(ok_status());
    assert_eq!(loads(&fx)[0].url, STATS);
}

#[test]
fn navigation_guard_in_tabs() {
    let mut c = core();
    visit(&mut c, STATS);
    let ok = Url::parse("http://reg.i2p/x").unwrap();
    assert!(c.tab_navigation(1, &ok).0);
    let bad = Url::parse("http://127.0.0.1:7657/").unwrap();
    let (allowed, _) = c.tab_navigation(1, &bad);
    assert!(!allowed);
    assert!(
        c.tabs()
            .active()
            .unwrap()
            .url
            .starts_with("eepview://blocked")
    );
    visit(&mut c, STATS);
    c.page_started(1, STATS);
    let (allowed, fx) = c.tab_navigation(1, &Url::parse("javascript:alert(1)").unwrap());
    assert!(!allowed);
    assert!(matches!(&fx[0], Effect::Emit(Event::Toast(t)) if t.text == "Blocked: javascript:"));
}

#[test]
fn new_windows_become_tabs() {
    let mut c = core();
    let fx = c.new_window(&Url::parse(REG).unwrap());
    assert_eq!(loads(&fx)[0].url, REG);
    assert_eq!(c.tab_infos().len(), 2);
    let fx = c.new_window(&Url::parse("http://example.com/").unwrap());
    assert!(matches!(&fx[0], Effect::Emit(Event::Toast(_))));
    assert_eq!(c.tab_infos().len(), 2);
    assert_eq!(Core::download_refused("x").len(), 1);
}

#[test]
fn per_site_js_rebuilds_the_webview() {
    let mut c = core();
    visit(&mut c, STATS);
    let fx = c.site_js_set("stats.i2p", false);
    let l = loads(&fx);
    assert!(l[0].rebuild && !l[0].js);
    assert!(!c.tab_info(1).unwrap().marks.js_on);
    // Navigating within the webview to a host with another choice rebuilds it again.
    let fx = c.page_started(1, REG);
    assert!(loads(&fx)[0].rebuild);
    let fx = c.settings_set(&json!({"jsDefault": false})).unwrap().1;
    assert!(loads(&fx)[0].rebuild);
}

#[test]
fn zoom_per_host() {
    let mut c = core();
    assert!(c.zoom(Zoom::In).is_empty());
    visit(&mut c, STATS);
    let fx = c.zoom(Zoom::In);
    assert_eq!(engine_ops(&fx), [&EngineOp::Zoom(1.1)]);
    assert!((c.tab_info(1).unwrap().zoom - 1.1).abs() < 1e-9);
    c.zoom(Zoom::Out);
    c.zoom(Zoom::Out);
    assert!((c.tab_info(1).unwrap().zoom - 0.9).abs() < 1e-9);
    c.zoom(Zoom::Reset);
    assert!((c.tab_info(1).unwrap().zoom - 1.0).abs() < 1e-9);
}

#[test]
fn find_flow() {
    let mut c = core();
    let fx = c.find("x", true, false);
    assert!(matches!(&fx[0], Effect::Emit(Event::Find(r)) if r.matches.is_none()));
    assert!(c.find_open());
}

#[test]
fn find_counts_and_cycles() {
    let mut c = core();
    visit(&mut c, STATS);
    let fx = c.find("i2p", true, false);
    assert!(matches!(engine_ops(&fx)[0], EngineOp::Find(op) if op.fresh));
    let fx = c.find_counted(1, Some(3));
    let want = FindResult {
        query: "i2p".into(),
        matches: Some(3),
        active: Some(1),
    };
    assert_eq!(fx, vec![Effect::Emit(Event::Find(want))]);
    let fx = c.find("i2p", false, false);
    assert!(
        fx.iter()
            .any(|e| matches!(e, Effect::Emit(Event::Find(r)) if r.active == Some(3)))
    );
}

#[test]
fn find_without_matches_and_close() {
    let mut c = core();
    visit(&mut c, STATS);
    c.find("i2p", true, false);
    assert!(c.find_counted(9, Some(1)).is_empty());
    let fx = c.find_counted(1, Some(0));
    assert!(matches!(&fx[0], Effect::Emit(Event::Find(r)) if r.active.is_none()));
    assert!(
        c.find("i2p", true, false)
            .iter()
            .all(|e| !matches!(e, Effect::Emit(_)))
    );
    let fx = c.find_close();
    assert!(has(
        &fx,
        &Effect::Web(WebOp::Engine(1, EngineOp::FindClear))
    ));
    assert!(!c.find_open());
}

#[test]
fn bookmarks_through_the_core() {
    let mut c = core();
    assert_eq!(c.bookmarks_list().len(), 4);
    visit(&mut c, "http://new.i2p/");
    let fx = c.bookmark_toggle(5);
    assert!(has(&fx, &Effect::Emit(Event::Bookmarks)));
    assert!(c.tab_info(1).unwrap().marks.bookmarked);
    c.bookmark_toggle(6);
    assert!(!c.tab_info(1).unwrap().marks.bookmarked);
}

#[test]
fn bookmark_commands() {
    let mut c = core();
    let new = NewBookmark {
        url: "x.i2p".into(),
        title: "X".into(),
        folder: None,
    };
    let (mut b, _) = c.bookmark_add(&new, 7).unwrap();
    b.title = "Y".into();
    assert!(!c.bookmark_update(&b).is_empty());
    assert_eq!(c.bookmark_find("x.i2p").unwrap().title, "Y");
    assert!(!c.bookmark_remove(&b.id).is_empty());
    assert!(c.bookmark_remove(&b.id).is_empty());
    let exported = c.bookmarks_export();
    assert_eq!(c.bookmarks_import(&exported, 8).unwrap().0, 0);
    assert!(c.bookmarks_import("nope", 8).is_err());
}

#[test]
fn history_and_suggest() {
    let mut c = core();
    visit(&mut c, STATS);
    assert_eq!(c.suggest("stat", 1)[0].url, STATS);
    let id = c.history_query(&HistoryQuery::default())[0].id.clone();
    assert!(!c.history_remove(&id).is_empty());
    assert!(c.history_remove(&id).is_empty());
    assert!(c.history_clear("all", 2).is_ok());
    assert!(c.history_clear("decade", 2).is_err());
}

#[test]
fn settings_round_trip() {
    let mut c = core();
    let (s, fx) = c.settings_set(&json!({"theme": "dark"})).unwrap();
    assert_eq!(s.theme, crate::store::settings::Theme::Dark);
    assert!(has(&fx, &Effect::Emit(Event::Settings)));
    assert!(c.settings_set(&json!({"homepage": "evil.com"})).is_err());
}

#[test]
fn shortcuts_dispatch() {
    let mut c = core();
    let fx = c.shortcut(Action::FocusAddress, 0);
    assert_eq!(
        fx,
        vec![
            Effect::Emit(Event::Shortcut("focus-address")),
            Effect::FocusToolbar
        ]
    );
    assert!(
        c.shortcut(Action::FindNext, 0)
            .contains(&Effect::Emit(Event::Shortcut("open-find")))
    );
    for action in [
        Action::NewTab,
        Action::NextTab,
        Action::PrevTab,
        Action::SelectTab(1),
        Action::Bookmarks,
        Action::History,
        Action::Settings,
        Action::Home,
        Action::Back,
        Action::Forward,
        Action::Reload,
        Action::HardReload,
        Action::Stop,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomReset,
        Action::Bookmark,
        Action::FindPrev,
        Action::ReopenTab,
        Action::CloseTab,
    ] {
        c.shortcut(action, 0);
    }
    assert!(!c.tab_infos().is_empty());
}

#[test]
fn hover_bubble() {
    let mut c = core();
    visit(&mut c, STATS);
    let fx = c.hover_link(1, Some("http://reg.i2p/a%20b"));
    assert!(
        matches!(&fx[0], Effect::Emit(Event::Hover(Some(t))) if t.text == "http://reg.i2p/a b")
    );
    assert!(c.hover_link(2, Some(REG)).is_empty());
    let fx = c.hover_link(1, None);
    let Effect::HoverLater(generation) = fx[0] else {
        panic!("{fx:?}")
    };
    assert_eq!(
        c.hover_expire(generation),
        vec![Effect::Emit(Event::Hover(None))]
    );
    assert!(c.hover_expire(generation).is_empty());
    assert!(c.hover_link(1, None).is_empty());
    let fx = c.page_started(1, REG);
    assert!(
        fx.iter().any(
            |e| matches!(e, Effect::Emit(Event::Hover(Some(t))) if t.text == "Loading reg.i2p…")
        )
    );
    let fx = c.tab_new(None, Place::End).1;
    assert!(has(&fx, &Effect::Emit(Event::Hover(None))));
}

#[test]
fn internal_page_links() {
    let mut c = core();
    let url = Url::parse("tauri://localhost/src/ui/bookmarks.html").unwrap();
    c.internal_loaded(&url);
    assert_eq!(c.tabs().active().unwrap().url, "eepview://bookmarks");
    assert!(c.internal_loaded(&url).is_empty());
    assert!(
        c.internal_loaded(&Url::parse("tauri://localhost/x.html").unwrap())
            .is_empty()
    );
}

#[test]
fn toolbar_requests() {
    let mut c = core();
    assert_eq!(c.set_toolbar_request(300.0), vec![Effect::Layout]);
    assert!((c.toolbar_request() - 300.0).abs() < 1e-9);
    c.set_toolbar_request(f64::NAN);
    assert!(c.toolbar_request().abs() < 1e-9);
}

#[test]
fn stores_persist_through_paths() {
    let dir = testdir::fresh("core");
    let paths = Paths {
        bookmarks: dir.join("bookmarks.json"),
        history: dir.join("history.json"),
        settings: dir.join("settings.json"),
        sites: dir.join("sites.json"),
    };
    let mut c = Core::new(Some(paths.clone()), "p", 0);
    c.router_changed(ok_status());
    visit(&mut c, "http://new.i2p/");
    c.bookmark_toggle(1);
    c.zoom(Zoom::In);
    c.settings_set(&json!({"keepCookies": true})).unwrap();
    let back = Core::new(Some(paths), "p", 0);
    assert_eq!(back.history_query(&HistoryQuery::default()).len(), 1);
    assert_eq!(back.bookmarks_list().len(), 5);
    assert!(back.settings().keep_cookies);
}

#[test]
fn save_errors_become_toasts() {
    let dir = testdir::fresh("core-bad");
    std::fs::write(dir.join("file"), "x").unwrap();
    let blocked = dir.join("file").join("x.json");
    let paths = Paths {
        bookmarks: blocked.clone(),
        history: blocked.clone(),
        settings: blocked.clone(),
        sites: blocked,
    };
    let mut c = Core::new(Some(paths), "p", 0);
    let fx = c.settings_set(&json!({"theme": "light"})).unwrap().1;
    assert!(
        matches!(&fx[0], Effect::Emit(Event::Toast(t)) if t.text.starts_with("Could not save"))
    );
}

#[test]
fn verdicts_map_to_states() {
    use crate::core::router::status_of;
    use crate::net::verify::Verdict;
    let cases = [
        (Verdict::Down("x".into()), true, "down"),
        (Verdict::NotI2p("x".into()), true, "not-i2p"),
    ];
    for (verdict, gate, state) in cases {
        assert_eq!(status_of(&verdict, "p", gate).state, state);
    }
}

#[test]
fn js_can_be_forced_off() {
    let mut c = core();
    c.force_js_off();
    c.site_js_set("stats.i2p", true);
    let fx = c.navigate(STATS).1;
    assert!(!loads(&fx)[0].js);
}

#[test]
fn pause_destroys_webviews_and_refuses_loads() {
    let mut c = core();
    visit(&mut c, STATS);
    let fx = c.pause();
    assert!(has(&fx, &Effect::Web(WebOp::Destroy(1))));
    assert!(c.paused() && !c.router().is_ok());
    assert!(matches!(c.view(), View::Internal(u) if u.contains("reason=paused")));
    assert!(c.pause().is_empty());
    let (res, fx) = c.navigate(REG);
    assert_eq!(res, NavResult::refused("router-down"));
    assert!(loads(&fx).is_empty());
}

#[test]
fn verify_while_paused_keeps_the_gate_closed() {
    let mut c = core();
    visit(&mut c, STATS);
    c.pause();
    let fx = c.router_changed(ok_status());
    assert!(loads(&fx).is_empty());
    assert!(c.paused() && !c.router().is_ok());
}

#[test]
fn resume_verifies_then_reloads() {
    let mut c = core();
    visit(&mut c, STATS);
    c.pause();
    assert!(!c.resume().is_empty());
    assert!(!c.paused());
    assert_eq!(c.router().state, "verifying");
    assert!(c.resume().is_empty());
    let l = c.router_changed(ok_status());
    assert_eq!(loads(&l).len(), 1);
    assert_eq!(c.view(), View::Web(1));
}

#[test]
fn resume_with_a_failed_verify_stays_closed() {
    let mut c = core();
    visit(&mut c, STATS);
    c.pause();
    c.resume();
    let fx = c.router_changed(down_status());
    assert!(loads(&fx).is_empty());
    assert!(!c.router().is_ok());
    assert!(matches!(c.view(), View::Internal(u) if u.starts_with("eepview://router-down")));
}
