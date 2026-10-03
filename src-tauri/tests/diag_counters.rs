// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R12.2 to R12.4 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! per-request results are counted, not recorded, and a report shows only coarse buckets.
//! The counters are global, so the tests of this file take turns.

use std::sync::{Mutex, MutexGuard, PoisonError};

use eepview_lib::diag::report::{Report, ReportKind, issue_url, text};
use eepview_lib::diag::sysinfo::{RouterKind, SystemInfo, UptimeBucket};
use eepview_lib::diag::{
    self, Code, ErrorKind, Field, HttpStatus, RefuseReason, RouterState, count, count_bucket,
    counter_lines, counters,
};
use proptest::prelude::*;

static TURN: Mutex<()> = Mutex::new(());

/// Takes the turn and starts from empty counters.
fn fresh() -> MutexGuard<'static, ()> {
    let guard = TURN.lock().unwrap_or_else(PoisonError::into_inner);
    diag::delete_logs().unwrap_or_default();
    guard
}

fn times(n: u64, code: Code, make: impl Fn() -> Field) {
    for _ in 0..n {
        count(code, make());
    }
}

fn shown(entries: &[(Code, Field, u64)]) -> Vec<(String, String, u64)> {
    entries
        .iter()
        .map(|(code, field, n)| (code.as_str().to_owned(), field.to_string(), *n))
        .collect()
}

// R12.2: `count` adds 1 to the session counter of the (code, field) pair.
#[test]
fn r12_2_count_adds_one_per_call() {
    let _turn = fresh();
    times(3, Code::GatekeeperRefused, || {
        Field::Refuse(RefuseReason::NotI2p)
    });
    assert_eq!(
        shown(&counters()),
        [(
            "gatekeeper-refused".to_owned(),
            "refuse=not-i2p".to_owned(),
            3
        )]
    );
    count(Code::GatekeeperRefused, Field::Refuse(RefuseReason::NotI2p));
    assert_eq!(shown(&counters())[0].2, 4);
}

// R12.2: each pair has its own counter.
#[test]
fn r12_2_pairs_count_apart() {
    let _turn = fresh();
    times(2, Code::GatekeeperRefused, || {
        Field::Refuse(RefuseReason::NotI2p)
    });
    times(5, Code::GatekeeperRefused, || {
        Field::Refuse(RefuseReason::Busy)
    });
    times(1, Code::PageLoadFailed, || Field::Status(HttpStatus::S503));
    let found = shown(&counters());
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found.contains(&("gatekeeper-refused".to_owned(), "refuse=busy".to_owned(), 5)));
    assert!(found.contains(&(
        "gatekeeper-refused".to_owned(),
        "refuse=not-i2p".to_owned(),
        2
    )));
    assert!(found.contains(&("page-load-failed".to_owned(), "status=503".to_owned(), 1)));
}

// R12.2: counting records no event: the ring does not grow.
#[test]
fn r12_2_count_records_no_event() {
    let _turn = fresh();
    let before = diag::recent(2000).len();
    times(25, Code::PageLoadFailed, || Field::Status(HttpStatus::S502));
    times(25, Code::GatekeeperRefused, || {
        Field::Refuse(RefuseReason::BadRequest)
    });
    times(25, Code::PageLoadFailed, || {
        Field::Error(ErrorKind::from(std::io::ErrorKind::TimedOut))
    });
    assert_eq!(diag::recent(2000).len(), before, "no record per request");
}

// R12.3: only counters above 0 are listed, sorted by code name and then by field text.
#[test]
fn r12_3_counters_are_sorted_by_code_then_field_text() {
    let _turn = fresh();
    let timed_out = || Field::Error(ErrorKind::from(std::io::ErrorKind::TimedOut));
    times(1, Code::PageLoadFailed, || Field::Status(HttpStatus::S503));
    times(1, Code::GatekeeperRefused, || {
        Field::Refuse(RefuseReason::Busy)
    });
    times(1, Code::PageLoadFailed, || Field::Status(HttpStatus::S500));
    times(1, Code::GatekeeperRefused, || {
        Field::Refuse(RefuseReason::NotI2p)
    });
    times(1, Code::PageLoadFailed, timed_out);
    let keys: Vec<(String, String)> = shown(&counters())
        .into_iter()
        .map(|(c, f, _)| (c, f))
        .collect();
    let expected: Vec<(String, String)> = [
        ("gatekeeper-refused", "refuse=busy"),
        ("gatekeeper-refused", "refuse=not-i2p"),
        ("page-load-failed", "error=timed-out"),
        ("page-load-failed", "status=500"),
        ("page-load-failed", "status=503"),
    ]
    .iter()
    .map(|(c, f)| ((*c).to_owned(), (*f).to_owned()))
    .collect();
    assert_eq!(keys, expected);
}

// R12.3: with nothing counted, there is no counter and no line.
#[test]
fn r12_3_no_counts_no_counters() {
    let _turn = fresh();
    assert!(counters().is_empty());
    assert!(counter_lines().is_empty());
}

// R12.3: the bucket of a number.
#[test]
fn r12_3_count_bucket_table() {
    let table = [
        (0, "0"),
        (1, "1+"),
        (9, "1+"),
        (10, "10+"),
        (99, "10+"),
        (100, "100+"),
        (999, "100+"),
        (1000, "1000+"),
        (123_456, "1000+"),
        (u64::MAX, "1000+"),
    ];
    for (n, bucket) in table {
        assert_eq!(count_bucket(n), bucket, "{n}");
    }
}

