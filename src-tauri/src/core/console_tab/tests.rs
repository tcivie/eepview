// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the console tab in the core (`docs/wiki/router-console.md`,
//! R23 to R30): the tab kind, the one console tab, its state, the address bar, back and
//! forward, close, and the rule that the core never makes a `WebOp` for it. They use only
//! the public interface of `Core`: commands and engine events in, effects out.

use serde_json::json;
use tauri::Url;

use crate::core::find::Zoom;
use crate::core::{ConsoleOp, Core, Effect, Event, Paths, View, WebOp};
use crate::session::Step;
use crate::store::testdir;
use crate::tabs::Place;
use crate::types::{HistoryQuery, NavResult, NewBookmark, RouterStatus};

const STATS: &str = "http://stats.i2p/";
const REG: &str = "http://reg.i2p/";
const HOME: &str = "http://127.0.0.1:7657/home";
const TUNNELS: &str = "http://127.0.0.1:7657/tunnels";
const CONFIG: &str = "http://127.0.0.1:7657/config";

// ---------------------------------------------------------------- helpers

fn status(state: &'static str, paused: bool) -> RouterStatus {
    RouterStatus {
        state,
        proxy: "127.0.0.1:4444".into(),
        version: None,
        detail: None,
        paused,
        managed: false,
    }
}

fn core() -> Core {
    let mut c = Core::new(None, "127.0.0.1:4444", 0);
    c.router_changed(status("ok", false));
    c
}

/// A core with the console tab open on the Java home page. Gives the console tab id.
fn with_console() -> (Core, u32) {
    let mut c = core();
    c.console_open(HOME);
    let id = c.console_tab().expect("console_open makes the console tab");
    (c, id)
}

/// The console tab after its first page loaded.
fn loaded() -> (Core, u32) {
    let (mut c, id) = with_console();
    c.console_started(HOME);
    c.console_finished(HOME);
    (c, id)
}

/// Navigates the active tab to a web page and plays the engine load events.
fn visit(c: &mut Core, url: &str) {
    c.navigate(url);
    let id = c.tabs().active_id();
    c.page_started(id, url);
    c.page_finished(id, url, 1);
}

fn ids(c: &Core) -> Vec<u32> {
    c.tab_infos().iter().map(|t| t.id).collect()
}

fn kinds(c: &Core) -> Vec<String> {
    c.tab_infos().iter().map(|t| t.kind.to_string()).collect()
}

fn has(fx: &[Effect], want: &Effect) -> bool {
    fx.contains(want)
}

fn console_ops(fx: &[Effect]) -> Vec<&ConsoleOp> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Console(op) => Some(op),
            _ => None,
        })
        .collect()
}

fn web_ops(fx: &[Effect]) -> Vec<&WebOp> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Web(op) => Some(op),
            _ => None,
        })
        .collect()
}

fn load_urls(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Web(WebOp::Load(l)) => Some(l.url.clone()),
            _ => None,
        })
        .collect()
}

/// True when a `WebOp` of any kind names tab `id`.
fn web_op_for(fx: &[Effect], id: u32) -> bool {
    web_ops(fx).iter().any(|op| match op {
        WebOp::Load(l) => l.tab == id,
        WebOp::Engine(tab, _) | WebOp::Destroy(tab) => *tab == id,
    })
}

fn engine_of(fx: &[Effect]) -> Vec<&ConsoleOp> {
    console_ops(fx)
        .into_iter()
        .filter(|op| **op != ConsoleOp::Close)
        .collect()
}

fn closes(fx: &[Effect]) -> bool {
    console_ops(fx).contains(&&ConsoleOp::Close)
}

// ---------------------------------------------------------------- R23: tab kind

#[test]
fn r23_the_console_tab_has_kind_console_and_fixed_marks() {
    // R23: kind "console", zoom 1, jsOn true, bookmarked false, icon null.
    let (c, id) = with_console();
    let info = c.tab_info(id).expect("console tab info");
    assert_eq!(info.kind, "console");
    assert!((info.zoom - 1.0).abs() < 1e-9);
    assert!(info.marks.js_on);
    assert!(!info.marks.bookmarked);
    assert_eq!(info.icon, None);
    assert_eq!(kinds(&c), ["internal", "console"]);
}

#[test]
fn r23_while_the_console_tab_is_active_the_view_is_the_console() {
    // R23: the content area shows the `console` webview.
    let (c, id) = with_console();
    assert_eq!(c.tabs().active_id(), id);
    assert_eq!(c.view(), View::Console(id));
}

