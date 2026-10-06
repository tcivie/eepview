// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the background statistics sampler
//! (`docs/wiki/router-console.md`, "Router statistics from the console"): the source order
//! (R31), the cadence (R34), the bandwidth history (R40), no console request after stop
//! (R45), the sampler thread (R47), one round (R48), the read-only answer (R49) and the quit
//! (R53). A round runs through `tick`, the thread through `start`, `wake`, `stop` and
//! `shutdown`, and the answer through `current_stats`.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::{App, AppHandle, Manager};

use super::*;
use crate::core::{Core, Paths};
use crate::net::console::{ConsoleKind, stats_path};
use crate::net::stats::RouterStats;
use crate::net::testing::{FakeConsole, FakeHelper, FakeRouter, dead_addr};
use crate::shell::commands::current_stats;
use crate::shell::console::{detect_now, set_console, stop as stop_console};
use crate::shell::env::DEFAULT_PROXY;
use crate::shell::state::Shared;
use crate::shell::testing::{Mock, app, core, detect_lock, invoke, open_gate, wait_for};

const JAVA: &str =
    include_str!("../../../tests/fixtures/console/java-2.13.0-xhr1-summaryframe.txt");
const I2PD: &str = include_str!("../../../tests/fixtures/console/i2pd-2.58.0-main-synthetic.txt");
const HELPER_JSON: &str = r#"{"version":"helper-2.10.0","uptimeMs":5000}"#;

// ---------------------------------------------------------------- helpers

fn handle(app: &App<Mock>) -> AppHandle<Mock> {
    app.handle().clone()
}

/// One round without a helper.
fn round(app: &App<Mock>) {
    tick(&handle(app), None);
}

/// An app whose gatekeeper runs, as after a passing VERIFY. The router must outlive it.
fn gated() -> (App<Mock>, FakeRouter) {
    let app = app();
    let router = FakeRouter::start();
    open_gate(&app, &router);
    (app, router)
}

/// A fake Java I2P console that answers its stats path with the fixture, stored as the
/// detected console of `app`.
fn java_console(app: &App<Mock>) -> FakeConsole {
    let fake = FakeConsole::serving(ConsoleKind::Java, stats_path(ConsoleKind::Java), 200, JAVA);
    set_console(app.handle(), Some(fake.verified()));
    fake
}

fn stats_requests(fake: &FakeConsole) -> usize {
    let path = stats_path(ConsoleKind::Java);
    fake.requests()
        .iter()
        .filter(|line| line.split(' ').nth(1) == Some(path))
        .count()
}

/// An open gate, a stored Java console that was stopped. The fake has seen no request since.
fn stopped_app() -> (App<Mock>, FakeRouter, FakeConsole, usize) {
    let (app, router) = gated();
    let fake = java_console(&app);
    stop_console(&handle(&app));
    let seen = fake.requests().len();
    (app, router, fake, seen)
}

fn now_ms() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    u64::try_from(since.as_millis()).unwrap()
}

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

/// The content of a `bandwidth.json` with `(t, in, out)` samples.
fn bandwidth_json(samples: &[(u64, u64, u64)]) -> String {
    let list: Vec<Value> = samples
        .iter()
        .map(|&(t, inbound, out)| json!({ "t": t, "in": inbound, "out": out }))
        .collect();
    json!({ "version": 1, "samples": list }).to_string()
}

/// An app with the shared state and the given paths, but no window.
fn app_with_paths(paths: Paths) -> App<Mock> {
    let app = mock_builder().build(mock_context(noop_assets())).unwrap();
    let core = Core::new(Some(paths), DEFAULT_PROXY, now_ms());
    app.manage(Shared::<Mock>::new(core));
    app
}

/// The history of an answer as `(t, in, out)`.
fn triples(stats: &RouterStats) -> Vec<(u64, u64, u64)> {
    stats
        .history
        .iter()
        .map(|s| (s.t, s.inbound, s.out))
        .collect()
}

/// Sampler tests share one sampler: they run one at a time, and each one starts from a
/// stopped sampler and leaves one behind.
static SAMPLER_LOCK: Mutex<()> = Mutex::new(());

struct Running {
    _lock: MutexGuard<'static, ()>,
}

