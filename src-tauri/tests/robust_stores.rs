// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Bookmarks, history, settings and site stores through the commands of `core::Core`:
//! random command sequences, corrupted files, the history cap. Each property names the
//! requirement it checks (`Req:`).
//!
//! Sources: `docs/wiki/ipc-contract.md` (IPC), `docs/wiki/browser-shell.md` (shell),
//! `docs/wiki/adr-0001-no-leak-architecture.md` (ADR), the doc comments of the public
//! functions, and the robustness brief (a corrupt file costs only that file).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use eepview_lib::core::{Core, Effect, Paths, WebOp};
use eepview_lib::nav::is_allowed_web;
use eepview_lib::tabs::Place;
use eepview_lib::types::{Bookmark, Cursor, HistoryEntry, HistoryQuery, NewBookmark, RouterStatus};
use proptest::prelude::*;
use serde_json::{Value, json};
use tauri::Url;

/// Entries the contract allows in the history.
const HISTORY_CAP: usize = 10_000;
const CAP_PLUS: u32 = 10_050;
static NEXT: AtomicU32 = AtomicU32::new(0);

fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// The four store paths in a fresh directory. The directory goes away when this drops, also
/// when a failing case returns early or panics.
struct TempPaths(Paths);

impl std::ops::Deref for TempPaths {
    type Target = Paths;

    fn deref(&self) -> &Paths {
        &self.0
    }
}

impl Drop for TempPaths {
    fn drop(&mut self) {
        if let Some(dir) = self.0.history.parent() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

fn paths() -> TempPaths {
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir: PathBuf =
        std::env::temp_dir().join(format!("eepview-robust-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    TempPaths(Paths {
        bookmarks: dir.join("bookmarks.json"),
        history: dir.join("history.json"),
        settings: dir.join("settings.json"),
        sites: dir.join("sites.json"),
        icons: dir.join("icons"),
    })
}

/// The JS flag a new tab for `url` loads with (needs a verified router).
fn site_js(core: &mut Core, url: &str) -> Option<bool> {
    let (_, fx) = core.tab_new(Some(url), Place::End);
    fx.iter().find_map(|e| match e {
        Effect::Web(WebOp::Load(load)) => Some(load.js),
        _ => None,
    })
}

fn files(paths: &Paths) -> [&PathBuf; 4] {
    [
        &paths.bookmarks,
        &paths.history,
        &paths.settings,
        &paths.sites,
    ]
}

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

fn all_history(core: &Core) -> Vec<HistoryEntry> {
    let query = HistoryQuery {
        limit: Some(usize::MAX),
        ..HistoryQuery::default()
    };
    core.history_query(&query)
}

fn url_of(n: u32) -> String {
    format!("http://site{n}.i2p/")
}

fn visit(core: &mut Core, n: u32, now: u64) {
    let id = core.tabs().active_id();
    core.navigate(&url_of(n));
    core.page_started(id, &url_of(n));
    core.page_finished(id, &url_of(n), now);
}

fn new_bookmark(url: &str, title: &str) -> NewBookmark {
    NewBookmark {
        url: url.to_owned(),
        title: title.to_owned(),
        folder: None,
    }
}

/// Req: browser-shell "Stores are JSON files with a `version` field".
fn assert_versioned(path: &PathBuf) -> Result<(), TestCaseError> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(());
    };
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| TestCaseError::fail(format!("{}: {e}", path.display())))?;
    prop_assert!(
        value["version"].is_number(),
        "{} has no version",
        path.display()
    );
    Ok(())
}

#[derive(Debug, Clone)]
enum Op {
    Visit(u32),
    Bookmark(u32, String),
    Unbookmark(usize),
    RemoveHistory(usize),
    ClearHistory(usize),
    Import(String),
    History(bool),
    Zoom,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..30u32).prop_map(Op::Visit),
        (0..30u32, "\\PC{0,12}").prop_map(|(n, t)| Op::Bookmark(n, t)),
        (0..50usize).prop_map(Op::Unbookmark),
        (0..50usize).prop_map(Op::RemoveHistory),
        (0..4usize).prop_map(Op::ClearHistory),
        import_text().prop_map(Op::Import),
        any::<bool>().prop_map(Op::History),
        Just(Op::Zoom),
    ]
}

