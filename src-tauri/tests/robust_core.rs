// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Random command sequences against the browser state machine (`core::Core`).
//! Each property names the requirement it checks (`Req:`).
//!
//! Sources: `docs/wiki/ipc-contract.md` (IPC), `docs/wiki/adr-0001-no-leak-architecture.md`
//! (ADR), `docs/wiki/browser-shell.md` and the doc comments of the public functions.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use eepview_lib::core::{Core, Effect, EngineOp, WebOp};
use eepview_lib::nav::is_allowed_web;
use eepview_lib::session::Step;
use eepview_lib::tabs::Place;
use eepview_lib::types::{HistoryQuery, RouterStatus};
use proptest::prelude::*;
use tauri::Url;

/// One command or engine event.
#[derive(Debug, Clone)]
enum Op {
    TabNew(Option<String>, bool),
    TabClose(Pick),
    TabSelect(Pick),
    TabMove(Pick, usize),
    Navigate(String),
    PageStarted(Pick, String),
    PageFinished(Pick, String),
    Title(Pick, String),
    Step(bool),
    Reload(bool),
    Stop,
    Home,
    Reopen,
    Cycle(bool),
    Number(usize),
    Router(usize),
    Pause,
    Resume,
    Hover(Pick, Option<String>),
    TabNavigation(Pick, String),
    NewWindow(String),
    Settings(bool, bool),
}

/// A tab id: the n-th open tab, or an id that may not exist.
#[derive(Debug, Clone)]
struct Pick(usize, bool);

const STATES: [&str; 5] = ["ok", "down", "verifying", "building", "not-i2p"];

fn pick() -> impl Strategy<Value = Pick> {
    (0..64usize, prop::bool::weighted(0.15)).prop_map(|(n, bogus)| Pick(n, bogus))
}

/// What the user types or the engine reports: I2P, clearnet, internal, text, garbage.
fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        "(https?://)?[a-z0-9]{1,8}(\\.[a-z0-9]{1,8}){0,2}\\.i2p(/[a-z0-9]{0,6}){0,2}",
        Just("http://example.com/".to_owned()),
        Just("http://127.0.0.1:7657/".to_owned()),
        Just("eepview://settings".to_owned()),
        Just("eepview://history?q=x".to_owned()),
        "[a-z ]{0,12}",
        "\\PC{0,40}",
    ]
}

/// What an engine reports for a page it was allowed to load.
fn engine_url() -> impl Strategy<Value = String> {
    prop_oneof![
        "https?://[a-z0-9]{1,6}\\.i2p(/[a-z0-9]{0,4}){0,2}",
        Just("http://stats.i2p/".to_owned()),
        Just("about:blank".to_owned()),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (prop::option::of(text()), any::<bool>()).prop_map(|(u, a)| Op::TabNew(u, a)),
        pick().prop_map(Op::TabClose),
        pick().prop_map(Op::TabSelect),
        (pick(), 0..100usize).prop_map(|(p, i)| Op::TabMove(p, i)),
        text().prop_map(Op::Navigate),
        (pick(), engine_url()).prop_map(|(p, u)| Op::PageStarted(p, u)),
        (pick(), engine_url()).prop_map(|(p, u)| Op::PageFinished(p, u)),
        (pick(), "\\PC{0,20}").prop_map(|(p, t)| Op::Title(p, t)),
        any::<bool>().prop_map(Op::Step),
        any::<bool>().prop_map(Op::Reload),
        Just(Op::Stop),
        Just(Op::Home),
        Just(Op::Reopen),
        any::<bool>().prop_map(Op::Cycle),
        (0..12usize).prop_map(Op::Number),
        (0..STATES.len()).prop_map(Op::Router),
        Just(Op::Pause),
        Just(Op::Resume),
        (pick(), prop::option::of(text())).prop_map(|(p, t)| Op::Hover(p, t)),
        (pick(), text()).prop_map(|(p, u)| Op::TabNavigation(p, u)),
        text().prop_map(Op::NewWindow),
        (any::<bool>(), any::<bool>()).prop_map(|(js, hist)| Op::Settings(js, hist)),
    ]
}

