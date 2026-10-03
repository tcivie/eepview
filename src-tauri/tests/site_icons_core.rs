// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Site icons, state machine side (R6 to R10, R22, R24 to R31 of `docs/wiki/site-icons.md`).
//!
//! `Core` gets page events and library commands, and returns effects. The tests read the
//! `FetchIcon` effects, the events, the `icon` fields, and the files in the `icons/` folder.
//! Expected hashes come from `shasum -a 256` of the host text, not from the code.

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};

use eepview_lib::core::{Core, Effect, Event, Paths};
use eepview_lib::icons::{Icon, data_url};
use eepview_lib::tabs::Place;
use eepview_lib::types::{HistoryQuery, NewBookmark, RouterStatus};
use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};

type Res<T> = Result<T, Box<dyn Error>>;

const T0: u64 = 1_000_000_000_000;
const HOUR: u64 = 60 * 60 * 1000;
const DAY: u64 = 24 * HOUR;

const ALPHA: &str = "alpha.i2p";
const ALPHA_STEM: &str = "3cb7d940c289d5f66ec11ccd02197102e5d1ecf80b0ff3bfb8a491910238ba26";
const BETA: &str = "beta.i2p";
const BETA_STEM: &str = "1c39035de7259a06bf73927f988ba408c7c6cd55981b782b438ae239f23f779f";
const GAMMA: &str = "gamma.i2p";
const GAMMA_STEM: &str = "183caef14feddbc751e56268f29440032956878da6b9d0008025c49b88ef024f";
const DELTA: &str = "delta.i2p";
const GHOST_STEM: &str = "664858eb5592751cac9cdf2baf771cf69bfb056e2015b0bb9185bf406d8c4289";

static NEXT: AtomicU32 = AtomicU32::new(0);

/// What keeps a host alive in the library.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Keep {
    History,
    Bookmark,
    Both,
}

fn fresh_paths(name: &str) -> Res<Paths> {
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "eepview-icons-core-{}-{name}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    Ok(Paths {
        bookmarks: dir.join("bookmarks.json"),
        history: dir.join("history.json"),
        settings: dir.join("settings.json"),
        sites: dir.join("sites.json"),
        icons: dir.join("icons"),
    })
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

/// A core on `paths` with a verified router.
fn open(paths: &Paths) -> Core {
    let mut core = Core::new(Some(paths.clone()), "127.0.0.1:4444", 0);
    core.router_changed(ok_status());
    core
}

fn png(px: u32) -> Res<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    RgbaImage::from_pixel(px, px, Rgba([10, 200, 30, 255])).write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}

/// A valid icon: a 32 px and a 64 px PNG.
fn icon() -> Res<Icon> {
    Ok(Icon {
        small: png(32)?,
        large: png(64)?,
    })
}

/// Loads `http://host/` in the active tab and finishes the load at `now`.
fn visit(core: &mut Core, host: &str, now: u64) -> Vec<Effect> {
    let url = format!("http://{host}/");
    core.navigate(&url);
    let id = core.tabs().active_id();
    core.page_started(id, &url);
    core.page_finished(id, &url, now)
}

/// The hosts of the `FetchIcon` effects.
fn fetches(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::FetchIcon(host) => Some(host.clone()),
            _ => None,
        })
        .collect()
}

fn has_event(effects: &[Effect], want: &Event) -> bool {
    effects.contains(&Effect::Emit(want.clone()))
}

/// R30: the effects carry both events.
fn assert_icons_changed(effects: &[Effect]) {
    assert!(
        has_event(effects, &Event::Icons),
        "R30: icons-changed missing"
    );
    assert!(
        has_event(effects, &Event::TabsChanged),
        "R30: tabs-changed missing"
    );
}

fn bookmark(core: &mut Core, host: &str) -> Res<()> {
    let new = NewBookmark {
        url: host.to_owned(),
        title: host.to_owned(),
        folder: None,
    };
    core.bookmark_add(&new, T0)?;
    Ok(())
}

/// The ids of the history entries on `host`.
fn history_ids(core: &Core, host: &str) -> Vec<String> {
    let needle = format!("http://{host}/");
    let all = core.history_query(&HistoryQuery::default());
    all.into_iter()
        .filter(|e| e.url == needle)
        .map(|e| e.id)
        .collect()
}

fn bookmark_id(core: &Core, host: &str) -> Res<String> {
    let found = core.bookmark_find(&format!("http://{host}/"));
    Ok(found.ok_or("no bookmark")?.id)
}

/// Visits `host`, answers the fetch with a valid icon, and leaves the host alive by `keep`.
fn add_site(core: &mut Core, host: &str, keep: Keep) -> Res<()> {
    if keep != Keep::History {
        bookmark(core, host)?;
    }
    let fx = visit(core, host, T0);
    assert_eq!(fetches(&fx), [host], "the visit asks for the icon");
    let fx = core.icon_fetched(host, Some(icon()?), T0);
    assert_icons_changed(&fx);
    if keep == Keep::Bookmark {
        for id in history_ids(core, host) {
            core.history_remove(&id);
        }
    }
    Ok(())
}

