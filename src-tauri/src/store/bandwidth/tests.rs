// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the saved bandwidth history (`docs/wiki/router-console.md`,
//! "Router statistics from the console"): the history in memory (R50), the file
//! `bandwidth.json` (R51) and the load at start (R52), plus the core calls that read and
//! write it (R49, R51, R52, R53).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::{load, save};
use crate::net::stats::{Bandwidth, History, RouterStats};
use crate::store::testdir;

// ---------------------------------------------------------------- helpers

fn now_ms() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    u64::try_from(since.as_millis()).unwrap()
}

fn with_bandwidth(in1s: u64, out1s: u64) -> RouterStats {
    RouterStats {
        bandwidth_bytes_per_second: Bandwidth {
            in1s: Some(in1s),
            out1s: Some(out1s),
            ..Bandwidth::default()
        },
        ..RouterStats::default()
    }
}

/// A history of `(t, in, out)` samples, at least 4 s apart.
fn history_of(samples: &[(u64, u64, u64)]) -> History {
    let mut history = History::default();
    for &(t, inbound, out) in samples {
        history.record_spaced(t, &with_bandwidth(inbound, out));
    }
    history
}

fn triples(history: &History) -> Vec<(u64, u64, u64)> {
    history
        .samples()
        .iter()
        .map(|s| (s.t, s.inbound, s.out))
        .collect()
}

/// The content of a `bandwidth.json` with `(t, in, out)` samples.
fn file_text(samples: &[(u64, u64, u64)]) -> String {
    let list: Vec<Value> = samples
        .iter()
        .map(|&(t, inbound, out)| json!({ "t": t, "in": inbound, "out": out }))
        .collect();
    json!({ "version": 1, "samples": list }).to_string()
}

/// A fresh folder and the path of `bandwidth.json` in it.
fn fresh_file(name: &str) -> (PathBuf, PathBuf) {
    let dir = testdir::fresh(name);
    let path = dir.join("bandwidth.json");
    (dir, path)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The history that `load` makes from a file with `text`, in a folder of its own.
fn loaded(text: &str, now: u64) -> History {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let (_dir, path) = fresh_file(&format!("bandwidth-load-{n}"));
    std::fs::write(&path, text).unwrap();
    load(&path, now)
}

// ---------------------------------------------------------------- R51 the file

#[test]
fn r51_the_file_holds_the_version_and_the_samples_and_no_other_key() {
    // R51: `{"version": 1, "samples": [{"t", "in", "out"}, ...]}`, oldest first.
    let (_dir, path) = fresh_file("bandwidth-shape");
    let history = history_of(&[(1_000_000, 10, 20), (1_005_000, 30, 40)]);
    save(&path, &history).unwrap();
    assert_eq!(
        read_json(&path),
        json!({
            "version": 1,
            "samples": [
                { "t": 1_000_000, "in": 10, "out": 20 },
                { "t": 1_005_000, "in": 30, "out": 40 }
            ]
        })
    );
}

#[test]
fn r51_an_empty_history_saves_an_empty_sample_list() {
    let (_dir, path) = fresh_file("bandwidth-empty");
    save(&path, &History::default()).unwrap();
    assert_eq!(read_json(&path), json!({ "version": 1, "samples": [] }));
}

#[test]
fn r51_a_save_leaves_no_temp_file_next_to_the_file() {
    // R51: "A save writes a temp file and renames it over the file".
    let (dir, path) = fresh_file("bandwidth-atomic");
    save(&path, &history_of(&[(1_000_000, 1, 2)])).unwrap();
    save(&path, &history_of(&[(1_000_000, 1, 2), (1_005_000, 3, 4)])).unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["bandwidth.json"]);
}

#[test]
fn r51_a_save_replaces_the_old_content() {
    let (_dir, path) = fresh_file("bandwidth-replace");
    save(&path, &history_of(&[(1_000_000, 1, 2), (1_005_000, 3, 4)])).unwrap();
    save(&path, &history_of(&[(2_000_000, 5, 6)])).unwrap();
    let file = read_json(&path);
    assert_eq!(
        file["samples"],
        json!([{ "t": 2_000_000, "in": 5, "out": 6 }])
    );
}