impl Running {
    fn begin() -> Self {
        let lock = SAMPLER_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        stop();
        assert!(
            wait_for(|| !running()),
            "a sampler of an earlier test ended"
        );
        Self { _lock: lock }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        stop();
        wait_for(|| !running());
    }
}

// ---------------------------------------------------------------- R48 step 1: no gatekeeper

#[test]
fn r48_with_no_gatekeeper_no_request_goes_to_the_console() {
    // R48 step 1: the router is not verified, so the round sends no request to the console.
    let app = app();
    let fake = java_console(&app);
    let seen = fake.requests().len();
    round(&app);
    assert_eq!(fake.requests().len(), seen, "{:?}", fake.requests());
}

#[test]
fn r48_with_no_gatekeeper_no_request_goes_to_the_helper() {
    // R48 step 1: no request to the helper either.
    let app = app();
    let helper = FakeHelper::serving(HELPER_JSON);
    tick(&handle(&app), Some((helper.addr(), "token".to_owned())));
    assert_eq!(helper.requests(), 0);
}

#[test]
fn r48_with_no_gatekeeper_the_latest_figures_are_all_null_and_no_sample_is_added() {
    // R48 step 1: "adds no sample. The latest figures become all `null`."
    let app = app();
    let _fake = java_console(&app);
    round(&app);
    assert_eq!(current_stats(&handle(&app)), RouterStats::default());
}

#[test]
fn r48_a_closed_gate_nulls_the_figures_and_keeps_the_history() {
    // R48 step 1: pausing the connection closes the gatekeeper; the history stays.
    let (app, _router) = gated();
    let _fake = java_console(&app);
    round(&app);
    invoke(&app, "connection_pause", json!({})).unwrap();
    round(&app);
    let stats = current_stats(&handle(&app));
    assert_eq!(stats.uptime_ms, None);
    assert_eq!(stats.tunnels.participating, None);
    assert_eq!(stats.bandwidth_bytes_per_second.in1s, None);
    assert_eq!(stats.history.len(), 1, "the history stays");
}

// ---------------------------------------------------------------- R31 source order

#[test]
fn r31_with_no_helper_and_no_console_every_field_is_null() {
    // R31 step 3: no source, every field null; the history is empty.
    let (app, _router) = gated();
    round(&app);
    assert_eq!(current_stats(&handle(&app)), RouterStats::default());
}

#[test]
fn r31_a_round_never_runs_detection() {
    // R31: "with no stored console it does not probe". A console that runs but was never
    // stored by detection gets no request.
    let (app, _router) = gated();
    let fake = FakeConsole::serving(ConsoleKind::Java, stats_path(ConsoleKind::Java), 200, JAVA);
    round(&app);
    assert_eq!(current_stats(&handle(&app)), RouterStats::default());
    assert!(fake.requests().is_empty(), "{:?}", fake.requests());
}

fn check_java_uptime_and_bandwidth(stats: &RouterStats) {
    assert_eq!(stats.uptime_ms, Some(28_800_000));
    assert_eq!(stats.uptime_resolution_ms, Some(3_600_000));
    assert_eq!(stats.network_status.as_deref(), Some("OK"));
    assert_eq!(stats.bandwidth_bytes_per_second.in1s, Some(53_910));
    assert_eq!(stats.bandwidth_bytes_per_second.out1s, Some(37_370));
    assert_eq!(stats.bandwidth_bytes_per_second.in5m, Some(37_830));
    assert_eq!(stats.bandwidth_bytes_per_second.out5m, Some(33_060));
}

fn check_java_counts(stats: &RouterStats) {
    assert_eq!(stats.active_peers, Some(1678));
    assert_eq!(stats.known_routers, Some(4905));
    assert_eq!(stats.floodfills, Some(1570));
    assert_eq!(stats.tunnels.participating, Some(398));
    assert_eq!(stats.tunnels.client, Some(2));
    assert_eq!(stats.tunnels.exploratory, Some(11));
    assert_eq!(stats.version, None);
}

#[test]
fn r31_a_round_takes_the_figures_of_the_stored_java_console() {
    let (app, _router) = gated();
    let _fake = java_console(&app);
    round(&app);
    let stats = current_stats(&handle(&app));
    check_java_uptime_and_bandwidth(&stats);
    check_java_counts(&stats);
}