/// The file names in the icons folder, sorted (empty when there is no folder).
fn files(paths: &Paths) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(&paths.icons)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// True when both icon files of the host hash `stem` exist.
fn has_files(paths: &Paths, stem: &str) -> bool {
    let list = files(paths);
    list.contains(&format!("{stem}.png")) && list.contains(&format!("{stem}-32.png"))
}

/// True when any file or `attempts.json` still names the host hash `stem`.
fn mentions(paths: &Paths, stem: &str) -> bool {
    let attempts = fs::read_to_string(paths.icons.join("attempts.json")).unwrap_or_default();
    attempts.contains(stem) || files(paths).iter().any(|n| n.contains(stem))
}

fn tab_icon(core: &Core) -> Option<String> {
    core.tab_info(core.tabs().active_id()).and_then(|t| t.icon)
}

#[test]
fn r6_a_finished_web_page_on_a_bookmarked_host_starts_one_fetch() -> Res<()> {
    // R6, R7: the page finished, the host has a bookmark.
    let mut core = open(&fresh_paths("r6-start")?);
    bookmark(&mut core, ALPHA)?;
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0)), [ALPHA]);
    Ok(())
}

#[test]
fn r6_a_page_that_never_finishes_starts_no_fetch() -> Res<()> {
    // R6: only a finished load starts a fetch.
    let mut core = open(&fresh_paths("r6-unfinished")?);
    bookmark(&mut core, ALPHA)?;
    let url = format!("http://{ALPHA}/");
    let (_, nav) = core.navigate(&url);
    let id = core.tabs().active_id();
    assert!(fetches(&nav).is_empty());
    assert!(fetches(&core.page_started(id, &url)).is_empty());
    Ok(())
}

#[test]
fn r6_internal_blank_and_blocked_pages_start_no_fetch() -> Res<()> {
    // R6: no fetch for `eepview://` pages, `about:blank` or a blocked page.
    let mut core = open(&fresh_paths("r6-internal")?);
    bookmark(&mut core, ALPHA)?;
    let id = core.tabs().active_id();
    assert!(fetches(&core.page_finished(id, "eepview://home", T0)).is_empty());
    assert!(fetches(&core.page_finished(id, "about:blank", T0)).is_empty());
    core.navigate("http://example.com/");
    let id = core.tabs().active_id();
    assert!(fetches(&core.page_finished(id, "http://example.com/", T0)).is_empty());
    core.navigate("eepview://settings");
    let id = core.tabs().active_id();
    assert!(fetches(&core.page_finished(id, "eepview://settings", T0)).is_empty());
    Ok(())
}

#[test]
fn r7_a_history_entry_alone_is_enough() -> Res<()> {
    // R7: no bookmark, but the visit is recorded in the history.
    let mut core = open(&fresh_paths("r7-history")?);
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0)), [ALPHA]);
    Ok(())
}

#[test]
fn r7_history_off_and_no_bookmark_fetches_and_stores_nothing() -> Res<()> {
    // R7: nothing is fetched and nothing is stored.
    let paths = fresh_paths("r7-off")?;
    let mut core = open(&paths);
    core.settings_set(&json!({"history": {"enabled": false}}))?;
    assert!(fetches(&visit(&mut core, ALPHA, T0)).is_empty());
    assert!(!mentions(&paths, ALPHA_STEM));
    assert!(tab_icon(&core).is_none());
    Ok(())
}

#[test]
fn r7_history_off_with_a_bookmark_still_fetches() -> Res<()> {
    // R7: the bookmark is enough.
    let mut core = open(&fresh_paths("r7-bookmark")?);
    core.settings_set(&json!({"history": {"enabled": false}}))?;
    bookmark(&mut core, ALPHA)?;
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0)), [ALPHA]);
    Ok(())
}

#[test]
fn r8_a_host_is_asked_once_in_24_hours_whatever_the_outcome() -> Res<()> {
    // R8: the attempt worked, or it failed: the next load inside 24 h asks nothing.
    for worked in [true, false] {
        let mut core = open(&fresh_paths("r8-once")?);
        assert_eq!(fetches(&visit(&mut core, ALPHA, T0)), [ALPHA]);
        core.icon_fetched(ALPHA, worked.then(icon).transpose()?, T0);
        assert!(
            fetches(&visit(&mut core, ALPHA, T0 + HOUR)).is_empty(),
            "{worked}"
        );
        assert!(
            fetches(&visit(&mut core, ALPHA, T0 + 23 * HOUR)).is_empty(),
            "{worked}"
        );
    }
    Ok(())
}

#[test]
fn r8_a_host_whose_fetch_is_still_running_is_not_asked_again() -> Res<()> {
    // R8, R10: each host at most once.
    let mut core = open(&fresh_paths("r8-running")?);
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0)), [ALPHA]);
    assert!(fetches(&visit(&mut core, ALPHA, T0 + 1000)).is_empty());
    Ok(())
}

