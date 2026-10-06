// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! IPC command tests on the Tauri mock runtime: each command runs through the real invoke
//! handler, as the UI calls it, and changes the core as the contract says.

use serde_json::{Value, json};
use tauri::App;
use tauri::async_runtime::block_on;

use super::*;
use crate::net::testing::FakeRouter;
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
        block_on(router_status(handle(&app))).unwrap().status.proxy,
        "127.0.0.1:4444"
    );
}

// ------------------------------------------------ router statistics (R41, R49)
//
// The R31, R34, R40 and R45 cases of the sources, the cadence, the history and the stop
// run through `shell::sampler::tick` in `shell/sampler/tests.rs`. Here `router_stats` only
// reads what the sampler stored.

mod router_stats_command {
    use super::*;
    use crate::net::console::{ConsoleKind, stats_path};
    use crate::net::stats::RouterStats;
    use crate::net::testing::FakeConsole;
    use crate::shell::console::{detect_now, set_console, stop};
    use crate::shell::sampler::tick;
    use crate::shell::testing::detect_lock;
    use tauri::AppHandle;

    const JAVA: &str =
        include_str!("../../../tests/fixtures/console/java-2.13.0-xhr1-summaryframe.txt");

    /// An app with an open gate and a stored Java console. The router must outlive it.
    fn gated_java_app() -> (App<Mock>, FakeRouter, FakeConsole) {
        let app = app();
        let router = FakeRouter::start();
        open_gate(&app, &router);
        let fake =
            FakeConsole::serving(ConsoleKind::Java, stats_path(ConsoleKind::Java), 200, JAVA);
        set_console(app.handle(), Some(fake.verified()));
        (app, router, fake)
    }

    #[test]
    fn r49_current_stats_before_any_round_is_all_null() {
        // R49: "Before the first round, every figure is `null`."
        let app = app();
        assert_eq!(current_stats(&handle(&app)), RouterStats::default());
    }

    #[test]
    fn r49_current_stats_answers_the_latest_figures_of_the_sampler() {
        let (app, _router, _fake) = gated_java_app();
        tick(&handle(&app), None);
        let stats = current_stats(&handle(&app));
        assert_eq!(stats.uptime_ms, Some(28_800_000));
        assert_eq!(stats.bandwidth_bytes_per_second.in1s, Some(53_910));
        assert_eq!(stats.tunnels.participating, Some(398));
        assert_eq!(stats.history.len(), 1);
    }

    #[test]
    fn r49_current_stats_sends_no_request_to_the_console() {
        // R49, R34: "`router_stats()` sends no request".
        let (app, _router, fake) = gated_java_app();
        let seen = fake.requests().len();
        let _ = current_stats(&handle(&app));
        let _ = ok(&app, "router_stats", json!({}));
        assert_eq!(fake.requests().len(), seen, "{:?}", fake.requests());
    }

    fn check_shape_figures(value: &Value) {
        assert_eq!(value["uptimeMs"], json!(28_800_000));
        assert_eq!(value["uptimeResolutionMs"], json!(3_600_000));
        assert_eq!(value["floodfills"], json!(1570));
        assert_eq!(value["activePeers"], json!(1678));
        assert_eq!(value["knownRouters"], json!(4905));
        assert_eq!(value["networkStatus"], json!("OK"));
    }

    fn check_shape_groups(value: &Value) {
        assert_eq!(value["tunnels"]["participating"], json!(398));
        assert_eq!(value["tunnels"]["client"], json!(2));
        assert_eq!(value["tunnels"]["exploratory"], json!(11));
        assert_eq!(value["tunnels"]["in"], Value::Null);
        assert_eq!(value["tunnels"]["out"], Value::Null);
        assert_eq!(value["bandwidthBytesPerSecond"]["in1s"], json!(53_910));
        assert_eq!(value["bandwidthBytesPerSecond"]["out5m"], json!(33_060));
        assert_eq!(value["tunnelBuildSuccessPercent"]["total"], Value::Null);
        assert_eq!(value["version"], Value::Null);
    }

    fn check_shape_history(value: &Value) {
        let history = value["history"].as_array().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["in"], json!(53_910));
        assert_eq!(history[0]["out"], json!(37_370));
        assert!(history[0]["t"].is_u64());
    }

    #[test]
    fn r41_the_router_stats_command_answers_the_contract_v1_7_shape() {
        // R39, R41, IPC contract v1.7: camelCase keys, the new fields, history samples.
        let (app, _router, _fake) = gated_java_app();
        tick(&handle(&app), None);
        let value = ok(&app, "router_stats", json!({}));
        check_shape_figures(&value);
        check_shape_groups(&value);
        check_shape_history(&value);
    }

    #[test]
    fn r41_the_router_stats_command_with_no_round_answers_all_null() {
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

    #[test]
    fn r45_after_stop_the_router_stats_command_answers_all_null_and_asks_no_console() {
        // R45, R49: after the stop a round stores all null; the command only reads it.
        let (app, _router, fake) = gated_java_app();
        stop(&handle(&app));
        let seen = fake.requests().len();
        tick(&handle(&app), None);
        let value = ok(&app, "router_stats", json!({}));
        assert_eq!(fake.requests().len(), seen);
        assert_eq!(value["tunnels"]["participating"], Value::Null);
        assert_eq!(value["uptimeMs"], Value::Null);
        assert_eq!(value["history"], json!([]));
    }

    /// Requests for the Java stats path.
    fn stats_requests(fake: &FakeConsole) -> usize {
        let path = stats_path(ConsoleKind::Java);
        fake.requests()
            .iter()
            .filter(|line| line.split(' ').nth(1) == Some(path))
            .count()
    }

    /// Calls `stop` of the console loops when dropped, also when the test fails, so the
    /// loops that `detect_now` starts never keep probing the ports during the next test.
    struct StopOnDrop(AppHandle<Mock>);

    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            stop(&self.0);
        }
    }

    #[test]
    fn r45_after_detect_now_the_console_is_queried_again() {
        // R45: "until `detect_now` runs again". `detect_now` probes the default ports and
        // stores what it finds; the stored console is then the fake.
        let _lock = detect_lock();
        let (app, _router, fake) = gated_java_app();
        stop(&handle(&app));
        let _guard = StopOnDrop(handle(&app));
        let _ = detect_now(&handle(&app));
        set_console(app.handle(), Some(fake.verified()));
        let before = stats_requests(&fake);
        tick(&handle(&app), None);
        assert_eq!(stats_requests(&fake), before + 1, "one request again");
        assert_eq!(
            current_stats(&handle(&app)).tunnels.participating,
            Some(398)
        );
    }
}