fn status(state: &'static str) -> RouterStatus {
    RouterStatus {
        state,
        proxy: "127.0.0.1:4444".into(),
        version: None,
        detail: None,
        paused: false,
        managed: false,
    }
}

/// The tab webviews the shell would hold, rebuilt from the effects.
#[derive(Default)]
struct Shell {
    live: BTreeSet<u32>,
    bad_load: Option<String>,
}

impl Shell {
    fn apply(&mut self, effects: &[Effect]) {
        for effect in effects {
            self.effect(effect);
        }
    }

    fn effect(&mut self, effect: &Effect) {
        match effect {
            Effect::Web(WebOp::Load(load)) => self.load(&load.url, load.tab),
            Effect::Web(WebOp::Destroy(id)) => {
                self.live.remove(id);
            }
            _ => {}
        }
    }

    fn load(&mut self, url: &str, tab: u32) {
        self.live.insert(tab);
        let allowed = Url::parse(url).is_ok_and(|u| is_allowed_web(&u));
        if !allowed && self.bad_load.is_none() {
            self.bad_load = Some(url.to_owned());
        }
    }
}

fn resolve(core: &Core, pick: &Pick) -> u32 {
    let ids: Vec<u32> = core.tab_infos().iter().map(|t| t.id).collect();
    if pick.1 || ids.is_empty() {
        return u32::try_from(pick.0).unwrap_or(0) * 1000;
    }
    ids[pick.0 % ids.len()]
}

fn run(core: &mut Core, op: &Op, now: u64) -> Vec<Effect> {
    match op {
        Op::TabNew(url, after) => {
            let place = if *after {
                Place::AfterActive
            } else {
                Place::End
            };
            core.tab_new(url.as_deref(), place).1
        }
        Op::TabClose(p) => core.tab_close(resolve(core, p)),
        Op::TabSelect(p) => core.tab_select(resolve(core, p)),
        Op::TabMove(p, i) => core.tab_move(resolve(core, p), *i),
        Op::Navigate(input) => core.navigate(input).1,
        Op::PageStarted(p, url) => core.page_started(resolve(core, p), url),
        Op::PageFinished(p, url) => core.page_finished(resolve(core, p), url, now),
        Op::Title(p, title) => core.title_changed(resolve(core, p), title),
        Op::Step(forward) => core.step(if *forward { Step::Forward } else { Step::Back }),
        Op::Reload(hard) => core.reload(*hard),
        Op::Stop => core.stop(),
        Op::Home => core.home(),
        Op::Reopen => core.tab_reopen(),
        Op::Cycle(forward) => core.tab_cycle(*forward),
        Op::Number(n) => core.tab_number(*n),
        Op::Router(i) => core.router_changed(status(STATES[*i])),
        Op::Pause => core.pause(),
        Op::Resume => core.resume(),
        Op::Hover(p, target) => core.hover_link(resolve(core, p), target.as_deref()),
        Op::TabNavigation(p, url) => run_navigation(core, resolve(core, p), url),
        Op::NewWindow(url) => run_new_window(core, url),
        Op::Settings(js, history) => run_settings(core, *js, *history),
    }
}

fn run_navigation(core: &mut Core, id: u32, url: &str) -> Vec<Effect> {
    Url::parse(url).map_or_else(|_| Vec::new(), |u| core.tab_navigation(id, &u).1)
}

fn run_new_window(core: &mut Core, url: &str) -> Vec<Effect> {
    Url::parse(url).map_or_else(|_| Vec::new(), |u| core.new_window(&u))
}

fn run_settings(core: &mut Core, js: bool, history: bool) -> Vec<Effect> {
    let patch = serde_json::json!({ "jsDefault": js, "history": { "enabled": history } });
    core.settings_set(&patch)
        .map(|(_, fx)| fx)
        .unwrap_or_default()
}

fn web_enabled(core: &Core) -> bool {
    core.router().state == "ok" && !core.paused()
}