#[test]
fn r8_the_attempt_time_survives_a_restart() -> Res<()> {
    // R8: after a restart the host is still not asked inside 24 h.
    for worked in [true, false] {
        let paths = fresh_paths("r8-restart")?;
        let mut core = open(&paths);
        visit(&mut core, ALPHA, T0);
        core.icon_fetched(ALPHA, worked.then(icon).transpose()?, T0);
        drop(core);
        let mut again = open(&paths);
        assert!(
            fetches(&visit(&mut again, ALPHA, T0 + 2 * HOUR)).is_empty(),
            "{worked}"
        );
        let later = fetches(&visit(&mut again, ALPHA, T0 + DAY + HOUR));
        assert_eq!(later, [ALPHA], "R9 after a restart, {worked}");
    }
    Ok(())
}

#[test]
fn r9_after_24_hours_the_next_page_load_asks_again() -> Res<()> {
    // R9: 24 h and a little later.
    let mut core = open(&fresh_paths("r9-again")?);
    visit(&mut core, ALPHA, T0);
    core.icon_fetched(ALPHA, Some(icon()?), T0);
    assert!(fetches(&visit(&mut core, ALPHA, T0 + DAY - 1000)).is_empty());
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0 + DAY + 1000)), [ALPHA]);
    Ok(())
}

#[test]
fn r9_r25_a_failed_second_attempt_keeps_the_stored_icon() -> Res<()> {
    // R9, R25: the retry fails; tab, bookmark, history and files stay as they were.
    let paths = fresh_paths("r9-fail")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Both)?;
    visit(&mut core, ALPHA, T0 + DAY + 1000);
    let before = fs::read(paths.icons.join(format!("{ALPHA_STEM}.png")))?;
    core.icon_fetched(ALPHA, None, T0 + DAY + 2000);
    assert_eq!(tab_icon(&core), Some(data_url(&icon()?.small)));
    assert_eq!(
        fs::read(paths.icons.join(format!("{ALPHA_STEM}.png")))?,
        before
    );
    assert!(has_files(&paths, ALPHA_STEM));
    Ok(())
}

/// Starts the fetches of `hosts` in order, without answering any. Returns the effects of each.
fn start_all(core: &mut Core, hosts: &[&str]) -> Vec<Vec<String>> {
    hosts
        .iter()
        .enumerate()
        .map(|(i, host)| fetches(&visit(core, host, T0 + u64::try_from(i).unwrap_or(0))))
        .collect()
}

#[test]
fn r10_at_most_two_fetches_run_at_the_same_time() -> Res<()> {
    // R10: the third and fourth host wait.
    let mut core = open(&fresh_paths("r10-two")?);
    let started = start_all(&mut core, &[ALPHA, BETA, GAMMA, DELTA]);
    assert_eq!(started[0], [ALPHA]);
    assert_eq!(started[1], [BETA]);
    assert!(
        started[2].is_empty() && started[3].is_empty(),
        "{started:?}"
    );
    Ok(())
}

#[test]
fn r10_a_waiting_host_starts_when_a_running_fetch_ends_in_order() -> Res<()> {
    // R10: gamma before delta; each ending fetch frees one slot.
    let mut core = open(&fresh_paths("r10-queue")?);
    start_all(&mut core, &[ALPHA, BETA, GAMMA, DELTA]);
    let first = core.icon_fetched(ALPHA, Some(icon()?), T0 + 10);
    assert_eq!(fetches(&first), [GAMMA]);
    let second = core.icon_fetched(BETA, None, T0 + 11);
    assert_eq!(fetches(&second), [DELTA]);
    assert!(fetches(&core.icon_fetched(GAMMA, None, T0 + 12)).is_empty());
    assert!(fetches(&core.icon_fetched(DELTA, None, T0 + 13)).is_empty());
    Ok(())
}

#[test]
fn r10_a_failed_fetch_also_frees_its_slot() -> Res<()> {
    // R10: `None` ends a fetch too.
    let mut core = open(&fresh_paths("r10-fail")?);
    start_all(&mut core, &[ALPHA, BETA, GAMMA]);
    assert_eq!(fetches(&core.icon_fetched(BETA, None, T0 + 10)), [GAMMA]);
    Ok(())
}

#[test]
fn r10_a_host_waits_in_the_queue_at_most_once() -> Res<()> {
    // R10: gamma finishes loading twice while it waits; it starts once.
    let mut core = open(&fresh_paths("r10-once")?);
    start_all(&mut core, &[ALPHA, BETA, GAMMA]);
    assert!(fetches(&visit(&mut core, GAMMA, T0 + 5)).is_empty());
    assert_eq!(fetches(&core.icon_fetched(ALPHA, None, T0 + 10)), [GAMMA]);
    assert!(fetches(&core.icon_fetched(BETA, None, T0 + 11)).is_empty());
    Ok(())
}