#[test]
fn r23_while_another_tab_is_active_the_view_is_that_tab() {
    // R23: the `console` webview is hidden while another tab is active.
    let (mut c, id) = with_console();
    let home = ids(&c)[0];
    let fx = c.tab_select(home);
    assert!(has(&fx, &Effect::Layout), "the view changes: {fx:?}");
    assert_eq!(c.view(), View::Internal("eepview://home".into()));
    let fx = c.tab_select(id);
    assert!(has(&fx, &Effect::Layout), "the view changes back: {fx:?}");
    assert_eq!(c.view(), View::Console(id));
}

#[test]
fn r23_a_web_tab_next_to_the_console_tab_keeps_its_own_view() {
    // R23: the console tab does not take the place of a web tab.
    let mut c = core();
    visit(&mut c, STATS);
    let web = c.tabs().active_id();
    c.console_open(HOME);
    let console = c.console_tab().expect("console tab");
    assert_ne!(web, console);
    c.tab_select(web);
    assert_eq!(c.view(), View::Web(web));
    c.tab_select(console);
    assert_eq!(c.view(), View::Console(console));
}

#[test]
fn r23_cycling_to_the_console_tab_shows_the_console() {
    // R23: next and previous tab reach the console tab like any tab.
    let (mut c, id) = with_console();
    let home = ids(&c)[0];
    c.tab_select(home);
    c.tab_cycle(true);
    assert_eq!(c.tabs().active_id(), id);
    assert_eq!(c.view(), View::Console(id));
}

#[test]
fn r23_a_router_state_change_does_not_touch_the_console_tab() {
    // R23: verifying, down and not-i2p do not touch the console tab: it is on loopback.
    let (mut c, id) = with_console();
    for next in [
        status("down", false),
        status("verifying", false),
        status("not-i2p", false),
        status("ok", false),
    ] {
        let fx = c.router_changed(next);
        assert_eq!(c.view(), View::Console(id));
        assert_eq!(c.console_tab(), Some(id));
        assert!(
            !closes(&fx),
            "the router state never closes the console tab"
        );
        assert!(!web_op_for(&fx, id), "{fx:?}");
    }
}

#[test]
fn r23_pause_and_resume_do_not_touch_the_console_tab() {
    // R23: paused does not change what the console tab shows.
    let (mut c, id) = with_console();
    let fx = c.pause();
    assert_eq!(c.view(), View::Console(id));
    assert_eq!(c.console_tab(), Some(id));
    assert!(!closes(&fx));
    assert!(!web_op_for(&fx, id), "{fx:?}");
    let fx = c.resume();
    assert_eq!(c.view(), View::Console(id));
    assert!(!closes(&fx));
    assert!(!web_op_for(&fx, id), "{fx:?}");
}

// ---------------------------------------------------------------- R24: one console tab

#[test]
fn r24_with_no_console_tab_open_makes_one_and_selects_it() {
    // R24: console_open() with none makes one and selects it.
    let mut c = core();
    assert_eq!(c.console_tab(), None);
    let fx = c.console_open(HOME);
    let id = c.console_tab().expect("a console tab");
    assert_eq!(c.tabs().active_id(), id);
    assert!(has(&fx, &Effect::Emit(Event::TabsChanged)), "{fx:?}");
    let info = c.tab_info(id).expect("info");
    assert_eq!(info.url, HOME);
    assert!(info.marks.active);
}

#[test]
fn r24_the_console_tab_comes_right_after_the_active_tab() {
    // R24: right after the active tab.
    let mut c = core();
    c.tab_new(None, Place::End);
    c.tab_new(None, Place::End);
    let before = ids(&c);
    assert_eq!(before.len(), 3);
    c.tab_select(before[0]);
    c.console_open(HOME);
    let id = c.console_tab().expect("a console tab");
    assert_eq!(ids(&c), [before[0], id, before[1], before[2]]);
    // And after the last tab when that one is active.
    let (mut c, id) = with_console();
    c.tab_close(id);
    c.tab_new(None, Place::End);
    let last = *ids(&c).last().expect("tabs");
    c.tab_select(last);
    c.console_open(HOME);
    assert_eq!(ids(&c).last().copied(), c.console_tab());
}

#[test]
fn r24_a_second_open_selects_the_console_tab_and_makes_no_second_one() {
    // R24: at most one console tab; a second open selects it.
    let (mut c, id) = with_console();
    c.tab_new(None, Place::End);
    assert_ne!(c.tabs().active_id(), id);
    let count = c.tab_infos().len();
    let fx = c.console_open(HOME);
    assert_eq!(c.console_tab(), Some(id));
    assert_eq!(c.tabs().active_id(), id);
    assert_eq!(c.tab_infos().len(), count, "no second console tab");
    assert_eq!(
        c.tab_infos().iter().filter(|t| t.kind == "console").count(),
        1
    );
    assert!(has(&fx, &Effect::Layout), "the view changes: {fx:?}");
    assert_eq!(c.view(), View::Console(id));
}