#[test]
fn r31_a_round_takes_the_figures_of_the_stored_i2pd_console() {
    let (app, _router) = gated();
    let fake = FakeConsole::serving(ConsoleKind::I2pd, "/", 200, I2PD);
    set_console(app.handle(), Some(fake.verified()));
    round(&app);
    let stats = current_stats(&handle(&app));
    assert_eq!(stats.uptime_ms, Some(93_784_000));
    assert_eq!(stats.uptime_resolution_ms, Some(1_000));
    assert_eq!(stats.bandwidth_bytes_per_second.in1s, Some(12_636));
    assert_eq!(stats.bandwidth_bytes_per_second.out1s, Some(5_806));
    assert_eq!(stats.known_routers, Some(3021));
    assert_eq!(stats.floodfills, Some(812));
    assert_eq!(
        stats.tunnels.client, None,
        "R38: i2pd gives no client count"
    );
    assert_eq!(stats.tunnels.participating, Some(157));
    assert_eq!(stats.tunnel_build_success_percent.total, Some(42));
}

#[test]
fn r31_a_helper_that_does_not_answer_falls_back_to_the_console() {
    // R31: the helper first; no answer from it, the console is next.
    let (app, _router) = gated();
    let _fake = java_console(&app);
    tick(&handle(&app), Some((dead_addr(), "token".to_owned())));
    let stats = current_stats(&handle(&app));
    assert_eq!(stats.tunnels.participating, Some(398));
}

#[test]
fn r31_a_helper_that_answers_is_the_source_and_the_console_is_not_asked() {
    // R31: "When the helper answers, eepview does not ask the console."
    let (app, _router) = gated();
    let fake = java_console(&app);
    let seen = fake.requests().len();
    let helper = FakeHelper::serving(HELPER_JSON);
    tick(&handle(&app), Some((helper.addr(), "token".to_owned())));
    let stats = current_stats(&handle(&app));
    assert_eq!(stats.version.as_deref(), Some("helper-2.10.0"));
    assert_eq!(stats.uptime_ms, Some(5_000));
    assert_eq!(stats.tunnels.participating, None, "no console figure");
    assert_eq!(fake.requests().len(), seen, "{:?}", fake.requests());
}

#[test]
fn r31_a_console_that_does_not_answer_200_gives_all_null_and_no_sample() {
    // R31 step 3, R40: no source, no history sample.
    let (app, _router) = gated();
    let fake = FakeConsole::serving(ConsoleKind::Java, stats_path(ConsoleKind::Java), 503, JAVA);
    set_console(app.handle(), Some(fake.verified()));
    round(&app);
    assert_eq!(current_stats(&handle(&app)), RouterStats::default());
}

#[test]
fn r48_a_helper_answer_gives_the_helper_version_to_the_core() {
    // R48 step 4: "it gives the helper's version to the core, as the router watcher did".
    let (app, _router) = gated();
    let helper = FakeHelper::serving(HELPER_JSON);
    tick(&handle(&app), Some((helper.addr(), "token".to_owned())));
    assert_eq!(
        core(&app).router().version.as_deref(),
        Some("helper-2.10.0")
    );
}

// ---------------------------------------------------------------- R34 cadence

#[test]
fn r34_a_round_sends_at_most_one_console_request() {
    // R34: "at most one helper request and one console request per round".
    let (app, _router) = gated();
    let fake = java_console(&app);
    round(&app);
    assert_eq!(stats_requests(&fake), 1);
    round(&app);
    assert_eq!(stats_requests(&fake), 2);
}

#[test]
fn r34_a_round_sends_at_most_one_helper_request() {
    let (app, _router) = gated();
    let helper = FakeHelper::serving(HELPER_JSON);
    tick(&handle(&app), Some((helper.addr(), "token".to_owned())));
    assert_eq!(helper.requests(), 1);
    tick(&handle(&app), Some((helper.addr(), "token".to_owned())));
    assert_eq!(helper.requests(), 2);
}

#[test]
fn r34_no_stored_console_means_no_request_at_all() {
    let (app, _router) = gated();
    let fake = java_console(&app);
    set_console(app.handle(), None);
    let before = fake.requests().len();
    round(&app);
    assert_eq!(fake.requests().len(), before);
}

// ---------------------------------------------------------------- R40 bandwidth history