#[test]
fn r22_a_stored_icon_is_two_files_named_by_the_hash_and_nothing_names_the_host() -> Res<()> {
    // R22, R23: <hash>.png (64 px), <hash>-32.png (32 px), attempts.json.
    let paths = fresh_paths("r22")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::History)?;
    let want = icon()?;
    assert_eq!(
        fs::read(paths.icons.join(format!("{ALPHA_STEM}.png")))?,
        want.large
    );
    assert_eq!(
        fs::read(paths.icons.join(format!("{ALPHA_STEM}-32.png")))?,
        want.small
    );
    for name in files(&paths) {
        assert!(!name.contains("alpha") && !name.contains("i2p"), "{name}");
    }
    let attempts = fs::read_to_string(paths.icons.join("attempts.json"))?;
    assert!(!attempts.contains("alpha") && !attempts.contains("i2p"));
    Ok(())
}

/// Every value under an `icon` key in `json`.
fn icon_values<'a>(json: &'a Value, out: &mut Vec<&'a Value>) {
    match json {
        Value::Object(map) => map
            .iter()
            .for_each(|(key, value)| icon_entry(key, value, out)),
        Value::Array(items) => items.iter().for_each(|v| icon_values(v, out)),
        _ => {}
    }
}

fn icon_entry<'a>(key: &str, value: &'a Value, out: &mut Vec<&'a Value>) {
    if key == "icon" {
        out.push(value);
    }
    icon_values(value, out);
}

/// R24: no icon data in a library file or export. An `icon` field is absent or null.
fn assert_no_icon_data(text: &str) -> Res<()> {
    assert!(!text.contains("data:image"), "R24: icon data in {text}");
    let json: Value = serde_json::from_str(text)?;
    let mut values = Vec::new();
    icon_values(&json, &mut values);
    assert!(values.iter().all(|v| v.is_null()), "R24: {values:?}");
    Ok(())
}

#[test]
fn r24_bookmarks_history_and_the_export_never_contain_icon_data() -> Res<()> {
    // R24: the icon is on screen, not in the files.
    let paths = fresh_paths("r24-files")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Both)?;
    assert!(core.bookmarks_list().iter().any(|b| b.icon.is_some()));
    assert_no_icon_data(&fs::read_to_string(&paths.bookmarks)?)?;
    assert_no_icon_data(&fs::read_to_string(&paths.history)?)?;
    assert_no_icon_data(&core.bookmarks_export())?;
    Ok(())
}

#[test]
fn r24_an_icon_sent_with_bookmark_update_is_ignored() -> Res<()> {
    // R24: `bookmark_update` ignores an `icon` it receives.
    let paths = fresh_paths("r24-update")?;
    let mut core = open(&paths);
    bookmark(&mut core, ALPHA)?;
    let mut sent = core
        .bookmark_find(&format!("http://{ALPHA}/"))
        .ok_or("no bookmark")?;
    sent.icon = Some("data:image/png;base64,SECRETAAAA".into());
    sent.title = "Renamed".into();
    core.bookmark_update(&sent);
    let text = fs::read_to_string(&paths.bookmarks)?;
    assert!(
        text.contains("Renamed") && !text.contains("SECRETAAAA"),
        "{text}"
    );
    let now = core
        .bookmark_find(&format!("http://{ALPHA}/"))
        .ok_or("no bookmark")?;
    assert_ne!(
        now.icon.as_deref(),
        Some("data:image/png;base64,SECRETAAAA")
    );
    Ok(())
}

#[test]
fn r24_an_icon_in_an_imported_bookmark_is_ignored() -> Res<()> {
    // R24: `bookmarks_import` ignores an `icon` it receives.
    let paths = fresh_paths("r24-import")?;
    let mut core = open(&paths);
    let json = r#"[{"id":"imp1","url":"http://gamma.i2p/","title":"G","icon":"data:image/png;base64,SECRETBBBB"}]"#;
    let (added, _) = core.bookmarks_import(json, T0)?;
    assert_eq!(added, 1);
    assert!(!fs::read_to_string(&paths.bookmarks)?.contains("SECRETBBBB"));
    assert!(!core.bookmarks_export().contains("SECRETBBBB"));
    let got = core
        .bookmark_find("http://gamma.i2p/")
        .ok_or("not imported")?;
    assert_eq!(got.icon, None);
    Ok(())
}

#[test]
fn r25_a_failed_fetch_never_changes_an_icon_on_disk() -> Res<()> {
    // R25: failures after a good fetch leave both files as they were.
    let paths = fresh_paths("r25")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Both)?;
    let (big, small) = (
        fs::read(paths.icons.join(format!("{ALPHA_STEM}.png")))?,
        fs::read(paths.icons.join(format!("{ALPHA_STEM}-32.png")))?,
    );
    for round in 1..=3 {
        visit(&mut core, ALPHA, T0 + round * (DAY + HOUR));
        core.icon_fetched(ALPHA, None, T0 + round * (DAY + HOUR));
    }
    assert_eq!(
        fs::read(paths.icons.join(format!("{ALPHA_STEM}.png")))?,
        big
    );
    assert_eq!(
        fs::read(paths.icons.join(format!("{ALPHA_STEM}-32.png")))?,
        small
    );
    Ok(())
}