#[test]
fn r24_opening_while_the_console_tab_is_active_changes_nothing_but_the_url() {
    // R24: console_open with the console tab active keeps one tab and shows the new url.
    let (mut c, id) = loaded();
    c.console_started(CONFIG);
    c.console_finished(CONFIG);
    let count = c.tab_infos().len();
    c.console_open(HOME);
    assert_eq!(c.console_tab(), Some(id));
    assert_eq!(c.tab_infos().len(), count);
    assert_eq!(c.tabs().active_id(), id);
    assert_eq!(c.tab_info(id).expect("info").url, HOME);
}

#[test]
fn r24_console_open_makes_no_web_op() {
    // R24 and R29: the shell has already started the load; the core makes no WebOp.
    let mut c = core();
    let fx = c.console_open(HOME);
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
    c.tab_new(None, Place::End);
    let fx = c.console_open(HOME);
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
}

// ---------------------------------------------------------------- R25: tab state

#[test]
fn r25_the_title_is_router_console_until_the_first_title_arrives() {
    // R25: until the first title arrives, it is `Router console`.
    let (c, id) = with_console();
    assert_eq!(c.tab_info(id).expect("info").title, "Router console");
}

#[test]
fn r25_the_title_is_the_document_title_of_the_console_page() {
    // R25: the tab title is the document title of the console page.
    let (mut c, id) = loaded();
    let fx = c.console_title("I2P Router Console - home");
    assert_eq!(
        c.tab_info(id).expect("info").title,
        "I2P Router Console - home"
    );
    assert!(has(&fx, &Effect::Emit(Event::TabUpdated(id))), "{fx:?}");
    c.console_title("Tunnels");
    assert_eq!(c.tab_info(id).expect("info").title, "Tunnels");
}

#[test]
fn r25_a_title_with_no_console_tab_does_nothing() {
    // R25: no console tab, no state to update.
    let mut c = core();
    assert!(c.console_title("x").is_empty());
    assert!(c.console_started(HOME).is_empty());
    assert!(c.console_finished(HOME).is_empty());
    assert_eq!(c.tab_infos().len(), 1);
}

#[test]
fn r25_the_url_and_loading_follow_the_page_loads_of_the_console_view() {
    // R25: the tab URL is the URL the console webview shows; loading follows its loads.
    let (mut c, id) = with_console();
    let fx = c.console_started(TUNNELS);
    let info = c.tab_info(id).expect("info");
    assert_eq!(info.url, TUNNELS);
    assert!(info.nav.loading);
    assert!(has(&fx, &Effect::Emit(Event::TabUpdated(id))), "{fx:?}");
    let fx = c.console_finished(TUNNELS);
    let info = c.tab_info(id).expect("info");
    assert_eq!(info.url, TUNNELS);
    assert!(!info.nav.loading);
    assert!(has(&fx, &Effect::Emit(Event::TabUpdated(id))), "{fx:?}");
}

#[test]
fn r25_a_console_page_never_enters_history() {
    // R25: a console page never enters history.
    let (mut c, _id) = loaded();
    c.console_title("I2P Router Console");
    c.console_started(TUNNELS);
    c.console_title("Tunnels");
    c.console_finished(TUNNELS);
    assert!(c.history_query(&HistoryQuery::default()).is_empty());
    assert!(c.suggest("127.0.0.1", 1).is_empty());
}

#[test]
fn r25_a_console_page_never_gets_a_site_icon() {
    // R25: never gets a site icon: no fetch, no icon.
    let (mut c, id) = with_console();
    let mut fx = c.console_started(HOME);
    fx.extend(c.console_title("Console"));
    fx.extend(c.console_finished(HOME));
    assert!(
        fx.iter().all(|e| !matches!(e, Effect::FetchIcon(_))),
        "{fx:?}"
    );
    assert_eq!(c.tab_info(id).expect("info").icon, None);
}

#[test]
fn r25_a_console_page_is_never_bookmarked() {
    // R25: never bookmarked; the bookmark command does nothing there.
    let (mut c, id) = loaded();
    let before = c.bookmarks_list().len();
    let fx = c.bookmark_toggle(5);
    assert!(!has(&fx, &Effect::Emit(Event::Bookmarks)), "{fx:?}");
    assert_eq!(c.bookmarks_list().len(), before);
    assert!(!c.tab_info(id).expect("info").marks.bookmarked);
    let new = NewBookmark {
        url: HOME.into(),
        title: "Console".into(),
        folder: None,
    };
    // Even a bookmark saved by another path never marks the console tab.
    if c.bookmark_add(&new, 6).is_ok() {
        assert!(!c.tab_info(id).expect("info").marks.bookmarked);
    }
}