// ---------------------------------------------------------------- R52 the load

#[test]
fn r52_load_returns_the_history_that_save_wrote() {
    let (_dir, path) = fresh_file("bandwidth-round-trip");
    let now = now_ms();
    let samples = [
        (now - 20_000, 1, 2),
        (now - 15_000, 3, 4),
        (now - 5_000, 5, 6),
    ];
    save(&path, &history_of(&samples)).unwrap();
    assert_eq!(triples(&load(&path, now)), samples);
}

#[test]
fn r52_a_missing_file_gives_an_empty_history() {
    let (_dir, path) = fresh_file("bandwidth-missing");
    assert!(load(&path, now_ms()).samples().is_empty());
}

#[test]
fn r52_a_folder_in_place_of_the_file_gives_an_empty_history() {
    // R52: "The load never fails the start."
    let dir = testdir::fresh("bandwidth-folder");
    assert!(load(&dir, now_ms()).samples().is_empty());
}

#[test]
fn r52_a_file_that_is_not_valid_json_of_that_form_gives_an_empty_history() {
    let now = now_ms();
    let valid = json!({ "t": now - 5_000, "in": 1, "out": 2 });
    let broken = [
        String::new(),
        "not json".to_owned(),
        "{".to_owned(),
        "[]".to_owned(),
        json!({ "version": 1 }).to_string(),
        json!({ "version": 1, "samples": "x" }).to_string(),
        json!({ "samples": [valid.clone()] }).to_string(),
        json!({ "version": 1, "samples": [valid, { "t": now - 4_000, "in": 1 }] }).to_string(),
    ];
    for text in &broken {
        assert!(loaded(text, now).samples().is_empty(), "{text}");
    }
}

#[test]
fn r52_a_version_other_than_1_gives_an_empty_history() {
    let now = now_ms();
    let sample = json!({ "t": now - 5_000, "in": 1, "out": 2 });
    for version in [json!(0), json!(2), json!("1")] {
        let text = json!({ "version": version, "samples": [sample.clone()] }).to_string();
        assert!(loaded(&text, now).samples().is_empty(), "{text}");
    }
}

#[test]
fn r52_samples_older_than_ten_minutes_are_dropped() {
    // R52: `now - 600 000 <= t`: exactly 10 minutes old stays, 1 ms older goes.
    let now = 5_000_000;
    let text = file_text(&[
        (now - 600_001, 1, 1),
        (now - 600_000, 2, 2),
        (now - 5_000, 3, 3),
    ]);
    assert_eq!(
        triples(&loaded(&text, now)),
        [(now - 600_000, 2, 2), (now - 5_000, 3, 3)]
    );
}

#[test]
fn r52_samples_later_than_now_are_dropped() {
    // R52: `t <= now`: a sample from the future (the clock went back) goes.
    let now = 5_000_000;
    let text = file_text(&[(now - 5_000, 1, 1), (now, 2, 2), (now + 1, 3, 3)]);
    assert_eq!(
        triples(&loaded(&text, now)),
        [(now - 5_000, 1, 1), (now, 2, 2)]
    );
}

#[test]
fn r52_a_sample_less_than_4_seconds_after_the_previous_kept_one_is_dropped() {
    let now = 5_000_000;
    let text = file_text(&[
        (now - 20_000, 1, 1),
        (now - 20_000 + 3_999, 2, 2),
        (now - 20_000 + 4_000, 3, 3),
    ]);
    assert_eq!(
        triples(&loaded(&text, now)),
        [(now - 20_000, 1, 1), (now - 16_000, 3, 3)]
    );
}

#[test]
fn r52_the_gap_is_measured_from_the_previous_kept_sample() {
    // R52: a dropped sample does not move the reference point.
    let now = 5_000_000;
    let base = now - 30_000;
    let text = file_text(&[
        (base, 1, 1),
        (base + 3_000, 2, 2),
        (base + 6_000, 3, 3),
        (base + 8_000, 4, 4),
        (base + 10_000, 5, 5),
    ]);
    assert_eq!(
        triples(&loaded(&text, now)),
        [(base, 1, 1), (base + 6_000, 3, 3), (base + 10_000, 5, 5)]
    );
}