#[test]
fn r26_removing_the_last_history_entry_deletes_the_icon_and_the_attempt() -> Res<()> {
    // R26, R30: `history_remove` of the last entry.
    let paths = fresh_paths("r26-remove")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::History)?;
    let ids = history_ids(&core, ALPHA);
    assert_eq!(ids.len(), 1);
    let fx = core.history_remove(&ids[0]);
    assert_icons_changed(&fx);
    assert!(!mentions(&paths, ALPHA_STEM), "{:?}", files(&paths));
    assert_eq!(tab_icon(&core), None);
    Ok(())
}

#[test]
fn r26_the_icon_stays_while_another_history_entry_of_the_host_stays() -> Res<()> {
    // R26: two pages of one host; removing one keeps the icon, removing both deletes it.
    let paths = fresh_paths("r26-two")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::History)?;
    let url = format!("http://{ALPHA}/other");
    core.navigate(&url);
    let id = core.tabs().active_id();
    core.page_started(id, &url);
    core.page_finished(id, &url, T0 + 5);
    let all = core.history_query(&HistoryQuery::default());
    assert_eq!(all.len(), 2, "{all:?}");
    core.history_remove(&all[0].id);
    assert!(has_files(&paths, ALPHA_STEM), "one entry is left");
    core.history_remove(&all[1].id);
    assert!(!mentions(&paths, ALPHA_STEM));
    Ok(())
}

#[test]
fn r26_the_icon_stays_while_the_host_has_a_bookmark() -> Res<()> {
    // R26: history goes, the bookmark keeps the icon and the 64 px icon shows.
    let paths = fresh_paths("r26-bookmark-keeps")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Both)?;
    for id in history_ids(&core, ALPHA) {
        core.history_remove(&id);
    }
    assert!(has_files(&paths, ALPHA_STEM));
    let marked = core
        .bookmark_find(&format!("http://{ALPHA}/"))
        .ok_or("no bookmark")?;
    assert_eq!(marked.icon, Some(data_url(&icon()?.large)));
    Ok(())
}

#[test]
fn r26_removing_the_last_bookmark_deletes_the_icon() -> Res<()> {
    // R26, R30: `bookmark_remove` of a bookmark-only host.
    let paths = fresh_paths("r26-bookmark-remove")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Bookmark)?;
    assert!(has_files(&paths, ALPHA_STEM));
    let fx = core.bookmark_remove(&bookmark_id(&core, ALPHA)?);
    assert_icons_changed(&fx);
    assert!(!mentions(&paths, ALPHA_STEM));
    Ok(())
}

#[test]
fn r26_removing_a_bookmark_keeps_the_icon_of_a_host_with_history() -> Res<()> {
    // R26: the history entry still keeps the host alive.
    let paths = fresh_paths("r26-bookmark-history")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Both)?;
    core.bookmark_remove(&bookmark_id(&core, ALPHA)?);
    assert!(has_files(&paths, ALPHA_STEM));
    Ok(())
}

#[test]
fn r26_moving_a_bookmark_to_another_host_deletes_the_icon_of_the_old_host() -> Res<()> {
    // R26, R30: `bookmark_update` with a new URL on another host.
    let paths = fresh_paths("r26-move")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Bookmark)?;
    let mut moved = core
        .bookmark_find(&format!("http://{ALPHA}/"))
        .ok_or("no bookmark")?;
    moved.url = BETA.to_owned();
    let fx = core.bookmark_update(&moved);
    assert_icons_changed(&fx);
    assert!(!mentions(&paths, ALPHA_STEM));
    assert!(
        !has_files(&paths, BETA_STEM),
        "the new host has no icon yet"
    );
    Ok(())
}

#[test]
fn r26_a_bookmark_edit_on_the_same_host_keeps_the_icon() -> Res<()> {
    // R26: only a move to another host clears.
    let paths = fresh_paths("r26-rename")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Bookmark)?;
    let mut edited = core
        .bookmark_find(&format!("http://{ALPHA}/"))
        .ok_or("no bookmark")?;
    edited.title = "New title".into();
    core.bookmark_update(&edited);
    assert!(has_files(&paths, ALPHA_STEM));
    Ok(())
}

#[test]
fn r26_history_clear_deletes_the_icons_of_the_cleared_entries_for_any_range() -> Res<()> {
    // R26, R30: hour, day, week and all each clear a visit made a moment ago.
    for range in ["hour", "day", "week", "all"] {
        let paths = fresh_paths("r26-clear")?;
        let mut core = open(&paths);
        add_site(&mut core, ALPHA, Keep::History)?;
        let fx = core.history_clear(range, T0 + 1000)?;
        assert_icons_changed(&fx);
        assert!(!mentions(&paths, ALPHA_STEM), "{range}");
    }
    Ok(())
}