#[test]
fn r25_find_does_nothing_in_a_console_tab() {
    // R25: find in page does nothing there: no engine call on any webview.
    let (mut c, _id) = loaded();
    let mut fx = c.find("router", true, false);
    fx.extend(c.find_close());
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
    assert!(
        engine_of(&fx).is_empty(),
        "no find call on the console view: {fx:?}"
    );
}

#[test]
fn r25_zoom_does_nothing_in_a_console_tab() {
    // R25: zoom does nothing there; the tab zoom stays 1.
    let (mut c, id) = loaded();
    let mut fx = Vec::new();
    for zoom in [Zoom::In, Zoom::Out, Zoom::Reset] {
        fx.extend(c.zoom(zoom));
    }
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
    assert!(engine_of(&fx).is_empty(), "{fx:?}");
    assert!((c.tab_info(id).expect("info").zoom - 1.0).abs() < 1e-9);
}

#[test]
fn r25_the_javascript_toggle_does_nothing_in_a_console_tab() {
    // R25 and R23: the console always runs JavaScript; a per-site switch never rebuilds it.
    let (mut c, id) = loaded();
    let mut fx = c.site_js_set("127.0.0.1", false);
    fx.extend(c.settings_set(&json!({"jsDefault": false})).unwrap().1);
    assert!(!web_op_for(&fx, id), "{fx:?}");
    assert!(load_urls(&fx).is_empty(), "{fx:?}");
    assert!(c.tab_info(id).expect("info").marks.js_on);
}

#[test]
fn r25_the_find_shortcut_leaves_the_find_bar_closed_in_a_console_tab() {
    // R25: the find shortcut does nothing there: no bar, no engine call, no layout change.
    use crate::shortcuts::Action;
    let (mut c, id) = loaded();
    assert!(!c.find_open());
    let fx = c.shortcut(Action::Find, 0);
    assert!(!c.find_open(), "the find bar stays closed: {fx:?}");
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
    assert!(console_ops(&fx).is_empty(), "{fx:?}");
    assert!(
        !has(&fx, &Effect::Layout),
        "no find bar to make room for: {fx:?}"
    );
    assert_eq!(c.console_tab(), Some(id));
}

#[test]
fn r25_find_leaves_the_find_bar_closed_and_makes_no_engine_call_in_a_console_tab() {
    // R25: find() does nothing there, forward or backward, with or without match case.
    let (mut c, id) = loaded();
    for (forward, match_case) in [(true, false), (false, false), (true, true), (false, true)] {
        let fx = c.find("router", forward, match_case);
        assert!(!c.find_open(), "the find bar stays closed: {fx:?}");
        assert!(web_ops(&fx).is_empty(), "{fx:?}");
        assert!(console_ops(&fx).is_empty(), "{fx:?}");
        assert!(!has(&fx, &Effect::Layout), "{fx:?}");
    }
    assert_eq!(c.console_tab(), Some(id));
}

// ---------------------------------------------------------------- R26: address bar

#[test]
fn r26_typing_an_i2p_address_makes_the_console_tab_a_web_tab() {
    // R26: the console tab becomes a web tab, with the normal load, and the console webview
    // is destroyed.
    let (mut c, id) = loaded();
    let (res, fx) = c.navigate("stats.i2p");
    assert_eq!(res, NavResult::ok());
    assert_eq!(c.console_tab(), None);
    assert!(closes(&fx), "the console webview is destroyed: {fx:?}");
    assert_eq!(load_urls(&fx), [STATS]);
    assert!(web_op_for(&fx, id), "the load is for the same tab: {fx:?}");
    let info = c.tab_info(id).expect("the tab stays");
    assert_eq!(info.kind, "web");
    assert_eq!(c.tabs().active_id(), id);
    assert_eq!(c.view(), View::Web(id));
    assert_eq!(kinds(&c), ["internal", "web"]);
}

#[test]
fn r26_typing_an_internal_page_makes_the_console_tab_an_internal_tab() {
    // R26: web or internal.
    let (mut c, id) = loaded();
    let (res, fx) = c.navigate("eepview://settings");
    assert_eq!(res, NavResult::ok());
    assert!(closes(&fx), "{fx:?}");
    assert_eq!(c.console_tab(), None);
    assert_eq!(c.tab_info(id).expect("info").kind, "internal");
    assert_eq!(c.view(), View::Internal("eepview://settings".into()));
}

