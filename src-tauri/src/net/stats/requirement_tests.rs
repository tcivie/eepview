// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the router statistics from the console
//! (`docs/wiki/router-console.md`, "Router statistics from the console"): the source order
//! (R31), the new `RouterStats` fields (R39) and the bandwidth history (R40). They use only
//! the public interface of `net::stats`.

use std::cell::Cell;

use serde_json::json;

use crate::net::stats::{
    Bandwidth, BuildSuccess, History, MIN_SAMPLE_GAP_MS, RouterStats, Sample, StatsSource, Tunnels,
    pick,
};

// ---------------------------------------------------------------- helpers

/// Statistics with a recognisable value in every field the console can give.
fn console_stats() -> RouterStats {
    RouterStats {
        uptime_ms: Some(28_800_000),
        uptime_resolution_ms: Some(3_600_000),
        network_status: Some("OK".to_owned()),
        known_routers: Some(4905),
        floodfills: Some(1570),
        active_peers: Some(1678),
        tunnels: Tunnels {
            participating: Some(398),
            client: Some(2),
            exploratory: Some(11),
            ..Tunnels::default()
        },
        bandwidth_bytes_per_second: Bandwidth {
            in1s: Some(53_910),
            out1s: Some(37_370),
            in5m: Some(37_830),
            out5m: Some(33_060),
        },
        ..RouterStats::default()
    }
}

/// Statistics that differ from `console_stats` in every field the helper can give.
fn helper_stats() -> RouterStats {
    RouterStats {
        version: Some("2.10.0".to_owned()),
        uptime_ms: Some(5_000),
        uptime_resolution_ms: Some(1),
        network_status: Some("FIREWALLED".to_owned()),
        known_routers: Some(10),
        active_peers: Some(20),
        tunnels: Tunnels {
            inbound: Some(1),
            out: Some(2),
            participating: Some(3),
            client: Some(4),
            exploratory: Some(5),
        },
        bandwidth_bytes_per_second: Bandwidth {
            in1s: Some(100),
            out1s: Some(200),
            in5m: Some(300),
            out5m: Some(400),
        },
        tunnel_build_success_percent: BuildSuccess {
            exploratory: Some(60),
            client: Some(70),
            total: None,
        },
        ..RouterStats::default()
    }
}

/// The statistics of an answer whose newest bandwidth sample is `in1s` / `out1s`.
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

/// What `record` stores for `now` and `stats`: the sample `record_spaced` must store too.
fn recorded(now: u64, stats: &RouterStats) -> Sample {
    let mut plain = History::default();
    plain.record(now, stats);
    let samples = plain.samples();
    assert_eq!(
        samples.len(),
        1,
        "record adds one sample to an empty history"
    );
    samples[0]
}

fn times(history: &History) -> Vec<u64> {
    history.samples().iter().map(|s| s.t).collect()
}

// ---------------------------------------------------------------- R31 pick

#[test]
fn r31_pick_takes_the_helper_stats_first() {
    // R31: the router helper is the first source.
    let (stats, source) = pick(Some(helper_stats()), || Some(console_stats()));
    assert_eq!(stats, helper_stats());
    assert_eq!(source, StatsSource::Helper);
}

#[test]
fn r31_pick_does_not_ask_the_console_when_the_helper_answers() {
    // R31: "When the helper answers, eepview does not ask the console."
    let asked = Cell::new(0);
    let _ = pick(Some(helper_stats()), || {
        asked.set(asked.get() + 1);
        Some(console_stats())
    });
    assert_eq!(asked.get(), 0, "the console closure must not run");
}

#[test]
fn r31_pick_counts_an_all_null_helper_answer_as_an_answer() {
    // R31: the helper "answers 200 with JSON" is `Some`, even when every field is null.
    let asked = Cell::new(false);
    let (stats, source) = pick(Some(RouterStats::default()), || {
        asked.set(true);
        Some(console_stats())
    });
    assert_eq!(stats, RouterStats::default());
    assert_eq!(source, StatsSource::Helper);
    assert!(!asked.get());
}

#[test]
fn r31_pick_falls_back_to_the_console_when_the_helper_does_not_answer() {
    // R31 step 2: no helper answer, the console answers.
    let (stats, source) = pick(None, || Some(console_stats()));
    assert_eq!(stats, console_stats());
    assert_eq!(source, StatsSource::Console);
}