/// Import text: valid exports, bare lists, wrong shapes and garbage.
fn import_text() -> impl Strategy<Value = String> {
    prop_oneof![
        "\\PC{0,60}",
        Just("[]".to_owned()),
        Just("{\"bookmarks\":[]}".to_owned()),
        Just("[{\"id\":\"1\",\"url\":\"http://x.i2p/\",\"title\":\"t\"}]".to_owned()),
        Just("[{\"id\":\"1\",\"url\":\"http://evil.com/\",\"title\":\"t\"}]".to_owned()),
        Just("[{\"id\":1}]".to_owned()),
        Just("null".to_owned()),
        Just("[".repeat(500)),
    ]
}

const RANGES: [&str; 4] = ["hour", "day", "week", "all"];

fn apply(core: &mut Core, op: &Op, now: u64) {
    match op {
        Op::Visit(n) => visit(core, *n, now),
        Op::Bookmark(n, title) => {
            let _ = core.bookmark_add(&new_bookmark(&url_of(*n), title), now);
        }
        Op::Unbookmark(i) => {
            let list = core.bookmarks_list();
            if !list.is_empty() {
                core.bookmark_remove(&list[i % list.len()].id);
            }
        }
        Op::RemoveHistory(i) => {
            let list = all_history(core);
            if !list.is_empty() {
                core.history_remove(&list[i % list.len()].id);
            }
        }
        Op::ClearHistory(i) => {
            let _ = core.history_clear(RANGES[*i], now);
        }
        Op::Import(text) => {
            let _ = core.bookmarks_import(text, now);
        }
        Op::History(on) => {
            let _ = core.settings_set(&json!({ "history": { "enabled": on } }));
        }
        Op::Zoom => {
            core.site_js_set("site1.i2p", true);
        }
    }
}

/// Req: IPC bookmarks/history: what the commands changed is what the next start reads.
fn same_after_reload(core: &Core, paths: &Paths) -> Result<(), TestCaseError> {
    let again = Core::new(Some(paths.clone()), "127.0.0.1:4444", 0);
    prop_assert_eq!(again.bookmarks_list(), core.bookmarks_list());
    prop_assert_eq!(all_history(&again), all_history(core));
    prop_assert_eq!(again.settings(), core.settings());
    Ok(())
}

proptest! {
    #![proptest_config(config(24))]

    // Req: IPC bookmarks and history commands: no sequence panics; every file on disk stays
    // valid JSON with a version; a restart reads back the same data.
    #[test]
    fn random_store_commands_persist(ops in prop::collection::vec(op(), 1..60)) {
        let paths = paths();
        let mut core = Core::new(Some(paths.clone()), "127.0.0.1:4444", 0);
        for (i, op) in ops.iter().enumerate() {
            apply(&mut core, op, 1_000 + i as u64);
            for file in files(&paths) {
                assert_versioned(file)?;
            }
        }
        same_after_reload(&core, &paths)?;
    }

    // Req: robustness brief "a corrupted file is handled without data loss beyond that file":
    // whatever one store file holds, the app starts with one tab, the other three stores keep
    // their data, and the corrupt store works again (it can be written and read back).
    #[test]
    fn a_corrupt_file_costs_only_that_file(which in 0..4usize, junk in corrupt_file()) {
        let paths = paths();
        let mut core = Core::new(Some(paths.clone()), "127.0.0.1:4444", 0);
        let _ = core.bookmark_add(&new_bookmark("http://keep.i2p/", "keep"), 1);
        visit(&mut core, 1, 2);
        // The site choice differs from the default (JS on), so the sites file keeps it.
        let _ = core.settings_set(&json!({ "keepCookies": true }));
        core.site_js_set("keep.i2p", false);
        let want_bookmarks = core.bookmarks_list();
        let want_history = all_history(&core);
        drop(core);
        let target = files(&paths)[which].clone();
        fs::write(&target, &junk).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let mut core = Core::new(Some(paths.clone()), "127.0.0.1:4444", 0);
        core.router_changed(ok_status());
        prop_assert_eq!(core.tabs().len(), 1);
        if which != 0 {
            prop_assert_eq!(core.bookmarks_list(), want_bookmarks);
        }
        if which != 1 {
            prop_assert_eq!(all_history(&core), want_history);
        }
        if which != 2 {
            prop_assert!(core.settings().keep_cookies, "settings lost");
        }
        if which != 3 {
            // Req: IPC `site_js_set`: the per-site choice is remembered across a restart.
            prop_assert_eq!(site_js(&mut core, "http://keep.i2p/"), Some(false), "sites lost");
        }
        let _ = core.bookmark_add(&new_bookmark("http://after.i2p/", "after"), 3);
        visit(&mut core, 2, 4);
        let _ = core.settings_set(&json!({ "keepCookies": false }));
        same_after_reload(&core, &paths)?;
    }
}