#[test]
fn r26_history_clear_keeps_the_icon_of_an_entry_older_than_the_range() -> Res<()> {
    // R26: the entry is 3 hours old; clearing the last hour keeps it, and its icon.
    let paths = fresh_paths("r26-old")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::History)?;
    core.history_clear("hour", T0 + 3 * HOUR)?;
    assert!(has_files(&paths, ALPHA_STEM));
    core.history_clear("day", T0 + 3 * HOUR)?;
    assert!(!mentions(&paths, ALPHA_STEM));
    Ok(())
}

#[test]
fn r27_clear_all_deletes_every_icon_without_a_bookmark_and_keeps_bookmarked_ones() -> Res<()> {
    // R27: alpha (history only) goes, beta (bookmark and history) stays.
    let paths = fresh_paths("r27")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::History)?;
    add_site(&mut core, BETA, Keep::Both)?;
    let fx = core.history_clear("all", T0 + 1000)?;
    assert_icons_changed(&fx);
    assert!(!mentions(&paths, ALPHA_STEM));
    assert!(has_files(&paths, BETA_STEM));
    assert_eq!(
        core.bookmark_find(&format!("http://{BETA}/"))
            .and_then(|b| b.icon),
        Some(data_url(&icon()?.large))
    );
    Ok(())
}

#[test]
fn r28_start_up_deletes_files_that_belong_to_no_host_and_files_it_did_not_write() -> Res<()> {
    // R28: alpha is visited, beta is bookmarked; the strays go, both icons stay.
    let paths = fresh_paths("r28")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::History)?;
    add_site(&mut core, BETA, Keep::Bookmark)?;
    drop(core);
    for stray in [
        "notes.txt".to_owned(),
        "pixel.png".to_owned(),
        format!("{GHOST_STEM}.png"),
        format!("{GHOST_STEM}-32.png"),
        format!("{GAMMA_STEM}.png"),
    ] {
        fs::write(paths.icons.join(stray), b"stray")?;
    }
    let reopened = open(&paths);
    assert!(has_files(&paths, ALPHA_STEM) && has_files(&paths, BETA_STEM));
    for name in files(&paths) {
        let known =
            name == "attempts.json" || name.starts_with(ALPHA_STEM) || name.starts_with(BETA_STEM);
        assert!(known, "R28: {name} was not deleted");
    }
    assert_eq!(
        reopened
            .bookmark_find(&format!("http://{BETA}/"))
            .and_then(|b| b.icon),
        Some(data_url(&icon()?.large))
    );
    Ok(())
}

#[test]
fn r28_start_up_deletes_the_icon_of_a_host_whose_history_is_gone() -> Res<()> {
    // R28: the history file no longer names alpha, so its icon goes at the next start.
    let paths = fresh_paths("r28-history")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::History)?;
    drop(core);
    fs::remove_file(&paths.history)?;
    let _core = open(&paths);
    assert!(!mentions(&paths, ALPHA_STEM));
    Ok(())
}

#[test]
fn r29_after_a_host_is_cleared_the_next_visit_asks_again() -> Res<()> {
    // R29: the 24 h rule starts fresh.
    let paths = fresh_paths("r29")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::History)?;
    core.history_clear("all", T0 + 1000)?;
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0 + 2000)), [ALPHA]);
    Ok(())
}

#[test]
fn r30_a_stored_icon_sends_icons_changed_and_tabs_changed() -> Res<()> {
    // R30: the effects of `icon_fetched` with an icon.
    let mut core = open(&fresh_paths("r30")?);
    visit(&mut core, ALPHA, T0);
    assert_icons_changed(&core.icon_fetched(ALPHA, Some(icon()?), T0 + 5));
    Ok(())
}

#[test]
fn r26_a_fetch_that_ends_after_its_host_left_the_library_stores_nothing() -> Res<()> {
    // R26: the host is cleared while the fetch runs. The late icon is not kept.
    let paths = fresh_paths("r26-late")?;
    let mut core = open(&paths);
    visit(&mut core, ALPHA, T0);
    core.history_clear("all", T0 + 10)?;
    core.icon_fetched(ALPHA, Some(icon()?), T0 + 20);
    assert!(!has_files(&paths, ALPHA_STEM));
    assert_eq!(tab_icon(&core), None);
    Ok(())
}

#[test]
fn r31_the_tab_and_the_history_row_get_the_32_px_icon_and_the_bookmark_the_64_px_icon() -> Res<()> {
    // R31: `data:image/png;base64,...`, small for tabs and history, large for bookmarks.
    let mut core = open(&fresh_paths("r31")?);
    add_site(&mut core, ALPHA, Keep::Both)?;
    let want = icon()?;
    let small = data_url(&want.small);
    assert!(small.starts_with("data:image/png;base64,"));
    assert_eq!(tab_icon(&core), Some(small.clone()));
    let history = core.history_query(&HistoryQuery::default());
    assert_eq!(history.first().and_then(|e| e.icon.clone()), Some(small));
    let bookmarks = core.bookmarks_list();
    let marked = bookmarks
        .iter()
        .find(|b| b.url.contains(ALPHA))
        .ok_or("no bookmark")?;
    assert_eq!(marked.icon, Some(data_url(&want.large)));
    Ok(())
}

