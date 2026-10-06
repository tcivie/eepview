// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The four router checks (`docs/wiki/router-checks.md`, requirements V1 to V11, V13, V14):
//! the core state machine and the outcome functions, through the public API only.
//! Each test names the requirement it checks (`V<n>:`). V12 lives in `router_checks_outproxy.rs`.

use eepview_lib::core::checks::{
    CheckFacts, I2PD_MIN_VERSION, JAVA_MIN_VERSION, Outcome, outproxy_outcome, parse_version,
    tunnels_outcome, version_outcome,
};
use eepview_lib::core::{Core, Effect, Event};
use eepview_lib::net::console::ConsoleKind;
use eepview_lib::net::outproxy::OutproxyFinding;
use eepview_lib::net::stats::{RouterStats, Tunnels};
use eepview_lib::types::{CheckId, CheckState, RouterStatus, VerifyCheck};

const ORDER: [CheckId; 4] = [
    CheckId::ProxyI2p,
    CheckId::Version,
    CheckId::NoOutproxy,
    CheckId::Tunnels,
];
const PROXY: &str = "127.0.0.1:4444";

/// Helpers that may unwrap and panic (test support only).
#[cfg(test)]
mod support {
    use super::*;

    pub(super) fn check(core: &Core, id: CheckId) -> &VerifyCheck {
        core.checks()
            .iter()
            .find(|c| c.id == id)
            .expect("every check is in the list")
    }

    pub(super) fn report_json() -> serde_json::Value {
        let mut core = core();
        pass_round(&mut core, 5_000);
        let facts = CheckFacts {
            kind: None,
            ..good_facts()
        };
        core.checks_seen(5_000, &facts);
        serde_json::to_value(core.router_report()).expect("the report serializes")
    }

    pub(super) fn not_checked_detail<'a>(outcome: &'a Outcome, why: &str) -> &'a str {
        match outcome {
            Outcome::NotChecked(detail) => detail,
            other => panic!("{why}: expected not-checked, got {other:?}"),
        }
    }

    pub(super) fn passed_detail<'a>(outcome: &'a Outcome, why: &str) -> &'a str {
        match outcome {
            Outcome::Passed(Some(detail)) => detail,
            other => panic!("{why}: expected passed with a detail, got {other:?}"),
        }
    }

    pub(super) fn failed_detail<'a>(outcome: &'a Outcome, why: &str) -> &'a str {
        match outcome {
            Outcome::Failed(detail) => detail,
            other => panic!("{why}: expected failed, got {other:?}"),
        }
    }
}

use support::{check, failed_detail, not_checked_detail, passed_detail, report_json};

fn core() -> Core {
    Core::new(None, PROXY, 0)
}

fn status(state: &'static str, detail: Option<&str>) -> RouterStatus {
    RouterStatus {
        state,
        proxy: PROXY.into(),
        version: None,
        detail: detail.map(str::to_owned),
        paused: false,
        managed: false,
    }
}

fn states(core: &Core) -> Vec<CheckState> {
    core.checks().iter().map(|c| c.state).collect()
}

fn router_event(effects: &[Effect]) -> bool {
    effects.contains(&Effect::Emit(Event::Router))
}

fn stats(network: Option<&str>, client: Option<u64>) -> RouterStats {
    RouterStats {
        network_status: network.map(str::to_owned),
        tunnels: Tunnels {
            client,
            ..Tunnels::default()
        },
        ..RouterStats::default()
    }
}

fn clear() -> OutproxyFinding {
    OutproxyFinding::Clear {
        file: "i2ptunnel.config".into(),
    }
}

fn good_facts() -> CheckFacts {
    CheckFacts {
        kind: Some(ConsoleKind::Java),
        version: Some("2.13.0".into()),
        outproxy: clear(),
        stats: Some(stats(Some("OK"), Some(2))),
    }
}

/// One passing VERIFY round at `now`.
fn pass_round(core: &mut Core, now: u64) -> Vec<Effect> {
    core.verify_started();
    core.router_checked(now, status("ok", None))
}

/// A core with the gate open and checks 2 to 4 seen as good at `now`.
fn open_core(now: u64) -> Core {
    let mut core = core();
    pass_round(&mut core, now);
    core.checks_seen(now, &good_facts());
    core
}

fn outcome_detail(outcome: &Outcome) -> &str {
    match outcome {
        Outcome::Passed(detail) => detail.as_deref().unwrap_or(""),
        Outcome::Failed(detail) | Outcome::NotChecked(detail) => detail,
    }
}