/// The invariants that hold after every command.
fn check(core: &Core, shell: &Shell) -> Result<(), TestCaseError> {
    let infos = core.tab_infos();
    // Req: IPC `tab_close`: closing the last tab opens a home tab, so a tab always exists.
    prop_assert!(!infos.is_empty(), "no tab");
    // Req: IPC `tab_new`/`tab_select`: one tab is active, always (the strip marks it).
    prop_assert_eq!(infos.iter().filter(|t| t.marks.active).count(), 1);
    let ids: BTreeSet<u32> = infos.iter().map(|t| t.id).collect();
    prop_assert_eq!(ids.len(), infos.len(), "ids repeat");
    prop_assert_eq!(infos.len(), core.tabs().len());
    prop_assert!(ids.contains(&core.tabs().active_id()));
    for info in &infos {
        prop_assert!(
            info.zoom.is_finite() && info.zoom > 0.0,
            "zoom {}",
            info.zoom
        );
        prop_assert!(info.kind == "web" || info.kind == "internal");
    }
    // Req: IPC Security rules: "No tab-* webview exists before VERIFY passes. When the router
    // goes down, or you pause, every tab-* is destroyed."
    if !web_enabled(core) {
        prop_assert!(
            shell.live.is_empty(),
            "webviews {:?} live while the router is not ok",
            shell.live
        );
    }
    // A closed tab keeps no webview.
    prop_assert!(
        shell.live.is_subset(&ids),
        "webview of a closed tab: {:?}",
        shell.live
    );
    // Req: ADR L4 / `Load.url`: a tab webview only ever loads an I2P URL.
    prop_assert!(shell.bad_load.is_none(), "Load of {:?}", shell.bad_load);
    Ok(())
}

fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