#[test]
fn r26_the_new_tab_has_a_new_back_and_forward_list() {
    // R26: a new back/forward list: the console pages are not in it.
    let (mut c, id) = loaded();
    c.console_started(TUNNELS);
    c.console_finished(TUNNELS);
    assert!(c.tab_info(id).expect("info").nav.can_back);
    c.navigate("stats.i2p");
    c.page_started(id, STATS);
    c.page_finished(id, STATS, 1);
    let nav = c.tab_info(id).expect("info").nav;
    assert!(!nav.can_back, "no console page behind the web page");
    assert!(!nav.can_forward);
    let fx = c.step(Step::Back);
    assert!(console_ops(&fx).is_empty(), "{fx:?}");
}

#[test]
fn r26_a_typed_console_address_is_refused_like_any_non_i2p_address() {
    // R26: refused with `not-i2p`; only console_open() loads the console.
    for typed in [HOME, "http://127.0.0.1:7657/", "http://localhost:7657/home"] {
        let (mut c, id) = loaded();
        let (res, fx) = c.navigate(typed);
        assert_eq!(res, NavResult::refused("not-i2p"), "{typed}");
        assert!(load_urls(&fx).is_empty(), "{typed}: {fx:?}");
        assert!(c.tab_info(id).is_some(), "{typed}: the tab stays");
    }
}

#[test]
fn r26_a_console_address_typed_in_a_web_tab_never_loads_there() {
    // R26 and R29: the same refusal in a web tab, with a console tab open next to it.
    let mut c = core();
    visit(&mut c, STATS);
    let web = c.tabs().active_id();
    c.console_open(HOME);
    c.tab_select(web);
    let (res, fx) = c.navigate(HOME);
    assert_eq!(res, NavResult::refused("not-i2p"));
    assert!(load_urls(&fx).is_empty(), "{fx:?}");
    assert!(c.console_tab().is_some(), "the console tab is not touched");
}

/// The `Router console` tab as the UI sees it: its url, title and kind.
fn console_state(c: &Core, id: u32) -> (String, String, String) {
    let info = c.tab_info(id).expect("the console tab is still there");
    (info.url, info.title, info.kind.to_string())
}

#[test]
fn r26_a_refused_console_address_leaves_the_console_tab_as_it_was() {
    // R26: refused: the tab stays, no blocked page, no effect on the `console` webview.
    for typed in [HOME, "http://127.0.0.1:7657/", "http://localhost:7657/home"] {
        let (mut c, id) = loaded();
        let before = console_state(&c, id);
        let (res, fx) = c.navigate(typed);
        assert_ne!(res, NavResult::ok(), "{typed}");
        assert_eq!(c.console_tab(), Some(id), "{typed}");
        assert_eq!(console_state(&c, id), before, "{typed}: nothing changed");
        assert_eq!(before.2, "console");
        assert!(!closes(&fx), "{typed}: the console view stays: {fx:?}");
        assert!(web_ops(&fx).is_empty(), "{typed}: {fx:?}");
        assert_eq!(c.view(), View::Console(id), "{typed}");
    }
}

#[test]
fn r26_an_invalid_input_leaves_the_console_tab_as_it_was() {
    // R26: an input the address rules refuse as `invalid` changes nothing either.
    for typed in ["http://", "http://exa mple.i2p/"] {
        let (mut c, id) = loaded();
        let before = console_state(&c, id);
        let (res, fx) = c.navigate(typed);
        assert_eq!(res, NavResult::refused("invalid"), "{typed}");
        assert_eq!(c.console_tab(), Some(id), "{typed}");
        assert_eq!(console_state(&c, id), before, "{typed}: nothing changed");
        assert!(!closes(&fx), "{typed}: {fx:?}");
        assert!(web_ops(&fx).is_empty(), "{typed}: {fx:?}");
    }
}

// ---------------------------------------------------------------- R27: back, forward, reload, stop

#[test]
fn r27_back_and_forward_act_on_the_console_webview() {
    // R27: back and forward are console engine calls, never a WebOp.
    let (mut c, id) = loaded();
    c.console_started(TUNNELS);
    c.console_finished(TUNNELS);
    let fx = c.step(Step::Back);
    assert_eq!(engine_of(&fx), [&ConsoleOp::Back], "{fx:?}");
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
    c.console_started(HOME);
    c.console_finished(HOME);
    let fx = c.step(Step::Forward);
    assert_eq!(engine_of(&fx), [&ConsoleOp::Forward], "{fx:?}");
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
    assert_eq!(c.console_tab(), Some(id));
}