fn assert_detail_rule(c: &VerifyCheck) {
    let has_text = c.detail.as_deref().is_some_and(|d| !d.is_empty());
    match c.state {
        CheckState::Failed | CheckState::NotChecked => assert!(has_text, "V3: {c:?}"),
        CheckState::Pending | CheckState::Running => assert_eq!(c.detail, None, "V3: {c:?}"),
        CheckState::Passed => {}
    }
}

fn assert_shape_rules(core: &Core) {
    for c in core.checks() {
        assert_detail_rule(c);
        assert_eq!(
            c.passed_at.is_some(),
            c.state == CheckState::Passed,
            "V4: {c:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// V1 to V4: the shape, the states, the detail and the pass time.

#[test]
fn v1_a_new_core_has_four_pending_checks_in_order() {
    let core = core();
    let ids: Vec<CheckId> = core.checks().iter().map(|c| c.id).collect();
    assert_eq!(ids, ORDER, "V1: four checks in the order of the table");
    assert_eq!(
        states(&core),
        [CheckState::Pending; 4],
        "V5: all pending at start"
    );
}

#[test]
fn v1_the_list_keeps_four_entries_in_order_through_every_step() {
    let mut core = core();
    let mut seen = vec![];
    core.verify_started();
    seen.push(core.checks().to_vec());
    core.router_checked(1_000, status("down", Some("refused")));
    seen.push(core.checks().to_vec());
    pass_round(&mut core, 2_000);
    core.checks_seen(2_000, &good_facts());
    seen.push(core.checks().to_vec());
    core.pause();
    seen.push(core.checks().to_vec());
    core.resume();
    seen.push(core.checks().to_vec());
    for list in seen {
        let ids: Vec<CheckId> = list.iter().map(|c| c.id).collect();
        assert_eq!(ids, ORDER, "V1");
    }
}

#[test]
fn v1_the_report_holds_the_status_and_the_same_four_checks() {
    let mut core = open_core(5_000);
    let report = core.router_report();
    assert_eq!(
        report.checks,
        core.checks(),
        "V1: the report carries the core checks"
    );
    assert_eq!(
        &report.status,
        core.router(),
        "V1: the report carries the router status"
    );
    core.router_changed(status("building", None));
    assert_eq!(core.router_report().checks.len(), 4, "V1");
}

#[test]
fn v1_the_report_serializes_the_status_fields_and_four_checks() {
    let json = report_json();
    for key in ["state", "proxy", "paused", "managed", "checks"] {
        assert!(
            json.get(key).is_some(),
            "V1: {key} at the top level of {json}"
        );
    }
    let list = json["checks"].as_array().expect("checks is an array");
    let ids: Vec<&str> = list
        .iter()
        .map(|c| c["id"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        ids,
        ["proxy-i2p", "version", "no-outproxy", "tunnels"],
        "V1"
    );
    for entry in list {
        let mut keys: Vec<&str> = entry
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["detail", "id", "passedAt", "state"], "V1: {entry}");
    }
}

#[test]
fn v2_v4_the_report_writes_states_in_kebab_case_and_pass_time_in_ms() {
    let json = report_json();
    let list = json["checks"].as_array().expect("checks is an array");
    assert_eq!(list[0]["state"], "passed", "V2");
    assert_eq!(list[0]["passedAt"], 5_000, "V4: the Unix time in ms");
    assert_eq!(list[1]["state"], "not-checked", "V2: kebab-case");
    assert!(list[1]["passedAt"].is_null(), "V4");
}

#[test]
fn v3_v4_detail_and_pass_time_follow_the_state_in_every_step() {
    let mut core = core();
    assert_shape_rules(&core);
    core.verify_started();
    assert_shape_rules(&core);
    core.router_checked(1_000, status("ok", None));
    assert_shape_rules(&core);
    let bad = CheckFacts {
        kind: None,
        version: None,
        outproxy: OutproxyFinding::Listed {
            file: "a.config".into(),
            outproxies: vec!["x.i2p".into()],
        },
        stats: Some(stats(Some("FIREWALLED"), Some(0))),
    };
    core.checks_seen(1_000, &bad);
    assert_shape_rules(&core);
    core.router_checked(2_000, status("down", Some("refused")));
    assert_shape_rules(&core);
    core.pause();
    assert_shape_rules(&core);
}

#[test]
fn v4_pass_time_stays_while_the_check_stays_passed() {
    let mut core = open_core(1_000);
    for now in [6_000, 11_000, 16_000] {
        pass_round(&mut core, now);
        core.checks_seen(now, &good_facts());
    }
    for c in core.checks() {
        assert_eq!(c.state, CheckState::Passed, "V4: {c:?}");
        assert_eq!(
            c.passed_at,
            Some(1_000),
            "V4: the time of the first pass, round after round"
        );
    }
}

#[test]
fn v4_a_check_that_leaves_passed_and_passes_again_gets_the_new_time() {
    let mut core = open_core(1_000);
    let old = CheckFacts {
        version: Some("2.3.0".into()),
        ..good_facts()
    };
    pass_round(&mut core, 6_000);
    core.checks_seen(6_000, &old);
    let version = check(&core, CheckId::Version);
    assert_eq!(version.state, CheckState::Failed, "V11");
    assert_eq!(version.passed_at, None, "V4: null in every other state");
    pass_round(&mut core, 11_000);
    core.checks_seen(11_000, &good_facts());
    assert_eq!(
        check(&core, CheckId::Version).passed_at,
        Some(11_000),
        "V4: the new pass time"
    );
    assert_eq!(
        check(&core, CheckId::ProxyI2p).passed_at,
        Some(1_000),
        "V4: check 1 never left"
    );
}

// ---------------------------------------------------------------------------------------------
// V5 to V8: check 1 and the gate.

#[test]
fn v5_a_round_start_turns_check_one_to_running_and_emits() {
    let mut core = core();
    let effects = core.verify_started();
    assert_eq!(
        check(&core, CheckId::ProxyI2p).state,
        CheckState::Running,
        "V5"
    );
    assert_eq!(
        states(&core)[1..],
        [CheckState::Pending; 3],
        "V5: checks 2 to 4 wait for check 1"
    );
    assert!(
        router_event(&effects),
        "V14: router-status when check 1 turns running"
    );
}

#[test]
fn v5_a_round_start_does_not_change_a_passed_check_one() {
    let mut core = open_core(1_000);
    let before = core.checks().to_vec();
    let effects = core.verify_started();
    assert_eq!(core.checks(), before, "V5: no flicker every 5 s");
    assert!(
        !router_event(&effects),
        "V5: nothing changed, nothing to emit"
    );
}

#[test]
fn v5_a_round_start_does_not_change_a_failed_check_one() {
    let mut core = core();
    core.verify_started();
    core.router_checked(1_000, status("down", Some("connection refused")));
    let before = core.checks().to_vec();
    let effects = core.verify_started();
    assert_eq!(
        core.checks(),
        before,
        "V5: a failed check 1 keeps its state while the round runs"
    );
    assert_eq!(
        check(&core, CheckId::ProxyI2p).state,
        CheckState::Failed,
        "V5"
    );
    assert!(
        !router_event(&effects),
        "V5: nothing changed, nothing to emit"
    );
}

#[test]
fn v5_a_second_round_start_with_check_one_running_changes_nothing() {
    let mut core = core();
    core.verify_started();
    let effects = core.verify_started();
    assert_eq!(
        check(&core, CheckId::ProxyI2p).state,
        CheckState::Running,
        "V5"
    );
    assert!(!router_event(&effects), "V5: check 1 did not change");
}

#[test]
fn v6_state_ok_makes_check_one_passed_with_the_round_time() {
    let mut core = core();
    core.verify_started();
    let effects = core.router_checked(7_000, status("ok", None));
    let first = check(&core, CheckId::ProxyI2p);
    assert_eq!(first.state, CheckState::Passed, "V6");
    assert_eq!(first.passed_at, Some(7_000), "V4");
    assert_eq!(
        core.router().state,
        "ok",
        "V6: the round sets the router status"
    );
    assert!(
        router_event(&effects),
        "V14: router-status when the round changes the checks"
    );
}

#[test]
fn v6_check_one_keeps_its_pass_time_across_passing_rounds() {
    let mut core = core();
    pass_round(&mut core, 7_000);
    pass_round(&mut core, 12_000);
    assert_eq!(check(&core, CheckId::ProxyI2p).passed_at, Some(7_000), "V4");
}

#[test]
fn v6_any_other_state_fails_check_one_with_the_round_detail() {
    for state in ["down", "not-i2p", "building", "verifying"] {
        let mut core = core();
        core.verify_started();
        let effects = core.router_checked(7_000, status(state, Some("wrong self-test page")));
        let first = check(&core, CheckId::ProxyI2p);
        assert_eq!(first.state, CheckState::Failed, "V6: {state}");
        assert_eq!(
            first.detail.as_deref(),
            Some("wrong self-test page"),
            "V6: {state}"
        );
        assert_eq!(first.passed_at, None, "V4: {state}");
        assert!(router_event(&effects), "V14: {state}");
    }
}

#[test]
fn v6_a_missing_round_detail_gives_the_router_state_as_detail() {
    for state in ["down", "not-i2p", "building", "verifying"] {
        let mut core = core();
        core.verify_started();
        core.router_checked(7_000, status(state, None));
        let first = check(&core, CheckId::ProxyI2p);
        assert_eq!(first.state, CheckState::Failed, "V6: {state}");
        assert_eq!(
            first.detail.as_deref(),
            Some(state),
            "V6: the router state is the detail"
        );
    }
}

#[test]
fn v6_router_changed_alone_does_not_change_the_checks() {
    let mut core = open_core(1_000);
    let before = core.checks().to_vec();
    core.router_changed(status("down", Some("refused")));
    assert_eq!(
        core.checks(),
        before,
        "V6: only a VERIFY round changes the checks"
    );
    assert_eq!(core.router().state, "down");
}

#[test]
fn v7_a_failed_round_sends_checks_two_to_four_back_to_pending() {
    let mut core = open_core(1_000);
    assert_eq!(
        states(&core),
        [CheckState::Passed; 4],
        "setup: all four passed"
    );
    core.verify_started();
    core.router_checked(6_000, status("down", Some("connection refused")));
    assert_eq!(
        check(&core, CheckId::ProxyI2p).state,
        CheckState::Failed,
        "V7"
    );
    assert_eq!(
        check(&core, CheckId::ProxyI2p).detail.as_deref(),
        Some("connection refused"),
        "V7: check 1 shows its reason"
    );
    for id in [CheckId::Version, CheckId::NoOutproxy, CheckId::Tunnels] {
        let c = check(&core, id);
        assert_eq!(c.state, CheckState::Pending, "V7: {id:?}");
        assert_eq!(c.detail, None, "V7: {id:?}");
        assert_eq!(c.passed_at, None, "V7: {id:?}");
    }
}

#[test]
fn v7_checks_two_to_four_stay_pending_while_check_one_is_not_passed() {
    let mut core = core();
    core.verify_started();
    let before = core.checks().to_vec();
    let effects = core.checks_seen(1_000, &good_facts());
    assert_eq!(
        core.checks(),
        before,
        "V7: the gate is closed, so checks_seen does nothing"
    );
    assert!(!router_event(&effects), "V7");
    core.router_checked(2_000, status("not-i2p", Some("wrong page")));
    core.checks_seen(2_000, &good_facts());
    assert_eq!(
        states(&core)[1..],
        [CheckState::Pending; 3],
        "V7: a failed check 1 closes the gate"
    );
}

#[test]
fn v7_checks_two_to_four_never_change_the_router_state_or_check_one() {
    let mut core = core();
    pass_round(&mut core, 1_000);
    let bad = CheckFacts {
        kind: Some(ConsoleKind::I2pd),
        version: Some("2.1.0".into()),
        outproxy: OutproxyFinding::Listed {
            file: "i2pd.conf".into(),
            outproxies: vec!["x.i2p".into()],
        },
        stats: Some(stats(Some("FIREWALLED"), Some(0))),
    };
    core.checks_seen(1_000, &bad);
    assert_eq!(
        states(&core)[1..],
        [CheckState::Failed; 3],
        "setup: all three failed"
    );
    assert_eq!(
        core.router().state,
        "ok",
        "the checks never change RouterStatus.state"
    );
    assert_eq!(
        check(&core, CheckId::ProxyI2p).state,
        CheckState::Passed,
        "the gate stays open"
    );
}

#[test]
fn v8_pause_makes_all_four_checks_pending() {
    let mut core = open_core(1_000);
    let effects = core.pause();
    assert_eq!(states(&core), [CheckState::Pending; 4], "V8");
    assert_shape_rules(&core);
    assert!(
        router_event(&effects),
        "V14: the card learns that the checks went back"
    );
}

#[test]
fn v8_the_checks_stay_pending_while_paused() {
    let mut core = open_core(1_000);
    core.pause();
    let effects = core.checks_seen(6_000, &good_facts());
    assert_eq!(states(&core), [CheckState::Pending; 4], "V8");
    assert!(!router_event(&effects), "V8");
}

#[test]
fn v8_resume_makes_all_four_pending_then_the_next_round_runs_as_v5_and_v6() {
    let mut core = open_core(1_000);
    core.pause();
    core.resume();
    assert_eq!(states(&core), [CheckState::Pending; 4], "V8");
    core.verify_started();
    assert_eq!(
        check(&core, CheckId::ProxyI2p).state,
        CheckState::Running,
        "V5"
    );
    core.router_checked(9_000, status("ok", None));
    assert_eq!(
        check(&core, CheckId::ProxyI2p).state,
        CheckState::Passed,
        "V6"
    );
    assert_eq!(check(&core, CheckId::ProxyI2p).passed_at, Some(9_000), "V4");
}

// ---------------------------------------------------------------------------------------------
// V9 and V14: checks 2 to 4 in the core.

#[test]
fn v9_a_passed_check_one_turns_checks_two_to_four_to_running() {
    let mut core = core();
    core.verify_started();
    core.router_checked(1_000, status("ok", None));
    assert_eq!(
        check(&core, CheckId::ProxyI2p).state,
        CheckState::Passed,
        "V6"
    );
    assert_eq!(
        states(&core)[1..],
        [CheckState::Running; 3],
        "V9: they run in the same round"
    );
    assert_shape_rules(&core);
}

#[test]
fn v9_the_facts_of_a_round_give_the_state_of_each_check() {
    let mut core = core();
    pass_round(&mut core, 1_000);
    let effects = core.checks_seen(1_500, &good_facts());
    for id in [CheckId::Version, CheckId::NoOutproxy, CheckId::Tunnels] {
        let c = check(&core, id);
        assert_eq!(c.state, CheckState::Passed, "V9: {id:?}");
        assert_eq!(c.passed_at, Some(1_500), "V4: {id:?}");
    }
    assert!(router_event(&effects), "V14: checks 2 to 4 changed");
}

#[test]
fn v9_checks_run_again_in_every_round_while_the_gate_is_open() {
    let mut core = open_core(1_000);
    pass_round(&mut core, 6_000);
    let facts = CheckFacts {
        stats: Some(stats(Some("FIREWALLED"), Some(2))),
        ..good_facts()
    };
    core.checks_seen(6_000, &facts);
    let tunnels = check(&core, CheckId::Tunnels);
    assert_eq!(
        tunnels.state,
        CheckState::Failed,
        "V9, V13: the new round shows the new fact"
    );
    assert!(
        tunnels
            .detail
            .as_deref()
            .is_some_and(|d| d.contains("FIREWALLED")),
        "V13"
    );
    assert_eq!(
        check(&core, CheckId::Version).state,
        CheckState::Passed,
        "V9"
    );
    pass_round(&mut core, 11_000);
    core.checks_seen(11_000, &good_facts());
    assert_eq!(
        check(&core, CheckId::Tunnels).state,
        CheckState::Passed,
        "V9: and again"
    );
}

#[test]
fn v9_the_checks_map_to_the_outcome_of_their_own_fact() {
    let mut core = core();
    pass_round(&mut core, 1_000);
    let facts = CheckFacts {
        kind: None,
        version: Some("2.13.0".into()),
        outproxy: OutproxyFinding::Listed {
            file: "i2ptunnel.config".into(),
            outproxies: vec!["exit.i2p".into()],
        },
        stats: Some(stats(Some("OK"), None)),
    };
    core.checks_seen(2_000, &facts);
    assert_eq!(
        check(&core, CheckId::Version).state,
        CheckState::NotChecked,
        "V11: no type"
    );
    assert_eq!(
        check(&core, CheckId::NoOutproxy).state,
        CheckState::Failed,
        "V12: an outproxy"
    );
    assert_eq!(
        check(&core, CheckId::Tunnels).state,
        CheckState::NotChecked,
        "V13: no figure"
    );
    assert!(
        check(&core, CheckId::NoOutproxy)
            .detail
            .as_deref()
            .is_some_and(|d| d.contains("exit.i2p")),
        "V12: the detail names the outproxy"
    );
    assert_shape_rules(&core);
}

#[test]
fn v14_a_round_that_changes_nothing_in_checks_two_to_four_emits_nothing_for_them() {
    let mut core = open_core(1_000);
    let before = core.checks().to_vec();
    let effects = core.checks_seen(6_000, &good_facts());
    assert_eq!(
        core.checks(),
        before,
        "V4: same state, same detail, same passedAt"
    );
    assert!(!router_event(&effects), "V14: no change, no router-status");
}

#[test]
fn v14_a_changed_detail_emits() {
    let mut core = open_core(1_000);
    let facts = CheckFacts {
        version: Some("2.14.0".into()),
        ..good_facts()
    };
    let effects = core.checks_seen(6_000, &facts);
    assert_eq!(check(&core, CheckId::Version).state, CheckState::Passed);
    assert!(
        router_event(&effects),
        "V14: the version detail names the version, which changed"
    );
}

#[test]
fn v14_a_changed_state_emits() {
    let mut core = open_core(1_000);
    let facts = CheckFacts {
        stats: Some(stats(Some("OK"), Some(0))),
        ..good_facts()
    };
    let effects = core.checks_seen(6_000, &facts);
    assert_eq!(
        check(&core, CheckId::Tunnels).state,
        CheckState::Failed,
        "V13"
    );
    assert!(router_event(&effects), "V14: a state change emits");
}

#[test]
fn v14_the_first_result_of_checks_two_to_four_emits() {
    let mut core = core();
    pass_round(&mut core, 1_000);
    let effects = core.checks_seen(1_000, &good_facts());
    assert!(router_event(&effects), "V14: running turned passed");
}

// ---------------------------------------------------------------------------------------------
// V11: check 2, the version.

fn assert_names(detail: &str, words: &[&str], why: &str) {
    for word in words {
        assert!(
            detail.contains(word),
            "{why}: {detail:?} must name {word:?}"
        );
    }
}

fn assert_not_checked(outcome: &Outcome, word: &str, why: &str) {
    let detail = not_checked_detail(outcome, why);
    assert!(
        detail.to_lowercase().contains(word),
        "{why}: {detail:?} must say {word:?}"
    );
}

#[test]
fn v11_an_unknown_router_type_is_not_checked() {
    let outcome = version_outcome(None, Some("2.13.0"));
    assert_not_checked(
        &outcome,
        "console",
        "V11: no stored console, the type is not known",
    );
}

#[test]
fn v11_a_missing_version_is_not_checked() {
    for kind in [ConsoleKind::Java, ConsoleKind::I2pd] {
        let outcome = version_outcome(Some(kind), None);
        assert_not_checked(
            &outcome,
            "version",
            "V11: the router did not report its version",
        );
    }
}

#[test]
fn v11_the_type_check_comes_before_the_version_check() {
    let outcome = version_outcome(None, None);
    assert!(
        matches!(outcome, Outcome::NotChecked(_)),
        "V11: {outcome:?}"
    );
}

#[test]
fn v11_other_text_is_not_checked_and_the_detail_quotes_it() {
    for text in ["abc", "1.2.3.4", "2.x.0", "v2.5.0", "2..3", "1."] {
        let outcome = version_outcome(Some(ConsoleKind::Java), Some(text));
        assert_names(
            not_checked_detail(&outcome, "V11"),
            &[text],
            "V11: quote the text",
        );
    }
}

#[test]
fn v11_parse_version_reads_one_to_three_integers() {
    assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)), "V11");
    assert_eq!(
        parse_version("2.7"),
        Some((2, 7, 0)),
        "V11: a missing part is 0"
    );
    assert_eq!(
        parse_version("3"),
        Some((3, 0, 0)),
        "V11: a missing part is 0"
    );
}