#[test]
fn r31_pick_asks_the_console_exactly_once() {
    // R31: "calls `console` once".
    let asked = Cell::new(0);
    let _ = pick(None, || {
        asked.set(asked.get() + 1);
        Some(console_stats())
    });
    assert_eq!(asked.get(), 1);
}

#[test]
fn r31_pick_gives_all_null_stats_when_no_source_answers() {
    // R31 step 3: "else no source: every field is `null`".
    let asked = Cell::new(0);
    let (stats, source) = pick(None, || {
        asked.set(asked.get() + 1);
        None
    });
    assert_eq!(stats, RouterStats::default());
    assert_eq!(source, StatsSource::None);
    assert_eq!(
        asked.get(),
        1,
        "the console is asked once, then there is no source"
    );
}

#[test]
fn r31_pick_never_mixes_fields_of_two_sources() {
    // R31: the figures come from the first source that answers, whole.
    let (stats, _) = pick(Some(helper_stats()), || Some(console_stats()));
    assert_eq!(
        stats.floodfills, None,
        "the console floodfills must not leak in"
    );
    assert_eq!(stats.uptime_resolution_ms, Some(1));
}

// ---------------------------------------------------------------- R39 contract shape

#[test]
fn r39_default_stats_serialize_to_the_contract_v1_6_shape() {
    // R39, IPC contract v1.7: every field null, with the new keys in camelCase.
    let value = serde_json::to_value(RouterStats::default()).unwrap();
    assert_eq!(
        value,
        json!({
            "version": null,
            "uptimeMs": null,
            "uptimeResolutionMs": null,
            "networkStatus": null,
            "knownRouters": null,
            "floodfills": null,
            "activePeers": null,
            "tunnels": {
                "in": null, "out": null, "participating": null,
                "client": null, "exploratory": null
            },
            "bandwidthBytesPerSecond": {
                "in1s": null, "out1s": null, "in5m": null, "out5m": null
            },
            "tunnelBuildSuccessPercent": {
                "exploratory": null, "client": null, "total": null
            },
            "history": []
        })
    );
}

#[test]
fn r39_new_fields_serialize_with_their_values() {
    // R39: uptimeResolutionMs, floodfills, tunnels.client, tunnels.exploratory and
    // tunnelBuildSuccessPercent.total.
    let mut stats = console_stats();
    stats.tunnel_build_success_percent.total = Some(42);
    let value = serde_json::to_value(&stats).unwrap();
    assert_eq!(value["uptimeResolutionMs"], json!(3_600_000));
    assert_eq!(value["floodfills"], json!(1570));
    assert_eq!(value["tunnels"]["client"], json!(2));
    assert_eq!(value["tunnels"]["exploratory"], json!(11));
    assert_eq!(value["tunnelBuildSuccessPercent"]["total"], json!(42));
}

// ---------------------------------------------------------------- R40 record_spaced

#[test]
fn r40_the_min_sample_gap_is_four_seconds() {
    // R40: "less than 4 s old".
    assert_eq!(MIN_SAMPLE_GAP_MS, 4_000);
}

#[test]
fn r40_record_spaced_adds_the_first_sample_to_an_empty_history() {
    // R40: "like `record`" when there is no newest sample.
    let stats = with_bandwidth(1_000, 2_000);
    let mut history = History::default();
    history.record_spaced(10_000, &stats);
    assert_eq!(history.samples(), vec![recorded(10_000, &stats)]);
}

#[test]
fn r40_record_spaced_drops_a_sample_closer_than_the_gap() {
    // R40: no sample when the newest one is less than MIN_SAMPLE_GAP_MS older than `now`.
    let stats = with_bandwidth(1_000, 2_000);
    let mut history = History::default();
    history.record_spaced(10_000, &stats);
    history.record_spaced(10_000 + MIN_SAMPLE_GAP_MS - 1, &with_bandwidth(5, 6));
    assert_eq!(times(&history), vec![10_000]);
}

#[test]
fn r40_record_spaced_drops_a_sample_at_the_same_instant() {
    // R40: the panel and the Network page asking in the same millisecond add one sample.
    let mut history = History::default();
    history.record_spaced(10_000, &with_bandwidth(1, 2));
    history.record_spaced(10_000, &with_bandwidth(3, 4));
    assert_eq!(times(&history), vec![10_000]);
}

#[test]
fn r40_record_spaced_keeps_the_first_of_two_close_samples() {
    // R40: the dropped sample changes nothing, so the kept one holds the first values.
    let first = with_bandwidth(1_000, 2_000);
    let mut history = History::default();
    history.record_spaced(10_000, &first);
    history.record_spaced(11_000, &with_bandwidth(9_999, 9_999));
    assert_eq!(history.samples(), vec![recorded(10_000, &first)]);
}