#[test]
fn r31_without_a_stored_icon_every_value_is_null() -> Res<()> {
    // R31: no icon, no value.
    let mut core = open(&fresh_paths("r31-none")?);
    visit(&mut core, ALPHA, T0);
    assert_eq!(tab_icon(&core), None);
    assert!(
        core.history_query(&HistoryQuery::default())
            .iter()
            .all(|e| e.icon.is_none())
    );
    assert!(core.bookmarks_list().iter().all(|b| b.icon.is_none()));
    Ok(())
}

#[test]
fn r31_internal_tabs_always_have_a_null_icon() -> Res<()> {
    // R31: an internal tab, with icons stored for other hosts.
    let mut core = open(&fresh_paths("r31-internal")?);
    add_site(&mut core, ALPHA, Keep::Both)?;
    let (info, _) = core.tab_new(Some("eepview://settings"), Place::End);
    let info = info.ok_or("no tab")?;
    assert_eq!(info.kind, "internal");
    assert_eq!(info.icon, None);
    for tab in core.tab_infos().iter().filter(|t| t.kind == "internal") {
        assert_eq!(tab.icon, None);
    }
    Ok(())
}

#[test]
fn r11_an_attempt_that_never_reached_the_site_does_not_count() -> Res<()> {
    // R11: after `icon_unreached` the next page load inside 24 h asks again.
    let mut core = open(&fresh_paths("r11-again")?);
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0)), [ALPHA]);
    core.icon_unreached(ALPHA, T0 + 10);
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0 + HOUR)), [ALPHA]);
    Ok(())
}

#[test]
fn r11_unreached_and_failed_differ_in_what_the_next_load_does() -> Res<()> {
    // R11: `icon_fetched(None, ...)` still counts; `icon_unreached` does not.
    let mut counted = open(&fresh_paths("r11-counted")?);
    visit(&mut counted, ALPHA, T0);
    counted.icon_fetched(ALPHA, None, T0 + 10);
    assert!(fetches(&visit(&mut counted, ALPHA, T0 + HOUR)).is_empty());
    let mut skipped = open(&fresh_paths("r11-skipped")?);
    visit(&mut skipped, ALPHA, T0);
    skipped.icon_unreached(ALPHA, T0 + 10);
    assert_eq!(fetches(&visit(&mut skipped, ALPHA, T0 + HOUR)), [ALPHA]);
    Ok(())
}

#[test]
fn r11_the_attempt_time_from_before_comes_back() -> Res<()> {
    // R11: a good fetch at T0, a retry after 25 h that never reached the site. The time
    // of T0 is back, so it is still expired, and the next load asks again.
    let mut core = open(&fresh_paths("r11-before")?);
    visit(&mut core, ALPHA, T0);
    core.icon_fetched(ALPHA, Some(icon()?), T0);
    let retry = T0 + DAY + HOUR;
    assert_eq!(fetches(&visit(&mut core, ALPHA, retry)), [ALPHA]);
    core.icon_unreached(ALPHA, retry + 10);
    assert_eq!(fetches(&visit(&mut core, ALPHA, retry + HOUR)), [ALPHA]);
    Ok(())
}

#[test]
fn r11_an_unreached_attempt_leaves_no_trace_after_a_restart() -> Res<()> {
    // R11, R8: the restored time is what the disk holds.
    let paths = fresh_paths("r11-restart")?;
    let mut core = open(&paths);
    visit(&mut core, ALPHA, T0);
    core.icon_unreached(ALPHA, T0 + 10);
    drop(core);
    let mut again = open(&paths);
    assert_eq!(fetches(&visit(&mut again, ALPHA, T0 + HOUR)), [ALPHA]);
    Ok(())
}

#[test]
fn r11_an_unreached_attempt_keeps_the_stored_icon() -> Res<()> {
    // R11, R25: no icon is touched.
    let paths = fresh_paths("r11-keep")?;
    let mut core = open(&paths);
    add_site(&mut core, ALPHA, Keep::Both)?;
    visit(&mut core, ALPHA, T0 + DAY + HOUR);
    core.icon_unreached(ALPHA, T0 + DAY + HOUR + 10);
    assert_eq!(tab_icon(&core), Some(data_url(&icon()?.small)));
    assert!(has_files(&paths, ALPHA_STEM));
    Ok(())
}