#[test]
fn v11_parse_version_ignores_the_text_after_a_dash() {
    assert_eq!(parse_version("2.10.0-3"), Some((2, 10, 0)), "V11");
    assert_eq!(
        parse_version("2.4.0-rc1 and more text"),
        Some((2, 4, 0)),
        "V11"
    );
    assert_eq!(parse_version("2.7-beta"), Some((2, 7, 0)), "V11");
}

#[test]
fn v11_parse_version_refuses_other_text() {
    for text in [
        "", "abc", "1.2.3.4", "1.x", "v1.2", "1..2", ".1", "1.", "a.b.c",
    ] {
        assert_eq!(parse_version(text), None, "V11: {text:?}");
    }
}

#[test]
fn v11_the_minimum_versions_are_java_2_4_0_and_i2pd_2_50_0() {
    assert_eq!(JAVA_MIN_VERSION, (2, 4, 0), "V11");
    assert_eq!(I2PD_MIN_VERSION, (2, 50, 0), "V11");
}

fn assert_passed_naming(outcome: &Outcome, words: &[&str], why: &str) {
    assert_names(passed_detail(outcome, why), words, why);
}

fn assert_failed_naming(outcome: &Outcome, words: &[&str], why: &str) {
    assert_names(failed_detail(outcome, why), words, why);
}