#[test]
fn r27_can_back_and_can_forward_follow_the_console_pages() {
    // R27: canBack and canForward follow the console pages loaded in that tab.
    let (mut c, id) = with_console();
    c.console_started(HOME);
    c.console_finished(HOME);
    let nav = c.tab_info(id).expect("info").nav;
    assert!(
        !nav.can_back && !nav.can_forward,
        "one page: nothing to step to"
    );
    c.console_started(TUNNELS);
    c.console_finished(TUNNELS);
    let nav = c.tab_info(id).expect("info").nav;
    assert!(nav.can_back && !nav.can_forward);
    c.step(Step::Back);
    c.console_started(HOME);
    c.console_finished(HOME);
    let nav = c.tab_info(id).expect("info").nav;
    assert!(!nav.can_back && nav.can_forward);
    c.step(Step::Forward);
    c.console_started(TUNNELS);
    c.console_finished(TUNNELS);
    let nav = c.tab_info(id).expect("info").nav;
    assert!(nav.can_back && !nav.can_forward);
}

#[test]
fn r27_reload_and_hard_reload_act_on_the_console_webview() {
    // R27: reload and hard reload are console engine calls.
    let (mut c, _id) = loaded();
    let fx = c.reload(false);
    assert_eq!(engine_of(&fx), [&ConsoleOp::Reload], "{fx:?}");
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
    let fx = c.reload(true);
    assert_eq!(engine_of(&fx), [&ConsoleOp::HardReload], "{fx:?}");
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
}

#[test]
fn r27_stop_acts_on_the_console_webview_while_it_loads() {
    // R27: stop is a console engine call.
    let (mut c, _id) = with_console();
    c.console_started(TUNNELS);
    let fx = c.stop();
    assert_eq!(engine_of(&fx), [&ConsoleOp::Stop], "{fx:?}");
    assert!(web_ops(&fx).is_empty(), "{fx:?}");
}

#[test]
fn r27_the_shortcuts_reach_the_console_webview_too() {
    // R27: the keyboard shortcuts for back, forward, reload and hard reload do the same.
    use crate::shortcuts::Action;
    let (mut c, _id) = loaded();
    c.console_started(TUNNELS);
    c.console_finished(TUNNELS);
    let mut all = c.shortcut(Action::Back, 0);
    c.console_started(HOME);
    c.console_finished(HOME);
    all.extend(c.shortcut(Action::Forward, 0));
    all.extend(c.shortcut(Action::Reload, 0));
    all.extend(c.shortcut(Action::HardReload, 0));
    assert!(web_ops(&all).is_empty(), "{all:?}");
    let ops = engine_of(&all);
    for want in [
        ConsoleOp::Back,
        ConsoleOp::Forward,
        ConsoleOp::Reload,
        ConsoleOp::HardReload,
    ] {
        assert!(ops.contains(&&want), "{want:?} missing: {all:?}");
    }
}

// ---------------------------------------------------------------- R28: close

#[test]
fn r28_closing_the_console_tab_destroys_the_console_webview() {
    // R28: closing destroys the `console` webview.
    let (mut c, id) = loaded();
    let fx = c.tab_close(id);
    assert!(closes(&fx), "{fx:?}");
    assert!(!web_op_for(&fx, id), "it is not a tab webview: {fx:?}");
    assert_eq!(c.console_tab(), None);
    assert!(c.tab_info(id).is_none());
    assert!(kinds(&c).iter().all(|k| k != "console"));
    assert_ne!(c.view(), View::Console(id));
    assert!(has(&fx, &Effect::Emit(Event::TabsChanged)), "{fx:?}");
}

#[test]
fn r28_closing_another_tab_keeps_the_console_tab() {
    // R28: only closing the console tab destroys the console webview.
    let (mut c, id) = loaded();
    c.tab_new(None, Place::End);
    let other = c.tabs().active_id();
    let fx = c.tab_close(other);
    assert!(!closes(&fx), "{fx:?}");
    assert_eq!(c.console_tab(), Some(id));
}

#[test]
fn r28_reopen_closed_tab_never_brings_a_console_tab_back() {
    // R28: "Reopen closed tab" never brings a console tab back.
    let (mut c, id) = loaded();
    c.tab_close(id);
    let fx = c.tab_reopen();
    assert_eq!(c.console_tab(), None);
    assert!(kinds(&c).iter().all(|k| k != "console"));
    assert!(
        load_urls(&fx).iter().all(|u| !u.contains("127.0.0.1")),
        "{fx:?}"
    );
    assert!(console_ops(&fx).is_empty(), "{fx:?}");
}

#[test]
fn r28_reopen_brings_back_the_last_closed_page_not_the_console() {
    // R28: the console tab is not on the stack of closed tabs.
    let mut c = core();
    visit(&mut c, STATS);
    let web = c.tabs().active_id();
    c.tab_new(None, Place::End);
    c.console_open(HOME);
    let console = c.console_tab().expect("console tab");
    c.tab_close(web);
    c.tab_close(console);
    let fx = c.tab_reopen();
    assert_eq!(load_urls(&fx), [STATS], "{fx:?}");
    assert_eq!(c.console_tab(), None);
}