#[test]
fn r40_a_round_adds_the_bandwidth_sample_of_the_console() {
    // R40: each round that gets figures adds the `in1s` and `out1s` sample.
    let (app, _router) = gated();
    let _fake = java_console(&app);
    round(&app);
    let stats = current_stats(&handle(&app));
    assert_eq!(stats.history.len(), 1);
    assert_eq!(stats.history[0].inbound, 53_910);
    assert_eq!(stats.history[0].out, 37_370);
}

#[test]
fn r40_two_rounds_less_than_4_seconds_apart_add_one_sample() {
    // R40: a woken round right after a timed round adds no second sample.
    let (app, _router) = gated();
    let _fake = java_console(&app);
    round(&app);
    let first = current_stats(&handle(&app));
    round(&app);
    let second = current_stats(&handle(&app));
    assert_eq!(first.history.len(), 1);
    assert_eq!(
        second.history.len(),
        1,
        "the second round is inside the 4 s gap"
    );
    assert_eq!(second.history[0].t, first.history[0].t);
}

#[test]
fn r40_a_round_adds_a_sample_again_after_the_gap() {
    let (app, _router) = gated();
    let _fake = java_console(&app);
    round(&app);
    std::thread::sleep(Duration::from_millis(4_100));
    round(&app);
    let stats = current_stats(&handle(&app));
    assert_eq!(stats.history.len(), 2, "4.1 s later the sample is kept");
    assert!(stats.history[1].t >= stats.history[0].t + 4_000);
}

#[test]
fn r40_a_helper_answer_without_bandwidth_adds_no_sample() {
    // R40: "A sample needs both `in1s` and `out1s`."
    let (app, _router) = gated();
    let helper = FakeHelper::serving(HELPER_JSON);
    tick(&handle(&app), Some((helper.addr(), "token".to_owned())));
    assert!(current_stats(&handle(&app)).history.is_empty());
}

// ---------------------------------------------------------------- R45 no request after stop

#[test]
fn r45_after_stop_a_round_sends_no_request_to_the_console() {
    let (app, _router, fake, seen) = stopped_app();
    round(&app);
    round(&app);
    assert_eq!(fake.requests().len(), seen, "{:?}", fake.requests());
}

#[test]
fn r45_after_stop_with_no_helper_every_field_is_null_and_the_history_is_empty() {
    let (app, _router, _fake, _seen) = stopped_app();
    round(&app);
    assert_eq!(current_stats(&handle(&app)), RouterStats::default());
}

#[test]
fn r45_after_stop_a_helper_that_answers_gives_the_figures_and_no_console_sample() {
    let (app, _router, fake, seen) = stopped_app();
    let helper = FakeHelper::serving(HELPER_JSON);
    tick(&handle(&app), Some((helper.addr(), "token".to_owned())));
    let stats = current_stats(&handle(&app));
    assert_eq!(stats.version.as_deref(), Some("helper-2.10.0"));
    assert_eq!(stats.uptime_ms, Some(5_000));
    assert_eq!(stats.tunnels.participating, None, "no console figure");
    assert_eq!(fake.requests().len(), seen, "the console is not asked");
    assert!(
        stats.history.iter().all(|s| s.inbound != 53_910),
        "no console sample: {:?}",
        stats.history
    );
}

#[test]
fn r45_after_stop_a_helper_that_does_not_answer_gives_all_null_not_the_console() {
    let (app, _router, fake, seen) = stopped_app();
    tick(&handle(&app), Some((dead_addr(), "token".to_owned())));
    assert_eq!(current_stats(&handle(&app)), RouterStats::default());
    assert_eq!(fake.requests().len(), seen);
}

/// Calls `stop` of the console loops when dropped, also when the test fails, so the loops
/// that `detect_now` starts never keep probing the ports during the next test.
struct StopOnDrop(AppHandle<Mock>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        stop_console(&self.0);
    }
}

#[test]
fn r45_after_detect_now_the_console_is_queried_again() {
    // R45: "until `detect_now` runs again". `detect_now` probes the default ports and stores
    // what it finds; the stored console is then the fake.
    let _lock = detect_lock();
    let (app, _router, fake, _seen) = stopped_app();
    let _guard = StopOnDrop(handle(&app));
    let _ = detect_now(&handle(&app));
    set_console(app.handle(), Some(fake.verified()));
    let before = stats_requests(&fake);
    round(&app);
    assert_eq!(stats_requests(&fake), before + 1, "one request again");
    assert_eq!(
        current_stats(&handle(&app)).tunnels.participating,
        Some(398)
    );
}