#[test]
fn v11_java_at_or_above_2_4_0_passes_and_the_detail_names_both_versions() {
    let java = Some(ConsoleKind::Java);
    assert_passed_naming(
        &version_outcome(java, Some("2.4.0")),
        &["2.4.0"],
        "V11: the minimum",
    );
    assert_passed_naming(
        &version_outcome(java, Some("2.5.1")),
        &["2.5.1", "2.4.0"],
        "V11",
    );
    assert_passed_naming(
        &version_outcome(java, Some("3.0.0")),
        &["3.0.0", "2.4.0"],
        "V11",
    );
}

#[test]
fn v11_java_below_2_4_0_fails_and_the_detail_names_both_versions() {
    let java = Some(ConsoleKind::Java);
    assert_failed_naming(
        &version_outcome(java, Some("2.3.9")),
        &["2.3.9", "2.4.0"],
        "V11",
    );
    assert_failed_naming(
        &version_outcome(java, Some("1.7.2")),
        &["1.7.2", "2.4.0"],
        "V11",
    );
}

#[test]
fn v11_i2pd_at_or_above_2_50_0_passes_and_the_detail_names_both_versions() {
    let i2pd = Some(ConsoleKind::I2pd);
    assert_passed_naming(
        &version_outcome(i2pd, Some("2.50.0")),
        &["2.50.0"],
        "V11: the minimum",
    );
    assert_passed_naming(
        &version_outcome(i2pd, Some("2.58.1")),
        &["2.58.1", "2.50.0"],
        "V11",
    );
}