#[test]
fn r28_a_restart_shows_no_console_tab() {
    // R28: eepview does not restore tabs at start, so a restart never shows a console tab.
    let dir = testdir::fresh("console-tab");
    let paths = Paths {
        bookmarks: dir.join("bookmarks.json"),
        history: dir.join("history.json"),
        settings: dir.join("settings.json"),
        sites: dir.join("sites.json"),
        icons: dir.join("icons"),
    };
    let mut c = Core::new(Some(paths.clone()), "127.0.0.1:4444", 0);
    c.router_changed(status("ok", false));
    c.console_open(HOME);
    c.console_started(HOME);
    c.console_finished(HOME);
    assert!(c.console_tab().is_some());
    drop(c);
    let back = Core::new(Some(paths), "127.0.0.1:4444", 0);
    assert_eq!(back.console_tab(), None);
    assert_eq!(back.tab_infos().len(), 1);
    assert_eq!(kinds(&back), ["internal"]);
}

// ---------------------------------------------------------------- R29: no WebOp, no console URL in a web tab

#[test]
fn r29_no_command_makes_a_web_op_for_the_console_tab() {
    // R29: the core never makes a WebOp (load, engine call, destroy) for the console tab.
    let (mut c, id) = loaded();
    let mut all: Vec<Effect> = Vec::new();
    all.extend(c.router_changed(status("down", false)));
    all.extend(c.router_changed(status("ok", false)));
    all.extend(c.pause());
    all.extend(c.resume());
    all.extend(c.router_changed(status("ok", false)));
    all.extend(c.reload(false));
    all.extend(c.reload(true));
    all.extend(c.stop());
    all.extend(c.step(Step::Back));
    all.extend(c.step(Step::Forward));
    all.extend(c.zoom(Zoom::In));
    all.extend(c.find("x", true, false));
    all.extend(c.find_close());
    all.extend(c.settings_set(&json!({"jsDefault": false})).unwrap().1);
    all.extend(c.site_js_set("stats.i2p", false));
    all.extend(c.console_started(TUNNELS));
    all.extend(c.console_title("Tunnels"));
    all.extend(c.console_finished(TUNNELS));
    all.extend(c.console_open(HOME));
    assert_eq!(c.console_tab(), Some(id));
    assert!(!web_op_for(&all, id), "{all:?}");
    assert!(
        load_urls(&all).iter().all(|u| !u.contains("127.0.0.1")),
        "{all:?}"
    );
}

#[test]
fn r29_a_loopback_load_in_a_web_tab_shows_the_blocked_page() {
    // R29: a load of a non-I2P URL in a web tab shows the blocked page, as before.
    let mut c = core();
    visit(&mut c, STATS);
    let web = c.tabs().active_id();
    c.console_open(HOME);
    let fx = c.load(web, HOME);
    assert!(load_urls(&fx).is_empty(), "{fx:?}");
    let info = c.tab_info(web).expect("web tab");
    assert!(info.url.starts_with("eepview://blocked"), "{}", info.url);
    assert!(c.console_tab().is_some());
}

#[test]
fn r29_a_console_url_never_reaches_a_web_tab_through_the_engine_guard() {
    // R29: a navigation, or a new window, to a console page in a web tab is refused.
    let mut c = core();
    visit(&mut c, STATS);
    let web = c.tabs().active_id();
    c.console_open(HOME);
    let url = Url::parse(HOME).expect("url");
    let (allowed, fx) = c.tab_navigation(web, &url);
    assert!(!allowed, "{fx:?}");
    let tabs = c.tab_infos().len();
    let fx = c.new_window(&url);
    assert!(load_urls(&fx).is_empty(), "{fx:?}");
    assert_eq!(c.tab_infos().len(), tabs, "no tab for a console URL");
    assert_eq!(
        kinds(&c).iter().filter(|k| k.as_str() == "console").count(),
        1
    );
}

#[test]
fn r29_the_page_events_of_the_console_view_never_reach_a_web_tab() {
    // R29: a console page start for a web tab id is ignored; the web tab keeps its page.
    let mut c = core();
    visit(&mut c, STATS);
    let web = c.tabs().active_id();
    c.console_open(HOME);
    c.console_started(CONFIG);
    c.console_finished(CONFIG);
    assert_eq!(c.tab_info(web).expect("web tab").url, STATS);
    assert_eq!(
        c.tab_info(c.console_tab().expect("console tab"))
            .expect("info")
            .url,
        CONFIG
    );
    assert!(c.page_started(web, HOME).is_empty());
}

// ---------------------------------------------------------------- R10 in the core: a new tab for .i2p