proptest! {
    #![proptest_config(config())]

    // Req: IPC tabs and navigation: no sequence of commands and engine events panics or breaks
    // the strip invariants (checked in `check`).
    #[test]
    fn random_command_sequences_keep_the_invariants(ops in prop::collection::vec(op(), 1..80)) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        let mut shell = Shell::default();
        check(&core, &shell)?;
        for (i, op) in ops.iter().enumerate() {
            let fx = run(&mut core, op, i as u64 + 1);
            shell.apply(&fx);
            check(&core, &shell).map_err(|e| TestCaseError::fail(format!("after {op:?}: {e}")))?;
        }
    }

    // Req: IPC `tab_new`: the new tab becomes active; `tab_close`: closing the last tab opens a
    // new home tab; tab count follows the commands.
    #[test]
    fn open_and_close_follow_the_contract(urls in prop::collection::vec(text(), 1..20), order in prop::collection::vec(0..100usize, 1..40)) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        core.router_changed(status("ok"));
        for url in &urls {
            let before = core.tabs().len();
            let (info, _) = core.tab_new(Some(url), Place::End);
            prop_assert_eq!(core.tabs().len(), before + 1);
            let info = info.ok_or_else(|| TestCaseError::fail("tab_new returned no tab"))?;
            prop_assert_eq!(core.tabs().active_id(), info.id);
        }
        for n in order {
            let ids: Vec<u32> = core.tab_infos().iter().map(|t| t.id).collect();
            let before = ids.len();
            core.tab_close(ids[n % ids.len()]);
            prop_assert_eq!(core.tabs().len(), if before == 1 { 1 } else { before - 1 });
        }
        while core.tabs().len() > 1 {
            let id = core.tabs().active_id();
            core.tab_close(id);
        }
        let id = core.tabs().active_id();
        core.tab_close(id);
        let info = core.tab_info(core.tabs().active_id()).ok_or_else(|| TestCaseError::fail("no tab"))?;
        prop_assert_eq!(info.url, core.home_url());
    }

    // Req: IPC `tab_move(id, index)`: moving keeps every tab, whatever the index.
    #[test]
    fn move_is_a_permutation(moves in prop::collection::vec((0..8usize, 0..30usize), 1..30)) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        for _ in 0..4 {
            core.tab_new(None, Place::End);
        }
        let before: BTreeSet<u32> = core.tab_infos().iter().map(|t| t.id).collect();
        for (n, index) in moves {
            let ids: Vec<u32> = core.tab_infos().iter().map(|t| t.id).collect();
            core.tab_move(ids[n % ids.len()], index);
            let after: BTreeSet<u32> = core.tab_infos().iter().map(|t| t.id).collect();
            prop_assert_eq!(&after, &before);
            prop_assert_eq!(core.tab_infos().len(), before.len());
        }
    }

    // Req: IPC `navigate`: anything that is not an I2P site is refused with `not-i2p` (or
    // `invalid`, `router-down`) and shows `eepview://blocked?url=…`; an accepted input never
    // leaves a clearnet URL in the tab.
    #[test]
    fn navigate_answers_by_the_contract(inputs in prop::collection::vec(text(), 1..20)) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        core.router_changed(status("ok"));
        let mut shell = Shell::default();
        for input in inputs {
            let (result, fx) = core.navigate(&input);
            shell.apply(&fx);
            let url = core.tab_info(core.tabs().active_id()).map(|t| t.url).unwrap_or_default();
            if result.ok {
                prop_assert!(result.reason.is_none());
                prop_assert!(!url.starts_with("http") || Url::parse(&url).is_ok_and(|u| is_allowed_web(&u)), "{input:?} -> {url}");
            } else {
                let reason = result.reason.unwrap_or("");
                prop_assert!(["not-i2p", "invalid", "router-down"].contains(&reason), "{reason}");
                prop_assert!(reason == "router-down" || url.starts_with("eepview://blocked?url="), "{input:?} -> {url}");
            }
            prop_assert!(shell.bad_load.is_none(), "Load of {:?}", shell.bad_load);
        }
    }

    // Req: ADR "Page events from a hidden web view are ignored while the tab shows an internal
    // page": the tab keeps its internal page and history gets no entry.
    #[test]
    fn hidden_webview_events_are_ignored(url in engine_url(), title in "\\PC{0,20}") {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        core.router_changed(status("ok"));
        let id = core.tabs().active_id();
        core.page_started(id, &url);
        core.page_finished(id, &url, 5);
        core.title_changed(id, &title);
        let info = core.tab_info(id).ok_or_else(|| TestCaseError::fail("no tab"))?;
        prop_assert_eq!(info.url, "eepview://home");
        prop_assert!(core.history_query(&HistoryQuery::default()).is_empty());
    }

    // Req: ADR Goal "no single mistake … may open a clearnet path": even when an engine reports
    // a clearnet page (a bug in L4 elsewhere), the core loads only I2P URLs in a tab webview.
    #[test]
    fn hostile_engine_events_never_make_a_clearnet_load(
        events in prop::collection::vec((any::<bool>(), "(https?://)?[a-z0-9.]{1,12}(:[0-9]{2,4})?(/[a-z]{0,4})?"), 1..20),
        later in prop::collection::vec(any::<u8>(), 1..10),
    ) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        let mut shell = Shell::default();
        shell.apply(&core.router_changed(status("ok")));
        shell.apply(&core.navigate("a.i2p").1);
        let id = core.tabs().active_id();
        for (finished, url) in events {
            let fx = if finished { core.page_finished(id, &url, 1) } else { core.page_started(id, &url) };
            shell.apply(&fx);
        }
        // The router drops and returns (the watcher does it every outage), then the user
        // switches tabs and reloads.
        shell.apply(&core.router_changed(status("down")));
        shell.apply(&core.router_changed(status("ok")));
        for n in later {
            let fx = match n % 3 {
                0 => core.reload(false),
                1 => core.step(Step::Back),
                _ => core.tab_new(None, Place::End).1,
            };
            shell.apply(&fx);
        }
        prop_assert!(shell.bad_load.is_none(), "Load of {:?}", shell.bad_load);
    }
}