#[test]
fn v11_i2pd_below_2_50_0_fails_and_the_detail_names_both_versions() {
    let i2pd = Some(ConsoleKind::I2pd);
    assert_failed_naming(
        &version_outcome(i2pd, Some("2.49.0")),
        &["2.49.0", "2.50.0"],
        "V11",
    );
    assert_failed_naming(
        &version_outcome(i2pd, Some("2.4.0")),
        &["2.4.0", "2.50.0"],
        "V11",
    );
}

#[test]
fn v11_each_type_has_its_own_minimum() {
    let java_on_i2pd = version_outcome(Some(ConsoleKind::I2pd), Some("2.13.0"));
    assert!(
        matches!(java_on_i2pd, Outcome::Failed(_)),
        "V11: 2.13.0 is below the i2pd minimum"
    );
    let i2pd_on_java = version_outcome(Some(ConsoleKind::Java), Some("2.50.0"));
    assert!(
        matches!(i2pd_on_java, Outcome::Passed(_)),
        "V11: 2.50.0 is above the Java minimum"
    );
}

#[test]
fn v11_versions_compare_part_by_part_as_numbers() {
    let java = Some(ConsoleKind::Java);
    assert!(
        matches!(version_outcome(java, Some("2.10.0")), Outcome::Passed(_)),
        "V11: 2.10.0 > 2.4.0"
    );
    let i2pd = Some(ConsoleKind::I2pd);
    assert!(
        matches!(version_outcome(i2pd, Some("2.9.0")), Outcome::Failed(_)),
        "V11: 2.9.0 < 2.50.0"
    );
    assert!(
        matches!(version_outcome(i2pd, Some("2.100.0")), Outcome::Passed(_)),
        "V11: 2.100.0 > 2.50.0"
    );
}