#[test]
fn r10_an_i2p_link_from_the_console_opens_a_new_normal_tab_next_to_the_console_tab() {
    // R10: the URL opens in a new normal tab through the existing guard (new_window).
    let (mut c, id) = loaded();
    let fx = c.new_window(&Url::parse(REG).expect("url"));
    assert_eq!(load_urls(&fx), [REG], "{fx:?}");
    assert_eq!(c.console_tab(), Some(id), "the console tab stays");
    assert_eq!(kinds(&c).iter().filter(|k| k.as_str() == "web").count(), 1);
    assert_eq!(c.tab_info(id).expect("info").kind, "console");
}

#[test]
fn r10_a_clearnet_link_from_the_console_opens_nothing() {
    // R10: anything else is dropped.
    let (mut c, id) = loaded();
    let tabs = c.tab_infos().len();
    let fx = c.new_window(&Url::parse("http://example.com/").expect("url"));
    assert!(load_urls(&fx).is_empty(), "{fx:?}");
    assert_eq!(c.tab_infos().len(), tabs);
    assert_eq!(c.console_tab(), Some(id));
}

// ---------------------------------------------------------------- R30: console gone

#[test]
fn r30_console_gone_closes_the_console_tab() {
    // R30: the console tab closes and the console webview is destroyed.
    let (mut c, id) = loaded();
    let fx = c.console_gone();
    assert!(closes(&fx), "{fx:?}");
    assert!(!web_op_for(&fx, id), "{fx:?}");
    assert!(has(&fx, &Effect::Emit(Event::TabsChanged)), "{fx:?}");
    assert_eq!(c.console_tab(), None);
    assert!(c.tab_info(id).is_none());
    assert!(kinds(&c).iter().all(|k| k != "console"));
    assert_ne!(c.view(), View::Console(id));
}

#[test]
fn r30_console_gone_with_no_console_tab_does_nothing() {
    // R30: if open: with none, no effect.
    let mut c = core();
    assert!(c.console_gone().is_empty());
    assert_eq!(c.tab_infos().len(), 1);
}

#[test]
fn r30_console_gone_closes_a_console_tab_that_is_not_active() {
    // R30: whether the tab is active or not.
    let (mut c, id) = loaded();
    let home = ids(&c)[0];
    c.tab_select(home);
    let fx = c.console_gone();
    assert!(closes(&fx), "{fx:?}");
    assert_eq!(c.console_tab(), None);
    assert!(c.tab_info(id).is_none());
    assert_eq!(c.tabs().active_id(), home, "the active tab stays");
}

#[test]
fn r30_a_console_tab_that_was_converted_to_a_web_tab_is_not_closed() {
    // R26 and R30: after typing an address the tab is a normal tab; console_gone leaves it.
    let (mut c, id) = loaded();
    c.navigate("stats.i2p");
    let fx = c.console_gone();
    assert!(!closes(&fx), "{fx:?}");
    assert!(c.tab_info(id).is_some(), "the web tab stays");
    assert_eq!(c.tab_info(id).expect("info").kind, "web");
}

#[test]
fn r30_the_console_can_be_opened_again_after_it_went_away() {
    // R24 and R30: a new console tab after the old one closed.
    let (mut c, first) = loaded();
    c.console_gone();
    c.console_open(HOME);
    let second = c.console_tab().expect("a new console tab");
    assert_ne!(first, second, "a new tab id");
    assert_eq!(c.tab_info(second).expect("info").kind, "console");
    assert_eq!(c.tab_info(second).expect("info").title, "Router console");
}

// ---------------------------------------------------------------- R19: fail closed

/// The toasts of `fx` as (kind, text).
fn toasts(fx: &[Effect]) -> Vec<(&'static str, String)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Emit(Event::Toast(t)) => Some((t.kind, t.text.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn r19_console_failed_closes_the_console_tab_and_destroys_the_view() {
    // R19: the rule list could not be attached: the tab closes, the webview is destroyed.
    let (mut c, id) = with_console();
    let fx = c.console_failed();
    assert!(closes(&fx), "{fx:?}");
    assert!(engine_of(&fx).is_empty(), "{fx:?}");
    assert_eq!(c.console_tab(), None);
    assert!(c.tab_info(id).is_none());
    assert!(kinds(&c).iter().all(|k| k != "console"));
    assert_ne!(c.view(), View::Console(id));
}

#[test]
fn r19_console_failed_shows_one_warning_toast() {
    // R19: a warning toast says the console could not be opened safely.
    let (mut c, _id) = with_console();
    let fx = c.console_failed();
    let want = (
        "warn",
        "The router console could not be opened safely".to_owned(),
    );
    assert_eq!(toasts(&fx), [want], "{fx:?}");
}