#[test]
fn r52_a_file_out_of_order_loads_in_time_order() {
    let now = 5_000_000;
    let text = file_text(&[
        (now - 5_000, 3, 3),
        (now - 15_000, 1, 1),
        (now - 10_000, 2, 2),
    ]);
    assert_eq!(
        triples(&loaded(&text, now)),
        [
            (now - 15_000, 1, 1),
            (now - 10_000, 2, 2),
            (now - 5_000, 3, 3)
        ]
    );
}

#[test]
fn r50_a_load_keeps_the_120_samples_of_the_last_ten_minutes() {
    // R50: "at 5 s per round, that is at least the last 120 samples".
    let now = 5_000_000;
    let samples: Vec<(u64, u64, u64)> = (0..120_u64)
        .map(|i| (now - (119 - i) * 5_000, i, i))
        .collect();
    let history = loaded(&file_text(&samples), now);
    assert_eq!(triples(&history), samples);
}

// ---------------------------------------------------------------- the core calls

mod core_calls {
    use super::*;
    use crate::core::{Core, Paths};

    fn paths_in(dir: &Path) -> Paths {
        Paths {
            bookmarks: dir.join("bookmarks.json"),
            history: dir.join("history.json"),
            settings: dir.join("settings.json"),
            sites: dir.join("sites.json"),
            icons: dir.join("icons"),
            bandwidth: dir.join("bandwidth.json"),
        }
    }

    /// A core made at `now` over a folder whose `bandwidth.json` holds `samples`.
    fn core_with(name: &str, samples: &[(u64, u64, u64)], now: u64) -> (Core, Paths) {
        let paths = paths_in(&testdir::fresh(name));
        std::fs::write(&paths.bandwidth, file_text(samples)).unwrap();
        (Core::new(Some(paths.clone()), "p", now), paths)
    }

    fn answered(core: &Core, now: u64) -> Vec<(u64, u64, u64)> {
        let stats = core.stats_answer(now);
        stats
            .history
            .iter()
            .map(|s| (s.t, s.inbound, s.out))
            .collect()
    }

    #[test]
    fn r52_a_new_core_loads_bandwidth_json() {
        let now = now_ms();
        let samples = [(now - 20_000, 1, 2), (now - 10_000, 3, 4)];
        let (core, _paths) = core_with("bandwidth-core-load", &samples, now);
        assert_eq!(answered(&core, now), samples);
    }

    #[test]
    fn r52_a_new_core_with_a_broken_file_starts_with_an_empty_history() {
        let paths = paths_in(&testdir::fresh("bandwidth-core-broken"));
        std::fs::write(&paths.bandwidth, "not json").unwrap();
        let core = Core::new(Some(paths), "p", now_ms());
        assert_eq!(core.stats_answer(now_ms()), RouterStats::default());
    }

    #[test]
    fn r52_a_new_core_with_a_missing_file_starts_with_an_empty_history() {
        let paths = paths_in(&testdir::fresh("bandwidth-core-missing"));
        let core = Core::new(Some(paths), "p", now_ms());
        assert_eq!(core.stats_answer(now_ms()), RouterStats::default());
    }

    #[test]
    fn r49_the_answer_before_any_round_has_null_figures_and_the_loaded_history() {
        let now = now_ms();
        let samples = [(now - 10_000, 7, 8)];
        let (core, _paths) = core_with("bandwidth-core-before", &samples, now);
        let stats = core.stats_answer(now);
        assert_eq!(stats.uptime_ms, None);
        assert_eq!(stats.bandwidth_bytes_per_second.in1s, None);
        assert_eq!(answered(&core, now), samples);
    }