// ---------------------------------------------------------------- R49 pages only read

#[test]
fn r49_current_stats_sends_no_request_and_adds_no_sample() {
    // R49, R34, R40: any number of reads send no request and add no sample.
    let (app, _router) = gated();
    let fake = java_console(&app);
    round(&app);
    let seen = fake.requests().len();
    let first = current_stats(&handle(&app));
    for _ in 0..20 {
        assert_eq!(current_stats(&handle(&app)), first);
    }
    assert_eq!(fake.requests().len(), seen, "{:?}", fake.requests());
    assert_eq!(first.history.len(), 1);
}

#[test]
fn r49_current_stats_never_runs_detection() {
    // R49: "never runs detection". A console that runs but was never stored gets no request.
    let (app, _router) = gated();
    let fake = FakeConsole::serving(ConsoleKind::Java, stats_path(ConsoleKind::Java), 200, JAVA);
    assert_eq!(current_stats(&handle(&app)), RouterStats::default());
    assert!(fake.requests().is_empty(), "{:?}", fake.requests());
}

#[test]
fn r49_current_stats_with_a_stored_console_but_no_round_is_all_null() {
    // R49: "Before the first round, every figure is `null`."
    let (app, _router) = gated();
    let fake = java_console(&app);
    let seen = fake.requests().len();
    assert_eq!(current_stats(&handle(&app)), RouterStats::default());
    assert_eq!(fake.requests().len(), seen);
}

#[test]
fn r49_before_the_first_round_the_history_is_the_one_loaded_at_start() {
    // R49, R52: figures null, `history` the stored samples, oldest first.
    let dir = crate::store::testdir::fresh("sampler-loaded");
    let paths = paths_in(&dir);
    let now = now_ms();
    let saved = [(now - 20_000, 1, 2), (now - 10_000, 3, 4)];
    std::fs::write(&paths.bandwidth, bandwidth_json(&saved)).unwrap();
    let app = app_with_paths(paths);
    let stats = current_stats(&handle(&app));
    assert_eq!(triples(&stats), saved);
    assert_eq!(stats.uptime_ms, None);
    assert_eq!(stats.bandwidth_bytes_per_second.in1s, None);
}

// ---------------------------------------------------------------- R48 step 5, R51 save

#[test]
fn r48_a_round_right_after_the_start_saves_no_history() {
    // R48 step 5, R51: the first timed save comes at least 60 s after the core was made.
    let dir = crate::store::testdir::fresh("sampler-no-save");
    let paths = paths_in(&dir);
    let app = app_with_paths(paths.clone());
    let router = FakeRouter::start();
    open_gate(&app, &router);
    let _fake = java_console(&app);
    round(&app);
    assert_eq!(current_stats(&handle(&app)).history.len(), 1);
    assert!(!paths.bandwidth.exists(), "no save is due yet");
}

// ---------------------------------------------------------------- R47 the sampler thread

#[test]
fn r47_start_returns_true_then_false_while_the_sampler_runs() {
    let (app, _router) = gated();
    let _run = Running::begin();
    assert!(start(&handle(&app)), "the first start starts the thread");
    assert!(running());
    assert!(!start(&handle(&app)), "a second start starts nothing");
    assert!(!start(&handle(&app)));
    assert!(running());
}

#[test]
fn r47_start_works_again_after_the_sampler_ended() {
    let (app, _router) = gated();
    let _run = Running::begin();
    assert!(start(&handle(&app)));
    stop();
    assert!(wait_for(|| !running()));
    assert!(start(&handle(&app)), "no thread runs, so start starts one");
}

#[test]
fn r47_a_round_runs_at_once_after_start() {
    // R47: "runs one round at once", not after 5 s.
    let (app, _router) = gated();
    let fake = java_console(&app);
    let _run = Running::begin();
    let started = Instant::now();
    assert!(start(&handle(&app)));
    assert!(wait_for(|| stats_requests(&fake) >= 1));
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
    assert!(wait_for(|| current_stats(&handle(&app)).history.len() == 1));
}

