// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement test R3.1 of `docs/wiki/diagnostics-and-bug-reports.md`: the panic hook
//! records `Code::Panic` with file, line and thread, and never the panic message.
//! The hook and the log are global, so this file sets them up once and its tests take turns.

use std::fs;
use std::path::PathBuf;
use std::process;
use std::sync::{Mutex, MutexGuard, Once, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use eepview_lib::diag::{self, Record};

static INIT: Once = Once::new();
static DIR: OnceLock<PathBuf> = OnceLock::new();
static TURN: Mutex<()> = Mutex::new(());

fn setup() -> (MutexGuard<'static, ()>, PathBuf) {
    let guard = TURN.lock().unwrap_or_else(PoisonError::into_inner);
    let dir = DIR
        .get_or_init(|| std::env::temp_dir().join(format!("eepview-diag-panic-{}", process::id())))
        .clone();
    INIT.call_once(|| {
        diag::init(&dir);
        diag::install_panic_hook();
    });
    (guard, dir)
}

fn panic_records() -> Vec<Record> {
    diag::recent(2000)
        .into_iter()
        .filter(|r| r.code.as_str() == "panic")
        .collect()
}

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

/// Helpers that panic on purpose. Test code, as far as clippy is concerned.
#[cfg(test)]
mod panics {
    use std::thread;

    /// Panics in a thread with the given name, and waits for the thread to end.
    pub fn panic_in(name: Option<&str>, message: &'static str) {
        let mut builder = thread::Builder::new();
        if let Some(name) = name {
            builder = builder.name(name.to_owned());
        }
        let joined = builder
            .spawn(move || std::panic::panic_any(message))
            .map(thread::JoinHandle::join);
        assert!(joined.is_ok_and(|r| r.is_err()), "the thread panicked");
    }
}

use panics::panic_in;

// R3.1: the hook records a `panic` event with `file`, `line` and `thread`, in that order.
#[test]
fn r3_1_hook_records_file_line_thread() {
    let (_turn, _dir) = setup();
    let before = panic_records().len();
    panic_in(Some("gatekeeper"), "PRIVATE-PANIC-TEXT-ONE");
    let records = panic_records();
    assert_eq!(records.len(), before + 1, "one panic, one event");
    let fields: Vec<String> = records[before]
        .fields
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(fields.len(), 3, "{fields:?}");
    assert!(fields[0].starts_with("file="), "{fields:?}");
    assert!(fields[1].starts_with("line="), "{fields:?}");
    assert_eq!(fields[2], "thread=gatekeeper");
}

// R3.1: a thread with an unknown name is `other`.
#[test]
fn r3_1_hook_reports_unknown_threads_as_other() {
    let (_turn, _dir) = setup();
    let before = panic_records().len();
    panic_in(Some("job-for-alice"), "PRIVATE-PANIC-TEXT-TWO");
    let records = panic_records();
    let last = diag::format_line(&records[before]);
    assert!(last.ends_with("thread=other"), "{last}");
}

// R3.1: the panic message is in no record and in no file.
#[test]
fn r3_1_hook_never_records_the_message() {
    let (_turn, dir) = setup();
    panic_in(None, "PRIVATE-PANIC-TEXT-THREE http://secret.i2p/x");
    for record in diag::recent(2000) {
        let line = diag::format_line(&record);
        assert!(!line.contains("PRIVATE-PANIC-TEXT"), "{line}");
        assert!(!line.contains("secret.i2p"), "{line}");
    }
    assert!(wait_for(|| dir.join("crashed").exists()));
    let file = fs::read_to_string(dir.join("eepview.log")).unwrap_or_default();
    assert!(!file.contains("PRIVATE-PANIC-TEXT"), "{file}");
    assert!(!file.contains("secret.i2p"), "{file}");
}

// R3.1: the hook writes the crash marker in the log folder.
#[test]
fn r3_1_hook_writes_the_crash_marker() {
    let (_turn, dir) = setup();
    fs::remove_file(dir.join("crashed")).unwrap_or_default();
    panic_in(Some("main"), "PRIVATE-PANIC-TEXT-FOUR");
    assert!(
        wait_for(|| dir.join("crashed").is_file()),
        "the marker exists"
    );
}