#[test]
fn r11_an_unreached_attempt_frees_its_slot_and_starts_the_next_host() -> Res<()> {
    // R10, R11: gamma waits; alpha never reached its site; gamma starts.
    let mut core = open(&fresh_paths("r11-slot")?);
    start_all(&mut core, &[ALPHA, BETA, GAMMA, DELTA]);
    assert_eq!(fetches(&core.icon_unreached(ALPHA, T0 + 10)), [GAMMA]);
    assert_eq!(fetches(&core.icon_unreached(BETA, T0 + 11)), [DELTA]);
    assert!(fetches(&core.icon_unreached(GAMMA, T0 + 12)).is_empty());
    Ok(())
}

#[test]
fn r11_a_host_whose_fetch_is_running_is_not_asked_again_until_it_ends() -> Res<()> {
    // R8, R11: the time is recorded when the request starts. While it runs, no second ask.
    let mut core = open(&fresh_paths("r11-running")?);
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0)), [ALPHA]);
    assert!(fetches(&visit(&mut core, ALPHA, T0 + 1000)).is_empty());
    core.icon_unreached(ALPHA, T0 + 2000);
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0 + 3000)), [ALPHA]);
    Ok(())
}

#[test]
fn r8_an_attempt_time_later_than_now_counts_as_expired() -> Res<()> {
    // R8: the clock went back by 5 days. The recorded attempt is in the future.
    let mut core = open(&fresh_paths("r8-clock")?);
    let future = T0 + 5 * DAY;
    visit(&mut core, ALPHA, future);
    core.icon_fetched(ALPHA, None, future);
    assert_eq!(fetches(&visit(&mut core, ALPHA, T0)), [ALPHA]);
    Ok(())
}

#[test]
fn r8_a_future_attempt_time_read_from_disk_counts_as_expired() -> Res<()> {
    // R8: the same after a restart.
    let paths = fresh_paths("r8-clock-restart")?;
    let mut core = open(&paths);
    let future = T0 + 5 * DAY;
    visit(&mut core, ALPHA, future);
    core.icon_fetched(ALPHA, Some(icon()?), future);
    drop(core);
    let mut again = open(&paths);
    assert_eq!(fetches(&visit(&mut again, ALPHA, T0)), [ALPHA]);
    Ok(())
}

/// A core with no files, so 10 000 visits do not write 10 000 history files.
fn memory_core() -> Core {
    let mut core = Core::new(None, "127.0.0.1:4444", 0);
    core.router_changed(ok_status());
    core
}

/// Loads the pages `pages` of `host`, each its own history entry, one time unit apart
/// from `start`. Returns the effects of the last load.
fn visit_pages(
    core: &mut Core,
    host: &str,
    pages: std::ops::Range<u64>,
    start: u64,
) -> Vec<Effect> {
    let mut last = Vec::new();
    for n in pages {
        let url = format!("http://{host}/page-{n}");
        core.navigate(&url);
        let id = core.tabs().active_id();
        core.page_started(id, &url);
        last = core.page_finished(id, &url, start + n);
    }
    last
}

#[test]
fn r26_a_visit_that_pushes_the_last_entry_of_a_host_out_at_the_cap_deletes_its_icon() -> Res<()> {
    // R26, R30: alpha has one entry and an icon. 10 000 pages of beta follow: 10 001 in all,
    // so the oldest entry (alpha) goes. Its icon goes, with its attempt time (R29).
    let mut core = memory_core();
    visit(&mut core, ALPHA, T0);
    core.icon_fetched(ALPHA, Some(icon()?), T0);
    assert_eq!(tab_icon(&core), Some(data_url(&icon()?.small)));
    visit_pages(&mut core, BETA, 0..9_999, T0 + 1);
    let last = visit_pages(&mut core, BETA, 9_999..10_000, T0 + 20_000);
    assert_icons_changed(&last);
    let again = visit(&mut core, ALPHA, T0 + 30_000);
    assert_eq!(fetches(&again), [ALPHA], "R29: asked again inside 24 h");
    assert_eq!(tab_icon(&core), None, "the icon was deleted with its entry");
    Ok(())
}

#[test]
fn r26_the_cap_keeps_the_icon_of_a_host_with_a_bookmark() -> Res<()> {
    // R26: the entry goes, the bookmark keeps the host alive.
    let mut core = memory_core();
    add_site(&mut core, ALPHA, Keep::Both)?;
    visit_pages(&mut core, BETA, 0..10_000, T0 + 1);
    visit(&mut core, ALPHA, T0 + 30_000);
    assert_eq!(tab_icon(&core), Some(data_url(&icon()?.small)));
    Ok(())
}

#[test]
fn r26_the_cap_keeps_the_icon_of_a_host_with_another_entry() -> Res<()> {
    // R26: alpha has two entries; only the older goes.
    let mut core = memory_core();
    add_site(&mut core, ALPHA, Keep::History)?;
    visit_pages(&mut core, ALPHA, 0..1, T0 + 1);
    visit_pages(&mut core, BETA, 0..9_999, T0 + 2);
    core.navigate(&format!("http://{ALPHA}/page-0"));
    assert_eq!(tab_icon(&core), Some(data_url(&icon()?.small)));
    Ok(())
}