#[test]
fn r40_record_spaced_adds_a_sample_at_exactly_the_gap() {
    // R40: "less than 4 s old" drops; exactly 4 s adds.
    let mut history = History::default();
    history.record_spaced(10_000, &with_bandwidth(1, 2));
    history.record_spaced(10_000 + MIN_SAMPLE_GAP_MS, &with_bandwidth(3, 4));
    assert_eq!(times(&history), vec![10_000, 10_000 + MIN_SAMPLE_GAP_MS]);
}

#[test]
fn r40_record_spaced_measures_the_gap_from_the_newest_kept_sample() {
    // R40: a dropped sample does not move the reference point.
    let mut history = History::default();
    let gap = MIN_SAMPLE_GAP_MS;
    history.record_spaced(10_000, &with_bandwidth(1, 1));
    history.record_spaced(10_000 + gap - 1, &with_bandwidth(2, 2)); // dropped
    history.record_spaced(10_000 + gap, &with_bandwidth(3, 3)); // kept
    history.record_spaced(10_000 + 2 * gap - 1, &with_bandwidth(4, 4)); // dropped
    history.record_spaced(10_000 + 2 * gap, &with_bandwidth(5, 5)); // kept
    assert_eq!(
        times(&history),
        vec![10_000, 10_000 + gap, 10_000 + 2 * gap]
    );
}

#[test]
fn r40_the_panel_and_the_network_page_together_add_one_sample_per_five_seconds() {
    // R40: two pages ask every 5 s, offset by 1 s; the history gets one sample per 5 s.
    let mut history = History::default();
    for tick in 0..6_u64 {
        let base = 100_000 + tick * 5_000;
        history.record_spaced(base, &with_bandwidth(tick, tick));
        history.record_spaced(base + 1_000, &with_bandwidth(tick, tick));
    }
    let expected: Vec<u64> = (0..6).map(|tick| 100_000 + tick * 5_000).collect();
    assert_eq!(times(&history), expected);
}

#[test]
fn r40_record_spaced_stores_the_same_sample_as_record() {
    // R40: "like `record`": the sample values are the ones `record` stores.
    let stats = with_bandwidth(53_910, 37_370);
    let mut history = History::default();
    history.record_spaced(50_000, &stats);
    history.record_spaced(60_000, &stats);
    assert_eq!(
        history.samples(),
        vec![recorded(50_000, &stats), recorded(60_000, &stats)]
    );
}

// ---------------------------------------------------------------- R49 recent, R50, R52 restored

const TEN_MINUTES_MS: u64 = 600_000;

/// A sample made the way the file makes it: from `{"t", "in", "out"}`.
fn sample(t: u64, inbound: u64, out: u64) -> Sample {
    serde_json::from_value(json!({ "t": t, "in": inbound, "out": out })).unwrap()
}

fn restored(samples: &[(u64, u64, u64)], now: u64) -> History {
    let list = samples.iter().map(|&(t, i, o)| sample(t, i, o)).collect();
    History::restored(now, list)
}

fn triples(samples: &[Sample]) -> Vec<(u64, u64, u64)> {
    samples.iter().map(|s| (s.t, s.inbound, s.out)).collect()
}

#[test]
fn r52_sample_deserializes_from_t_in_out() {
    // R51, R52: the file writes a sample as `{"t": <Unix ms>, "in": <B/s>, "out": <B/s>}`.
    let value = sample(1_234, 56, 78);
    assert_eq!((value.t, value.inbound, value.out), (1_234, 56, 78));
}

#[test]
fn r52_sample_serializes_to_t_in_out_and_nothing_else() {
    let value = serde_json::to_value(sample(1_234, 56, 78)).unwrap();
    assert_eq!(value, json!({ "t": 1_234, "in": 56, "out": 78 }));
}

#[test]
fn r52_restored_keeps_the_samples_of_the_last_ten_minutes() {
    // R52: `now - 600 000 <= t <= now`, both ends included.
    let now = 5_000_000;
    let history = restored(
        &[
            (now - TEN_MINUTES_MS - 1, 1, 1),
            (now - TEN_MINUTES_MS, 2, 2),
            (now - 5_000, 3, 3),
            (now, 4, 4),
            (now + 1, 5, 5),
        ],
        now,
    );
    assert_eq!(
        triples(&history.samples()),
        [
            (now - TEN_MINUTES_MS, 2, 2),
            (now - 5_000, 3, 3),
            (now, 4, 4)
        ]
    );
}