#[test]
fn v11_a_missing_part_is_zero_and_a_suffix_is_ignored() {
    let java = Some(ConsoleKind::Java);
    assert!(
        matches!(version_outcome(java, Some("2.4")), Outcome::Passed(_)),
        "V11: 2.4 is 2.4.0"
    );
    assert!(
        matches!(version_outcome(java, Some("2")), Outcome::Failed(_)),
        "V11: 2 is 2.0.0"
    );
    assert!(
        matches!(version_outcome(java, Some("3")), Outcome::Passed(_)),
        "V11: 3 is 3.0.0"
    );
}

#[test]
fn v11_a_suffix_is_ignored() {
    let java = Some(ConsoleKind::Java);
    assert!(
        matches!(version_outcome(java, Some("2.4.0-rc1")), Outcome::Passed(_)),
        "V11"
    );
    assert!(
        matches!(version_outcome(java, Some("2.3.9-9")), Outcome::Failed(_)),
        "V11"
    );
    let i2pd = Some(ConsoleKind::I2pd);
    assert!(
        matches!(version_outcome(i2pd, Some("2.7")), Outcome::Failed(_)),
        "V11: 2.7 is 2.7.0"
    );
}

// ---------------------------------------------------------------------------------------------
// V12: check 3, the outcome of a finding (the files are read in router_checks_outproxy.rs).