    #[test]
    fn r49_the_answer_gives_the_latest_figures() {
        let now = now_ms();
        let (mut core, _paths) = core_with("bandwidth-core-latest", &[], now);
        let latest = RouterStats {
            uptime_ms: Some(28_800_000),
            floodfills: Some(1570),
            ..RouterStats::default()
        };
        core.set_latest_stats(now, latest);
        let stats = core.stats_answer(now);
        assert_eq!(stats.uptime_ms, Some(28_800_000));
        assert_eq!(stats.floodfills, Some(1570));
        assert_eq!(stats.tunnels.participating, None);
    }

    #[test]
    fn r49_the_answer_gives_only_the_samples_of_the_last_ten_minutes() {
        // R49: `history` holds the samples whose `t` is at most 600 000 ms before now.
        let now = now_ms();
        let samples = [(now - 100_000, 1, 1), (now - 1_000, 2, 2)];
        let (core, _paths) = core_with("bandwidth-core-window", &samples, now);
        assert_eq!(answered(&core, now + 550_000), [(now - 1_000, 2, 2)]);
        assert_eq!(answered(&core, now + 99_999), samples);
    }

    #[test]
    fn r51_the_first_timed_save_comes_60_seconds_after_the_core_was_made() {
        let now = now_ms();
        let samples = [(now - 10_000, 1, 2)];
        let (mut core, paths) = core_with("bandwidth-core-first", &samples, now);
        std::fs::remove_file(&paths.bandwidth).unwrap();
        core.set_latest_stats(now, RouterStats::default());
        assert!(!core.save_stats_if_due(now + 59_999));
        assert!(!paths.bandwidth.exists(), "no save before 60 s");
        assert!(core.save_stats_if_due(now + 60_000));
        assert_eq!(triples(&load(&paths.bandwidth, now + 60_000)), samples);
    }

    #[test]
    fn r51_a_save_is_due_again_60_seconds_after_the_last_one() {
        let now = now_ms();
        let (mut core, paths) = core_with("bandwidth-core-again", &[], now);
        core.set_latest_stats(now, RouterStats::default());
        assert!(core.save_stats_if_due(now + 60_000));
        std::fs::remove_file(&paths.bandwidth).unwrap();
        assert!(!core.save_stats_if_due(now + 60_000));
        assert!(!core.save_stats_if_due(now + 119_999), "never more often");
        assert!(!paths.bandwidth.exists());
        assert!(core.save_stats_if_due(now + 120_000));
        assert!(paths.bandwidth.exists());
    }

    #[test]
    fn r51_a_failed_save_changes_nothing_and_the_next_due_save_tries_again() {
        // R51: "A save that fails changes nothing in memory; the next due save tries again."
        let now = now_ms();
        let dir = testdir::fresh("bandwidth-core-fail");
        let blocked = dir.join("blocked");
        std::fs::write(&blocked, "x").unwrap();
        let mut paths = paths_in(&dir);
        paths.bandwidth = blocked.join("bandwidth.json");
        let mut core = Core::new(Some(paths.clone()), "p", now);
        core.set_latest_stats(now, RouterStats::default());
        let before = core.stats_answer(now);
        assert!(!core.save_stats_if_due(now + 60_000), "the save fails");
        assert_eq!(core.stats_answer(now), before);
        std::fs::remove_file(&blocked).unwrap();
        std::fs::create_dir(&blocked).unwrap();
        assert!(
            core.save_stats_if_due(now + 60_001),
            "the next call tries again"
        );
        assert!(paths.bandwidth.exists());
    }

    #[test]
    fn r53_save_stats_writes_the_file_with_paths_and_does_nothing_without() {
        // R53: the quit saves once more, whatever the time since the last save.
        let now = now_ms();
        let samples = [(now - 10_000, 1, 2), (now - 5_000, 3, 4)];
        let (mut core, paths) = core_with("bandwidth-core-quit", &samples, now);
        std::fs::remove_file(&paths.bandwidth).unwrap();
        core.set_latest_stats(now, RouterStats::default());
        assert!(core.save_stats(now));
        assert_eq!(triples(&load(&paths.bandwidth, now)), samples);
        let mut bare = Core::new(None, "p", now);
        bare.set_latest_stats(now, RouterStats::default());
        assert!(!bare.save_stats(now), "no paths, no file");
    }
}