/// File contents a crash, a bad disk or an editor can leave.
fn corrupt_file() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..300),
        Just(Vec::new()),
        Just(b"{".to_vec()),
        Just(b"null".to_vec()),
        Just(b"[]".to_vec()),
        Just(b"{\"version\":1}".to_vec()),
        Just(b"{\"version\":\"x\",\"bookmarks\":5,\"entries\":{},\"settings\":[]}".to_vec()),
        Just(b"\xEF\xBB\xBF{\"version\":1,\"entries\":[]}".to_vec()),
        Just(vec![b'['; 100_000]),
        Just(b"{\"version\":1,\"entries\":[{\"id\":1}],\"bookmarks\":[{\"id\":1}]}".to_vec()),
    ]
}

// Req: IPC "History: At most 10 000 entries".
#[test]
fn history_never_holds_more_than_the_cap() {
    let mut core = Core::new(None, "127.0.0.1:4444", 0);
    core.router_changed(ok_status());
    for n in 0..CAP_PLUS {
        visit(&mut core, n, u64::from(n) + 1);
    }
    assert_eq!(all_history(&core).len(), HISTORY_CAP);
    assert_eq!(
        all_history(&core)[0].url,
        url_of(CAP_PLUS - 1),
        "newest first"
    );
}

// Req: IPC "History: At most 10 000 entries", also for a file that already holds more (an
// older version, a hand edit).
#[test]
fn an_oversized_history_file_is_cut_to_the_cap() {
    let paths = paths();
    let entries: Vec<Value> = (0..HISTORY_CAP + 2_000)
        .map(|n| json!({ "id": format!("{n:x}-1"), "url": url_of(u32::try_from(n).unwrap_or(0)), "title": "", "visited": n, "visits": 1 }))
        .collect();
    let text = json!({ "version": 1, "entries": entries }).to_string();
    fs::write(&paths.history, text).unwrap();
    let core = Core::new(Some(paths.clone()), "127.0.0.1:4444", 0);
    assert!(
        all_history(&core).len() <= HISTORY_CAP,
        "{} entries",
        all_history(&core).len()
    );
}

// Req: IPC "Nothing is recorded while `history.enabled` is false".
#[test]
fn history_off_records_nothing() {
    let mut core = Core::new(None, "127.0.0.1:4444", 0);
    core.router_changed(ok_status());
    core.settings_set(&json!({ "history": { "enabled": false } }))
        .unwrap();
    for n in 0..20 {
        visit(&mut core, n, u64::from(n) + 1);
    }
    assert!(all_history(&core).is_empty());
}