#[test]
fn r52_restored_drops_a_sample_less_than_four_seconds_after_the_previous_kept_one() {
    let now = 5_000_000;
    let base = now - 30_000;
    let history = restored(
        &[
            (base, 1, 1),
            (base + 3_999, 2, 2),
            (base + 4_000, 3, 3),
            (base + 7_999, 4, 4),
            (base + 8_000, 5, 5),
        ],
        now,
    );
    assert_eq!(
        times(&history),
        vec![base, base + 4_000, base + 8_000],
        "the gap counts from the previous kept sample"
    );
}

#[test]
fn r52_restored_keeps_the_samples_in_time_order() {
    let now = 5_000_000;
    let history = restored(
        &[
            (now - 5_000, 3, 3),
            (now - 15_000, 1, 1),
            (now - 10_000, 2, 2),
        ],
        now,
    );
    assert_eq!(
        times(&history),
        vec![now - 15_000, now - 10_000, now - 5_000]
    );
}

#[test]
fn r52_restored_of_nothing_is_an_empty_history() {
    assert!(restored(&[], 5_000_000).samples().is_empty());
}

#[test]
fn r52_restored_keeps_the_values_of_each_sample() {
    let now = 5_000_000;
    let history = restored(&[(now - 5_000, 53_910, 37_370)], now);
    assert_eq!(triples(&history.samples()), [(now - 5_000, 53_910, 37_370)]);
}

#[test]
fn r49_recent_gives_the_samples_of_the_last_ten_minutes_oldest_first() {
    // R49: "stored samples whose `t` is at most 10 minutes (600 000 ms) before now".
    let now = 5_000_000;
    let mut history = History::default();
    for (t, v) in [(now - 590_000, 1), (now - 300_000, 2), (now - 5_000, 3)] {
        history.record_spaced(t, &with_bandwidth(v, v));
    }
    assert_eq!(
        triples(&history.recent(now)),
        [
            (now - 590_000, 1, 1),
            (now - 300_000, 2, 2),
            (now - 5_000, 3, 3)
        ]
    );
}

#[test]
fn r49_recent_drops_a_sample_more_than_ten_minutes_old() {
    let now = 5_000_000;
    let mut history = History::default();
    history.record_spaced(now - TEN_MINUTES_MS - 1, &with_bandwidth(1, 1));
    history.record_spaced(now - TEN_MINUTES_MS + 4_000, &with_bandwidth(2, 2));
    assert_eq!(
        triples(&history.recent(now)),
        [(now - TEN_MINUTES_MS + 4_000, 2, 2)]
    );
}

#[test]
fn r49_recent_keeps_a_sample_exactly_ten_minutes_old() {
    let now = 5_000_000;
    let mut history = History::default();
    history.record_spaced(now - TEN_MINUTES_MS, &with_bandwidth(1, 1));
    assert_eq!(times_of(&history.recent(now)), vec![now - TEN_MINUTES_MS]);
}

#[test]
fn r49_recent_of_an_empty_history_is_empty() {
    assert!(History::default().recent(5_000_000).is_empty());
}

#[test]
fn r49_recent_changes_nothing_in_the_history() {
    let now = 5_000_000;
    let mut history = History::default();
    history.record_spaced(now - 5_000, &with_bandwidth(1, 1));
    let before = history.samples();
    let _ = history.recent(now + 3 * TEN_MINUTES_MS);
    assert_eq!(history.samples(), before, "a read adds and removes nothing");
}

fn times_of(samples: &[Sample]) -> Vec<u64> {
    samples.iter().map(|s| s.t).collect()
}

#[test]
fn r50_the_history_keeps_every_sample_of_the_last_ten_minutes() {
    // R50: at 5 s per round, at least the last 120 samples; nothing older than 10 minutes.
    let start = 1_000_000;
    let mut history = History::default();
    for i in 0..300_u64 {
        history.record_spaced(start + i * 5_000, &with_bandwidth(i, i));
    }
    let newest = start + 299 * 5_000;
    let kept = history.samples();
    assert!(kept.len() >= 120, "kept {}", kept.len());
    assert_eq!(kept.last().map(|s| s.t), Some(newest));
    assert!(kept.iter().all(|s| newest - s.t <= TEN_MINUTES_MS));
    assert!(
        kept.iter().any(|s| newest - s.t >= 595_000),
        "the oldest of 10 minutes stays"
    );
}