#[test]
fn r47_wake_makes_the_running_sampler_do_a_round_now() {
    let (app, _router) = gated();
    let fake = java_console(&app);
    let _run = Running::begin();
    assert!(start(&handle(&app)));
    assert!(wait_for(|| stats_requests(&fake) >= 1));
    std::thread::sleep(Duration::from_millis(300));
    let before = stats_requests(&fake);
    let woken = Instant::now();
    wake();
    assert!(wait_for(|| stats_requests(&fake) > before));
    assert!(
        woken.elapsed() < Duration::from_secs(4),
        "a timed round is 5 s away"
    );
}

#[test]
fn r47_wake_with_no_sampler_does_nothing() {
    let (app, _router) = gated();
    let fake = java_console(&app);
    let _run = Running::begin();
    let seen = fake.requests().len();
    wake();
    std::thread::sleep(Duration::from_millis(300));
    assert!(!running(), "wake never starts a sampler");
    assert_eq!(fake.requests().len(), seen);
}

#[test]
fn r47_a_console_that_detection_stores_wakes_the_sampler() {
    // R47: "When `set_console` stores a found console that differs from the stored one, it
    // calls `wake`": the figures show before the next timed round, 5 s later.
    let (app, _router) = gated();
    let _run = Running::begin();
    assert!(start(&handle(&app)));
    std::thread::sleep(Duration::from_millis(500));
    let stored = Instant::now();
    let fake = java_console(&app);
    assert!(wait_for(|| stats_requests(&fake) >= 1));
    assert!(
        stored.elapsed() < Duration::from_secs(4),
        "{:?}",
        stored.elapsed()
    );
    assert!(wait_for(|| current_stats(&handle(&app))
        .tunnels
        .participating
        == Some(398)));
}

// ---------------------------------------------------------------- R53 stop and quit

#[test]
fn r53_stop_ends_the_sampler_without_waiting_for_its_5_seconds() {
    let (app, _router) = gated();
    let fake = java_console(&app);
    let _run = Running::begin();
    assert!(start(&handle(&app)));
    assert!(wait_for(|| stats_requests(&fake) >= 1));
    std::thread::sleep(Duration::from_millis(300));
    let stopped = Instant::now();
    stop();
    assert!(wait_for(|| !running()));
    assert!(
        stopped.elapsed() < Duration::from_secs(4),
        "{:?}",
        stopped.elapsed()
    );
}

#[test]
fn r53_after_stop_the_sampler_starts_no_round() {
    let (app, _router) = gated();
    let fake = java_console(&app);
    let _run = Running::begin();
    assert!(start(&handle(&app)));
    assert!(wait_for(|| stats_requests(&fake) >= 1));
    stop();
    assert!(wait_for(|| !running()));
    let seen = stats_requests(&fake);
    wake();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(stats_requests(&fake), seen, "no round after stop");
}

#[test]
fn r53_shutdown_stops_the_sampler() {
    let (app, _router) = gated();
    let _run = Running::begin();
    assert!(start(&handle(&app)));
    shutdown(&handle(&app));
    assert!(wait_for(|| !running()));
}

#[test]
fn r53_shutdown_stops_the_console_loops() {
    // R53, R22, R45: after the quit, a round asks no console.
    let _run = Running::begin();
    let (app, _router) = gated();
    let fake = java_console(&app);
    shutdown(&handle(&app));
    let seen = fake.requests().len();
    round(&app);
    assert_eq!(fake.requests().len(), seen, "{:?}", fake.requests());
}

#[test]
fn r53_shutdown_saves_the_history_once() {
    // R53, R51: the quit writes `bandwidth.json`, whatever the time since the last save.
    let dir = crate::store::testdir::fresh("sampler-quit");
    let paths = paths_in(&dir);
    let now = now_ms();
    let saved = [(now - 20_000, 1, 2), (now - 10_000, 3, 4)];
    std::fs::write(&paths.bandwidth, bandwidth_json(&saved)).unwrap();
    let app = app_with_paths(paths.clone());
    std::fs::remove_file(&paths.bandwidth).unwrap();
    let _run = Running::begin();
    shutdown(&handle(&app));
    let text = std::fs::read_to_string(&paths.bandwidth).unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(file["version"], json!(1));
    assert_eq!(file["samples"].as_array().unwrap().len(), saved.len());
}