proptest! {
    #![proptest_config(config(64))]

    // Req: IPC `history_query`: "newest first, ordered by (visited desc, id desc)", `limit`
    // caps the answer, and the `before` cursor pages through the list with no entry twice
    // and none lost.
    #[test]
    fn history_pages_in_order(visits in prop::collection::vec((0..40u32, 0..50u64), 1..120), limit in 1..12usize) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        core.router_changed(ok_status());
        for (n, at) in &visits {
            visit(&mut core, *n, *at);
        }
        let all = all_history(&core);
        for pair in all.windows(2) {
            prop_assert!((pair[0].visited, &pair[0].id) >= (pair[1].visited, &pair[1].id));
        }
        let mut paged: Vec<HistoryEntry> = Vec::new();
        let mut before = None;
        loop {
            let page = core.history_query(&HistoryQuery { q: None, before: before.clone(), limit: Some(limit) });
            prop_assert!(page.len() <= limit);
            let Some(last) = page.last() else { break };
            before = Some(Cursor::Entry { visited: last.visited, id: last.id.clone() });
            paged.extend(page);
            prop_assert!(paged.len() <= all.len());
        }
        prop_assert_eq!(paged, all);
    }

    // Req: IPC `history_clear(range)`: the range removes what is newer than the range, keeps
    // what is older; "all" removes everything; no panic for any time.
    #[test]
    fn history_clear_by_range(times in prop::collection::vec(0..2_000_000_000u64, 1..30), range in 0..4usize) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        core.router_changed(ok_status());
        for (n, at) in times.iter().enumerate() {
            visit(&mut core, u32::try_from(n).unwrap_or(0), *at);
        }
        let now = 1_000_000_000u64;
        let before = all_history(&core);
        let _ = core.history_clear(RANGES[range], now);
        let after = all_history(&core);
        let window = [3_600_000u64, 86_400_000, 604_800_000, u64::MAX][range];
        for e in &before {
            let inside = e.visited > now.saturating_sub(window) && e.visited <= now;
            let kept = after.iter().any(|a| a.id == e.id);
            let older = e.visited.saturating_add(window) < now;
            prop_assert!(!older || kept || range == 3, "older entry {e:?} lost");
            prop_assert!(!(inside && kept) || (range != 3 && e.visited > now), "{e:?} survived {}", RANGES[range]);
        }
        if range == 3 {
            prop_assert!(after.is_empty());
        }
    }

    // Req: IPC `bookmarks_import(json) -> number`, `bookmarks_export() -> string (JSON)`: any
    // text is handled without a panic; a refused import changes nothing; an export imports
    // into a fresh browser and brings every address back.
    #[test]
    fn bookmark_import_and_export(text in import_text()) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        let before = core.bookmarks_list();
        let result = core.bookmarks_import(&text, 5);
        if result.is_err() {
            prop_assert_eq!(core.bookmarks_list(), before.clone());
        }
        let export = core.bookmarks_export();
        let mut fresh = Core::new(None, "127.0.0.1:4444", 0);
        prop_assert!(fresh.bookmarks_import(&export, 6).is_ok(), "own export refused: {export}");
        let urls = |list: Vec<Bookmark>| list.into_iter().map(|b| b.url).collect::<std::collections::BTreeSet<_>>();
        prop_assert_eq!(urls(fresh.bookmarks_list()), urls(core.bookmarks_list()));
    }

    // Req: IPC `suggest(input)`: at most 8 suggestions, never a panic.
    #[test]
    fn suggestions_are_at_most_eight(input in "\\PC{0,12}", visits in prop::collection::vec(0..40u32, 0..30)) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        core.router_changed(ok_status());
        for (i, n) in visits.iter().enumerate() {
            visit(&mut core, *n, i as u64);
        }
        prop_assert!(core.suggest(&input, 100).len() <= 8);
    }

    // Req: IPC `settings_set(patch)`: a bad patch is an error, not a panic; the settings that
    // come back stay usable (the zoom stays a finite positive factor).
    #[test]
    fn settings_patches_never_panic(patch in patch_value()) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        core.router_changed(ok_status());
        let before = core.settings().clone();
        match core.settings_set(&patch) {
            Err(_) => prop_assert_eq!(core.settings(), &before),
            Ok((settings, _)) => prop_assert!(settings.zoom_default.is_finite() && settings.zoom_default > 0.0),
        }
        for info in core.tab_infos() {
            prop_assert!(info.zoom.is_finite() && info.zoom > 0.0);
        }
    }
}

