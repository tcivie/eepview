// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R2.4 and R2.5 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! `diag::init`, `diag::recent` and `diag::delete_logs`. The log state is global, so this
//! file calls `init` once and its tests take turns.

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Mutex, MutexGuard, Once, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use eepview_lib::diag::{self, Code, Field};

static INIT: Once = Once::new();
static DIR: OnceLock<PathBuf> = OnceLock::new();
static TURN: Mutex<()> = Mutex::new(());

/// Starts the log in a fresh folder (once) and takes the turn.
fn setup() -> (MutexGuard<'static, ()>, PathBuf) {
    let guard = TURN.lock().unwrap_or_else(PoisonError::into_inner);
    let dir = DIR
        .get_or_init(|| std::env::temp_dir().join(format!("eepview-diag-init-{}", process::id())))
        .clone();
    INIT.call_once(|| diag::init(&dir));
    (guard, dir)
}

fn log_text(dir: &Path) -> String {
    fs::read_to_string(dir.join("eepview.log")).unwrap_or_default()
}

/// Polls up to 3 seconds: the file write may happen on another thread.
fn wait_for(mut check: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        if check() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    check()
}

// R2.4: after `init`, an event reaches `eepview.log` in that folder, as its formatted line.
#[test]
fn r2_4_events_after_init_reach_the_log_file() {
    let (_turn, dir) = setup();
    diag::event(Code::Startup, &[Field::Count(920_001)]);
    let record = diag::recent(50)
        .into_iter()
        .rev()
        .find(|r| diag::format_line(r).contains("count=920001"))
        .expect("the event is in the ring");
    let line = diag::format_line(&record);
    assert!(
        wait_for(|| log_text(&dir).contains(&line)),
        "{line} is in the file"
    );
}

// R2.4: `recent(n)` returns at most n records, oldest first.
#[test]
fn r2_4_recent_is_bounded_and_oldest_first() {
    let (_turn, _dir) = setup();
    for n in 0..5 {
        diag::event(Code::Startup, &[Field::Count(930_000 + n)]);
    }
    let last = diag::recent(3);
    assert_eq!(last.len(), 3);
    assert!(last.windows(2).all(|w| w[0].at <= w[1].at));
    let newest = diag::format_line(&last[2]);
    assert!(newest.contains("count=930004"), "{newest}");
}

// R2.4: `delete_logs` empties the ring and deletes the files.
#[test]
fn r2_4_delete_logs_empties_ring_and_files() {
    let (_turn, dir) = setup();
    diag::event(Code::Startup, &[Field::Count(940_001)]);
    assert!(wait_for(|| log_text(&dir).contains("count=940001")));
    diag::delete_logs().expect("delete");
    assert!(diag::recent(2000).is_empty(), "the ring is empty");
    assert!(!dir.join("eepview.log").exists(), "the log file is gone");
    assert!(!dir.join("eepview.1.log").exists());
    assert!(!dir.join("eepview.2.log").exists());
}

// R3.4: without a crash marker, the last run did not crash.
#[test]
fn r3_4_no_marker_means_no_crash() {
    let (_turn, _dir) = setup();
    assert!(!diag::crashed_last_run());
}