// Req: task "open and close 1 000 tabs": the state machine holds up, ids stay unique, no
// webview is left behind, and the work is fast.
#[test]
fn soak_one_thousand_tabs() {
    let started = Instant::now();
    let mut core = Core::new(None, "127.0.0.1:4444", 0);
    let mut shell = Shell::default();
    shell.apply(&core.router_changed(status("ok")));
    let mut seen = BTreeSet::new();
    for round in 0..1000u64 {
        let (info, fx) = core.tab_new(Some(&format!("http://site{round}.i2p/")), Place::End);
        shell.apply(&fx);
        let id = info.map_or(0, |t| t.id);
        assert!(seen.insert(id), "id {id} reused");
        shell.apply(&core.page_started(id, &format!("http://site{round}.i2p/")));
        shell.apply(&core.page_finished(id, &format!("http://site{round}.i2p/"), round));
    }
    assert_eq!(core.tabs().len(), 1001);
    while core.tabs().len() > 1 {
        let id = core.tabs().active_id();
        shell.apply(&core.tab_close(id));
    }
    let id = core.tabs().active_id();
    shell.apply(&core.tab_close(id));
    assert_eq!(core.tabs().len(), 1);
    assert!(shell.live.len() <= 1, "{} webviews left", shell.live.len());
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "{:?}",
        started.elapsed()
    );
    // "Reopen closed tab" keeps a bounded list, not 1 000 entries.
    for _ in 0..1000 {
        core.tab_reopen();
    }
    assert!(
        core.tabs().len() < 200,
        "{} tabs after reopen",
        core.tabs().len()
    );
}

/// What the user does in the back/forward model.
#[derive(Debug, Clone)]
enum Nav {
    Go(String),
    Back,
    Forward,
}

fn nav_op() -> impl Strategy<Value = Nav> {
    prop_oneof![
        3 => "[a-z]{1,3}".prop_map(|n| Nav::Go(format!("http://{n}.i2p/"))),
        1 => prop_oneof![Just("eepview://settings"), Just("eepview://bookmarks")].prop_map(|u| Nav::Go(u.to_owned())),
        2 => Just(Nav::Back),
        2 => Just(Nav::Forward),
    ]
}

/// The engine of the tab: it shows the page of every load and native step.
fn engine_plays(core: &mut Core, id: u32, fx: &[Effect], entries: &[String], index: usize) {
    for effect in fx {
        let url = match effect {
            Effect::Web(WebOp::Load(load)) => Some(load.url.clone()),
            Effect::Web(WebOp::Engine(_, EngineOp::Back | EngineOp::Forward)) => {
                entries.get(index).cloned()
            }
            _ => None,
        };
        if let Some(url) = url {
            core.page_started(id, &url);
            core.page_finished(id, &url, 1);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..config() })]

    // Req: PO decision "back/forward is standard per-tab session history and a new navigation
    // clears the forward list": a list of pages and a position; Back and Forward move the
    // position (and do nothing at an end); a new page cuts the pages after the position.
    // The tab shows the page at the position, and the back/forward flags follow it.
    #[test]
    fn back_and_forward_are_a_standard_session_history(ops in prop::collection::vec(nav_op(), 1..40)) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        core.router_changed(status("ok"));
        let id = core.tabs().active_id();
        let mut entries = vec!["eepview://home".to_owned()];
        let mut index = 0usize;
        for op in ops {
            let fx = match &op {
                Nav::Go(url) if *url != entries[index] => {
                    entries.truncate(index + 1);
                    entries.push(url.clone());
                    index += 1;
                    core.navigate(url).1
                }
                Nav::Back if index > 0 => {
                    index -= 1;
                    core.step(Step::Back)
                }
                Nav::Forward if index + 1 < entries.len() => {
                    index += 1;
                    core.step(Step::Forward)
                }
                _ => continue,
            };
            engine_plays(&mut core, id, &fx, &entries, index);
            let info = core.tab_info(id).ok_or_else(|| TestCaseError::fail("no tab"))?;
            prop_assert_eq!(&info.url, &entries[index], "after {:?}", op);
            prop_assert_eq!(info.nav.can_back, index > 0, "after {:?}", op);
            prop_assert_eq!(info.nav.can_forward, index + 1 < entries.len(), "after {:?}", op);
        }
    }
}