fn patch_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(json!({ "zoomDefault": 0 })),
        Just(json!({ "zoomDefault": -3.5 })),
        Just(json!({ "zoomDefault": 1e300 })),
        Just(json!({ "zoomDefault": "big" })),
        Just(json!({ "homepage": "http://example.com/" })),
        Just(json!({ "homepage": "javascript:alert(1)" })),
        Just(json!({ "homepage": "" })),
        Just(json!({ "theme": "neon" })),
        Just(json!({ "history": 5 })),
        Just(json!([1, 2])),
        Just(json!(null)),
        "\\PC{0,20}".prop_map(|s| json!({ "homepage": s })),
        "\\PC{0,20}".prop_map(|s| json!({ s: 1 })),
    ]
}

// Req: ADR Goal "no single mistake … may open a clearnet path": a settings file that names a
// clearnet homepage (hand edit, damage) never makes a tab webview load a clearnet URL.
#[test]
fn a_clearnet_homepage_in_the_settings_file_never_loads() {
    for homepage in [
        "http://example.com/",
        "https://127.0.0.1:7657/",
        "file:///etc/passwd",
        "javascript:alert(1)",
    ] {
        let paths = paths();
        let text = json!({ "version": 1, "settings": { "homepage": homepage, "theme": "system",
            "jsDefault": true, "history": { "enabled": true }, "keepCookies": false, "zoomDefault": 1.0 } });
        fs::write(&paths.settings, text.to_string()).unwrap();
        let mut core = Core::new(Some(paths.clone()), "127.0.0.1:4444", 0);
        let mut fx = core.router_changed(ok_status());
        fx.extend(core.tab_new(None, Place::End).1);
        fx.extend(core.home());
        let id = core.tabs().active_id();
        fx.extend(core.tab_close(id));
        for url in load_urls(&fx) {
            assert!(
                Url::parse(&url).is_ok_and(|u| is_allowed_web(&u)),
                "{homepage}: Load of {url}"
            );
        }
    }
}

fn load_urls(fx: &[Effect]) -> Vec<String> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Web(WebOp::Load(load)) => Some(load.url.clone()),
            _ => None,
        })
        .collect()
}

proptest! {
    #![proptest_config(config(128))]

    // Req: PO decision "bookmark_add of a non-I2P URL is refused with reason not-i2p": clearnet,
    // loopback, userinfo tricks, IDN and dangerous schemes are refused and nothing is saved.
    #[test]
    fn bookmarks_refuse_non_i2p_urls(url in non_i2p_url()) {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        let before = core.bookmarks_list();
        let result = core.bookmark_add(&new_bookmark(&url, "t"), 1);
        match result {
            Err(reason) => prop_assert!(reason.contains("not-i2p"), "reason {reason:?} for {url}"),
            Ok(_) => prop_assert!(false, "{url} was bookmarked"),
        }
        prop_assert_eq!(core.bookmarks_list(), before);
    }

    // Req: IPC `bookmark_add({url, title, folder?}) -> Bookmark`: an I2P address is saved, and
    // `bookmark_find` finds it again.
    #[test]
    fn bookmarks_accept_i2p_urls(host in "[a-z0-9]{1,8}\\.i2p", path in "(/[a-z0-9]{0,6}){0,2}") {
        let mut core = Core::new(None, "127.0.0.1:4444", 0);
        let url = format!("http://{host}{path}");
        let (bookmark, _) = core.bookmark_add(&new_bookmark(&url, "t"), 1).map_err(TestCaseError::fail)?;
        prop_assert_eq!(core.bookmark_find(&url), Some(bookmark));
    }
}

fn non_i2p_url() -> impl Strategy<Value = String> {
    prop_oneof![
        "https?://[a-z0-9]{1,8}\\.(com|org|net|onion)(/[a-z]{0,5})?",
        "https?://([0-9]{1,3}\\.){3}[0-9]{1,3}(:[0-9]{1,5})?/",
        "http://[a-z0-9]{1,8}@[a-z0-9]{1,8}\\.com/#\\.i2p",
        "http://[a-z0-9]{1,8}\\.i2p@[a-z0-9]{1,8}\\.com/",
        "(javascript|data|file|ftp|about):[ -~]{0,20}",
        Just("http://[::1]:7657/".to_owned()),
        Just("http://xn--bcher-kva.i2p/".to_owned()),
        Just("http://b\u{fc}cher.i2p/".to_owned()),
    ]
}