// R12.3: a line is `<code> <key>=<value> count=<bucket>`, in the order of `counters`.
#[test]
fn r12_3_counter_lines_show_the_bucket_only() {
    let _turn = fresh();
    times(3, Code::GatekeeperRefused, || {
        Field::Refuse(RefuseReason::NotI2p)
    });
    times(12, Code::PageLoadFailed, || Field::Status(HttpStatus::S502));
    times(150, Code::PageLoadFailed, || {
        Field::Status(HttpStatus::Other5xx)
    });
    assert_eq!(
        counter_lines(),
        [
            "gatekeeper-refused refuse=not-i2p count=1+",
            "page-load-failed status=502 count=10+",
            "page-load-failed status=other-5xx count=100+",
        ]
    );
}

// R12.3: the exact number is not in the line.
#[test]
fn r12_3_counter_lines_never_hold_the_exact_number() {
    let _turn = fresh();
    times(137, Code::PageLoadFailed, || {
        Field::Status(HttpStatus::S504)
    });
    let lines = counter_lines();
    assert_eq!(lines.len(), 1);
    assert!(!lines[0].contains("137"), "{}", lines[0]);
}

// R12.4: `delete_logs` clears the counters.
#[test]
fn r12_4_delete_logs_clears_the_counters() {
    let _turn = fresh();
    times(4, Code::GatekeeperRefused, || {
        Field::Refuse(RefuseReason::Upstream)
    });
    assert!(!counters().is_empty());
    diag::delete_logs().expect("delete");
    assert!(counters().is_empty());
    assert!(counter_lines().is_empty());
}

fn sample_report(log: Vec<String>) -> Report {
    Report {
        kind: ReportKind::General,
        description: "pages fail".to_owned(),
        info: SystemInfo {
            version: "0.1.0".to_owned(),
            commit: "abc1234".to_owned(),
            os: "macOS 14.5".to_owned(),
            arch: "aarch64",
            engine: "WebKit 621.1.15".to_owned(),
            router_kind: RouterKind::I2pd,
            router_version: Some("2.50.0".to_owned()),
            router_state: RouterState::Ok,
            managed: true,
            js_default: true,
            tabs: 1,
            uptime: UptimeBucket::from_secs(30),
        },
        log,
        include_log: true,
    }
}

/// Every counter line is `<code> <key>=<value> count=<bucket>` with a bucket above 0.
fn line_is_coarse(line: &str) -> bool {
    let Some((head, bucket)) = line.rsplit_once(" count=") else {
        return false;
    };
    ["1+", "10+", "100+", "1000+"].contains(&bucket) && head.split(' ').count() == 2
}

/// The status values of the lines, the only number-like value a site can influence.
fn statuses(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|l| l.split(' ').find_map(|w| w.strip_prefix("status=")))
        .map(str::to_owned)
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, failure_persistence: None, ..ProptestConfig::default() })]

    // R12: whatever statuses a site makes the router answer with, and however many requests it
    // makes, the report holds only the five status values and the four buckets.
    #[test]
    fn r12_no_value_a_site_controls_reaches_the_report(
        ops in proptest::collection::vec((any::<u16>(), 1u64..400), 1..12),
    ) {
        let _turn = fresh();
        let mut exact = Vec::new();
        for (code, n) in &ops {
            if let Some(status) = HttpStatus::from_code(*code) {
                times(*n, Code::PageLoadFailed, || Field::Status(status));
                exact.push((*code, *n));
            }
        }
        let lines = counter_lines();
        prop_assert!(lines.iter().all(|l| line_is_coarse(l)), "{:?}", lines);
        for status in statuses(&lines) {
            prop_assert!(
                ["500", "502", "503", "504", "other-5xx"].contains(&status.as_str()),
                "{}", status
            );
        }
        let rep = sample_report(lines.clone());
        let body = text(&rep);
        let (url, _) = issue_url(&rep);
        for (code, _) in exact {
            if ![500, 502, 503, 504].contains(&code) {
                let raw = format!("status={code}");
                prop_assert!(!body.contains(&raw), "{}", body);
                prop_assert!(!url.contains(&raw), "{}", url);
            }
        }
        for line in &lines {
            prop_assert!(body.contains(line.as_str()), "{}", line);
        }
    }

    // R12.3: the bucket depends only on the range, so two counts in a range look the same.
    #[test]
    fn r12_3_buckets_hide_the_exact_number(a in 1u64..10, b in 1u64..10, c in 10u64..100, d in 10u64..100, e in 100u64..1000, f in 100u64..1000, g in 1000u64..u64::MAX, h in 1000u64..u64::MAX) {
        prop_assert_eq!(count_bucket(a), count_bucket(b));
        prop_assert_eq!(count_bucket(c), count_bucket(d));
        prop_assert_eq!(count_bucket(e), count_bucket(f));
        prop_assert_eq!(count_bucket(g), count_bucket(h));
        let all = [count_bucket(a), count_bucket(c), count_bucket(e), count_bucket(g)];
        prop_assert_eq!(all, ["1+", "10+", "100+", "1000+"]);
    }
}
