// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! IPC command tests on the Tauri mock runtime: each command runs through the real invoke
//! handler, as the UI calls it, and changes the core as the contract says.

use serde_json::{Value, json};
use tauri::App;
use tauri::async_runtime::block_on;

use super::*;
use crate::net::testing::{FakeRouter, dead_addr};
use crate::shell::testing::{Mock, app, core, gate_open, invoke, open_gate, wait_for};

fn handle(app: &App<Mock>) -> AppHandle<Mock> {
    app.handle().clone()
}

fn ok(app: &App<Mock>, cmd: &str, args: Value) -> Value {
    invoke(app, cmd, args).unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

#[test]
fn tab_commands_change_the_tab_strip() {
    let app = app();
    let tab = ok(&app, "tab_new", json!({ "url": "http://a.i2p/" }));
    let id = tab["id"].as_u64().unwrap();
    assert_eq!(ok(&app, "tab_list", json!({})).as_array().unwrap().len(), 2);
    ok(&app, "tab_move", json!({ "id": id, "index": 0 }));
    ok(&app, "tab_select", json!({ "id": id }));
    assert_eq!(
        core(&app).tabs().active().unwrap().id,
        u32::try_from(id).unwrap()
    );
    ok(&app, "tab_close", json!({ "id": id }));
    assert_eq!(core(&app).tabs().len(), 1);
}

#[test]
fn page_commands_run_on_the_active_tab() {
    let app = app();
    let nav = ok(&app, "navigate", json!({ "input": "http://a.i2p/" }));
    assert!(nav["ok"].is_boolean());
    for cmd in [
        "go_back",
        "go_forward",
        "stop",
        "home",
        "find_close",
        "zoom_in",
        "zoom_out",
        "zoom_reset",
    ] {
        assert_eq!(ok(&app, cmd, json!({})), Value::Null, "{cmd}");
    }
    ok(&app, "reload", json!({ "hard": true }));
    ok(&app, "reload", json!({}));
    let find = json!({ "query": "x", "forward": true, "matchCase": false });
    ok(&app, "find", find);
    ok(&app, "site_js_set", json!({ "host": "a.i2p", "on": false }));
    ok(&app, "chrome_set_height", json!({ "px": 300.0 }));
    assert!((core(&app).toolbar_request() - 300.0).abs() < f64::EPSILON);
}

#[test]
fn bookmark_commands_round_trip() {
    let app = app();
    let added = ok(
        &app,
        "bookmark_add",
        json!({ "bookmark": { "url": "http://a.i2p/", "title": "A" } }),
    );
    let found = ok(&app, "bookmark_find", json!({ "url": "http://a.i2p/" }));
    assert_eq!(found["id"], added["id"]);
    let mut renamed = added.clone();
    renamed["title"] = json!("B");
    ok(&app, "bookmark_update", json!({ "bookmark": renamed }));
    let title = |app: &App<Mock>| {
        ok(app, "bookmark_find", json!({ "url": "http://a.i2p/" }))["title"].clone()
    };
    assert_eq!(title(&app), json!("B"));
    let export = ok(&app, "bookmarks_export", json!({}));
    let count = ok(&app, "bookmarks_list", json!({}))
        .as_array()
        .unwrap()
        .len();
    ok(&app, "bookmark_remove", json!({ "id": added["id"] }));
    assert_eq!(title(&app), Value::Null);
    assert!(ok(&app, "bookmarks_import", json!({ "json": export })).is_u64());
    assert_eq!(
        ok(&app, "bookmarks_list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        count
    );
    let bad = json!({ "bookmark": { "url": "https://example.com/" } });
    assert!(invoke(&app, "bookmark_add", bad).is_err());
    assert!(invoke(&app, "bookmarks_import", json!({ "json": "not json" })).is_err());
}

#[test]
fn history_and_suggest_commands() {
    let app = app();
    assert_eq!(ok(&app, "history_query", json!({})), json!([]));
    assert_eq!(
        ok(&app, "history_query", json!({ "query": { "q": "a" } })),
        json!([])
    );
    ok(&app, "history_remove", json!({ "id": "none" }));
    assert_eq!(
        ok(&app, "history_clear", json!({ "range": "all" })),
        Value::Null
    );
    assert!(invoke(&app, "history_clear", json!({ "range": "year" })).is_err());
    assert!(ok(&app, "suggest", json!({ "input": "a" })).is_array());
}

#[test]
fn settings_commands() {
    let app = app();
    let before = ok(&app, "settings_get", json!({}));
    assert_eq!(ok(&app, "settings_set", json!({ "patch": {} })), before);
    assert!(invoke(&app, "settings_set", json!({ "patch": { "nope": 1 } })).is_err());
}

#[test]
fn router_commands() {
    let app = app();
    assert_eq!(
        ok(&app, "router_status", json!({}))["proxy"],
        json!("127.0.0.1:4444")
    );
    assert!(ok(&app, "router_stats", json!({}))["history"].is_array());
    let answer = ok(&app, "router_control", json!({ "action": "restart" }));
    assert_eq!(answer, json!({ "ok": false, "reason": "external" }));
    assert!(invoke(&app, "router_control", json!({ "action": "explode" })).is_err());
    assert_eq!(ok(&app, "platform", json!({})), json!(PLATFORM));
}

#[test]
fn pause_closes_the_gate_and_resume_verifies_again() {
    let app = app();
    let router = FakeRouter::start();
    open_gate(&app, &router);
    ok(&app, "connection_pause", json!({}));
    assert!(!gate_open(&app));
    assert!(core(&app).paused());
    ok(&app, "connection_resume", json!({}));
    assert!(!core(&app).paused());
    block_on(resume(handle(&app), Ok(router.addr))).unwrap();
    assert!(wait_for(|| gate_open(&app)));
}

#[test]
fn resume_with_a_bad_proxy_keeps_the_gate_closed() {
    let app = app();
    block_on(resume(handle(&app), Err("EEPVIEW_PROXY: bad".into()))).unwrap();
    assert!(wait_for(|| core(&app).router().state == "down"));
    assert!(!gate_open(&app));
}

#[test]
fn helper_stats_are_empty_without_a_helper() {
    assert_eq!(helper_stats(None), RouterStats::default());
    let dead = helper_stats(Some((dead_addr(), "token".into())));
    assert_eq!(dead, RouterStats::default());
}

#[test]
fn export_files_land_in_the_folder() {
    let dir = std::env::temp_dir().join(format!("eepview-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_export(&dir, "[]", 0).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[]");
    assert!(write_export(&dir.join("missing"), "[]", 0).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn direct_calls_match_the_ipc_answers() {
    let app = app();
    assert_eq!(block_on(tab_list(handle(&app))).unwrap().len(), 1);
    assert!(block_on(tab_new(handle(&app), None)).is_ok());
    assert_eq!(
        block_on(router_status(handle(&app))).unwrap().proxy,
        "127.0.0.1:4444"
    );
}

// ------------------------------------------------ router statistics (R23, R26, R32)

mod stats_from_console {
    use super::*;
    use crate::net::console::{ConsoleKind, stats_path};
    use crate::net::stats::{RouterStats, Sample};
    use crate::net::testing::FakeConsole;
    use crate::shell::console::set_console;

    const JAVA: &str =
        include_str!("../../../tests/fixtures/console/java-2.13.0-xhr1-summaryframe.txt");
    const I2PD: &str =
        include_str!("../../../tests/fixtures/console/i2pd-2.58.0-main-synthetic.txt");

    /// A fake Java I2P console that answers its stats path with the fixture, stored as the
    /// detected console of `app`.
    fn java_console(app: &App<Mock>) -> FakeConsole {
        let fake =
            FakeConsole::serving(ConsoleKind::Java, stats_path(ConsoleKind::Java), 200, JAVA);
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

    #[test]
    fn r23_with_no_helper_and_no_console_every_field_is_null() {
        // R23 step 3: no source, every field null; the history is empty.
        let app = app();
        assert_eq!(current_stats(&handle(&app), None), RouterStats::default());
    }

    #[test]
    fn r23_current_stats_never_runs_detection() {
        // R23: "with no stored console it does not probe". A console that runs but was
        // never stored by detection gets no request.
        let app = app();
        let fake =
            FakeConsole::serving(ConsoleKind::Java, stats_path(ConsoleKind::Java), 200, JAVA);
        let stats = current_stats(&handle(&app), None);
        assert_eq!(stats, RouterStats::default());
        assert!(
            fake.requests().is_empty(),
            "requests: {:?}",
            fake.requests()
        );
    }

    #[test]
    fn r23_current_stats_reads_the_stored_java_console() {
        let app = app();
        let _fake = java_console(&app);
        let stats = current_stats(&handle(&app), None);
        assert_eq!(stats.uptime_ms, Some(28_800_000));
        assert_eq!(stats.uptime_resolution_ms, Some(3_600_000));
        assert_eq!(stats.network_status.as_deref(), Some("OK"));
        assert_eq!(stats.bandwidth_bytes_per_second.in1s, Some(53_910));
        assert_eq!(stats.bandwidth_bytes_per_second.out1s, Some(37_370));
        assert_eq!(stats.bandwidth_bytes_per_second.in5m, Some(37_830));
        assert_eq!(stats.bandwidth_bytes_per_second.out5m, Some(33_060));
        assert_eq!(stats.active_peers, Some(1678));
        assert_eq!(stats.known_routers, Some(4905));
        assert_eq!(stats.floodfills, Some(1570));
        assert_eq!(stats.tunnels.participating, Some(398));
        assert_eq!(stats.tunnels.client, Some(2));
        assert_eq!(stats.tunnels.exploratory, Some(11));
        assert_eq!(stats.version, None);
    }

    #[test]
    fn r23_current_stats_reads_the_stored_i2pd_console() {
        let app = app();
        let fake = FakeConsole::serving(ConsoleKind::I2pd, "/", 200, I2PD);
        set_console(app.handle(), Some(fake.verified()));
        let stats = current_stats(&handle(&app), None);
        assert_eq!(stats.uptime_ms, Some(93_784_000));
        assert_eq!(stats.uptime_resolution_ms, Some(1_000));
        assert_eq!(stats.bandwidth_bytes_per_second.in1s, Some(12_636));
        assert_eq!(stats.bandwidth_bytes_per_second.out1s, Some(5_806));
        assert_eq!(stats.known_routers, Some(3021));
        assert_eq!(stats.floodfills, Some(812));
        assert_eq!(stats.tunnels.client, Some(14));
        assert_eq!(stats.tunnels.participating, Some(157));
        assert_eq!(stats.tunnel_build_success_percent.total, Some(42));
    }

    #[test]
    fn r23_a_helper_that_does_not_answer_falls_back_to_the_console() {
        // R23: the helper first; no answer from it, the console is next.
        let app = app();
        let _fake = java_console(&app);
        let stats = current_stats(&handle(&app), Some((dead_addr(), "token".into())));
        assert_eq!(stats.tunnels.participating, Some(398));
    }

    #[test]
    fn r23_a_console_that_does_not_answer_200_gives_all_null_and_no_sample() {
        // R23 step 3, R32: no source, no history sample.
        let app = app();
        let fake =
            FakeConsole::serving(ConsoleKind::Java, stats_path(ConsoleKind::Java), 503, JAVA);
        set_console(app.handle(), Some(fake.verified()));
        assert_eq!(current_stats(&handle(&app), None), RouterStats::default());
    }

    #[test]
    fn r26_each_call_makes_at_most_one_console_request() {
        // R26: "each `router_stats()` call makes at most one console request".
        let app = app();
        let fake = java_console(&app);
        let _ = current_stats(&handle(&app), None);
        assert_eq!(stats_requests(&fake), 1);
        let _ = current_stats(&handle(&app), None);
        assert_eq!(stats_requests(&fake), 2);
    }

    #[test]
    fn r26_no_stored_console_means_no_request_at_all() {
        let app = app();
        let fake = java_console(&app);
        set_console(app.handle(), None);
        let before = fake.requests().len();
        let _ = current_stats(&handle(&app), None);
        assert_eq!(fake.requests().len(), before);
    }

    #[test]
    fn r32_an_answer_from_the_console_includes_its_bandwidth_sample() {
        // R32: "The `history` of the answer includes the new sample."
        let app = app();
        let _fake = java_console(&app);
        let stats = current_stats(&handle(&app), None);
        assert_eq!(stats.history.len(), 1);
        let sample: Sample = stats.history[0];
        assert_eq!(sample.inbound, 53_910);
        assert_eq!(sample.out, 37_370);
    }

    #[test]
    fn r32_two_answers_less_than_4_seconds_apart_add_one_sample() {
        // R32: the panel and the Network page together still add one sample per 5 s.
        let app = app();
        let _fake = java_console(&app);
        let first = current_stats(&handle(&app), None);
        let second = current_stats(&handle(&app), None);
        assert_eq!(first.history.len(), 1);
        assert_eq!(
            second.history.len(),
            1,
            "the second call is inside the 4 s gap"
        );
        assert_eq!(second.history[0].t, first.history[0].t);
    }

    #[test]
    fn r32_a_console_sample_is_added_again_after_the_gap() {
        let app = app();
        let _fake = java_console(&app);
        let first = current_stats(&handle(&app), None);
        std::thread::sleep(std::time::Duration::from_millis(4_100));
        let second = current_stats(&handle(&app), None);
        assert_eq!(first.history.len(), 1);
        assert_eq!(second.history.len(), 2, "4.1 s later the sample is kept");
        assert!(second.history[1].t >= second.history[0].t + 4_000);
    }

    #[test]
    fn r33_the_router_stats_command_answers_the_contract_v1_6_shape() {
        // R31, R33, IPC contract v1.6: camelCase keys, the new fields, history samples.
        let app = app();
        let _fake = java_console(&app);
        let value = ok(&app, "router_stats", json!({}));
        assert_eq!(value["uptimeMs"], json!(28_800_000));
        assert_eq!(value["uptimeResolutionMs"], json!(3_600_000));
        assert_eq!(value["floodfills"], json!(1570));
        assert_eq!(value["activePeers"], json!(1678));
        assert_eq!(value["knownRouters"], json!(4905));
        assert_eq!(value["networkStatus"], json!("OK"));
        assert_eq!(value["tunnels"]["participating"], json!(398));
        assert_eq!(value["tunnels"]["client"], json!(2));
        assert_eq!(value["tunnels"]["exploratory"], json!(11));
        assert_eq!(value["tunnels"]["in"], Value::Null);
        assert_eq!(value["tunnels"]["out"], Value::Null);
        assert_eq!(value["bandwidthBytesPerSecond"]["in1s"], json!(53_910));
        assert_eq!(value["bandwidthBytesPerSecond"]["out5m"], json!(33_060));
        assert_eq!(value["tunnelBuildSuccessPercent"]["total"], Value::Null);
        assert_eq!(value["version"], Value::Null);
        let history = value["history"].as_array().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["in"], json!(53_910));
        assert_eq!(history[0]["out"], json!(37_370));
        assert!(history[0]["t"].is_u64());
    }

    #[test]
    fn r33_the_router_stats_command_with_no_source_answers_all_null() {
        let app = app();
        let value = ok(&app, "router_stats", json!({}));
        for key in [
            "uptimeMs",
            "uptimeResolutionMs",
            "floodfills",
            "activePeers",
            "version",
        ] {
            assert_eq!(value[key], Value::Null, "{key}");
        }
        assert_eq!(value["tunnels"]["client"], Value::Null);
        assert_eq!(value["tunnelBuildSuccessPercent"]["total"], Value::Null);
        assert_eq!(value["history"], json!([]));
    }
}