#[test]
fn v12_a_clear_http_proxy_passes_and_the_detail_names_the_file() {
    let outcome = outproxy_outcome(&OutproxyFinding::Clear {
        file: "i2pd.conf".into(),
    });
    match outcome {
        Outcome::Passed(Some(detail)) => assert!(detail.contains("i2pd.conf"), "V12: {detail:?}"),
        other => panic!("V12: expected passed with the file name, got {other:?}"),
    }
}

#[test]
fn v12_an_http_proxy_with_outproxies_fails_and_the_detail_names_each() {
    let finding = OutproxyFinding::Listed {
        file: "i2ptunnel.config".into(),
        outproxies: vec!["exit-one.i2p".into(), "exit-two.i2p".into()],
    };
    assert_failed_naming(
        &outproxy_outcome(&finding),
        &["exit-one.i2p", "exit-two.i2p"],
        "V12",
    );
}

#[test]
fn v12_an_unknown_finding_is_not_checked_with_its_reason() {
    let reason = "both a Java I2P and an i2pd configuration use that port";
    match outproxy_outcome(&OutproxyFinding::Unknown(reason.into())) {
        Outcome::NotChecked(detail) => assert!(detail.contains(reason), "V12: {detail:?}"),
        other => panic!("V12: expected not-checked, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// V13: check 4, the network and the tunnels.

#[test]
fn v13_no_statistics_is_not_checked() {
    let outcome = tunnels_outcome(None);
    assert_not_checked(
        &outcome,
        "statistics",
        "V13: the router statistics are not available",
    );
}

#[test]
fn v13_ok_with_one_or_more_client_tunnels_passes_and_names_the_count() {
    for count in [1_u64, 2, 14] {
        let outcome = tunnels_outcome(Some(&stats(Some("OK"), Some(count))));
        let want = count.to_string();
        assert_names(
            passed_detail(&outcome, "V13"),
            &[want.as_str()],
            "V13: the count",
        );
    }
}

#[test]
fn v13_a_status_other_than_ok_fails_and_the_detail_names_it() {
    for network in ["FIREWALLED", "TESTING", "ERROR"] {
        let outcome = tunnels_outcome(Some(&stats(Some(network), Some(3))));
        assert_failed_naming(&outcome, &[network], "V13");
    }
}

#[test]
fn v13_no_client_tunnel_fails_and_the_detail_says_so() {
    let outcome = tunnels_outcome(Some(&stats(Some("OK"), Some(0))));
    match outcome {
        Outcome::Failed(detail) => {
            let lower = detail.to_lowercase();
            assert!(lower.contains("client tunnel"), "V13: {detail:?}");
        }
        other => panic!("V13: expected failed, got {other:?}"),
    }
}

#[test]
fn v13_a_missing_client_figure_is_not_checked_and_the_detail_names_it() {
    let outcome = tunnels_outcome(Some(&stats(Some("OK"), None)));
    assert_not_checked(&outcome, "client", "V13: i2pd never reports tunnels.client");
}

#[test]
fn v13_a_missing_status_is_not_checked_and_the_detail_names_it() {
    let outcome = tunnels_outcome(Some(&stats(None, Some(2))));
    assert_not_checked(&outcome, "status", "V13: the network status is missing");
}

#[test]
fn v13_both_figures_missing_is_not_checked() {
    let outcome = tunnels_outcome(Some(&RouterStats::default()));
    assert!(
        matches!(outcome, Outcome::NotChecked(_)),
        "V13: {outcome:?}"
    );
}

#[test]
fn v13_a_failure_wins_over_a_missing_figure() {
    let testing = tunnels_outcome(Some(&stats(Some("TESTING"), None)));
    assert_failed_naming(&testing, &["TESTING"], "V13: TESTING with no client figure");
    let none = tunnels_outcome(Some(&stats(None, Some(0))));
    assert!(
        matches!(none, Outcome::Failed(_)),
        "V13: zero client tunnels is a failure: {none:?}"
    );
}

#[test]
fn v13_the_details_are_never_empty() {
    let cases = [
        tunnels_outcome(None),
        tunnels_outcome(Some(&stats(Some("OK"), Some(0)))),
        tunnels_outcome(Some(&stats(Some("OK"), None))),
        tunnels_outcome(Some(&stats(None, Some(1)))),
        tunnels_outcome(Some(&stats(Some("FIREWALLED"), Some(1)))),
        version_outcome(None, None),
        version_outcome(Some(ConsoleKind::Java), None),
        version_outcome(Some(ConsoleKind::Java), Some("2.3.0")),
    ];
    for outcome in cases {
        assert!(
            !outcome_detail(&outcome).is_empty(),
            "V3: {outcome:?} needs a detail"
        );
    }
}
