// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement test R16.1 of `docs/wiki/diagnostics-and-bug-reports.md`: the panic hook writes
//! the crash marker even when another thread holds the log lock. Unix only: the test blocks
//! the log writer on a named pipe, and a thread that waits in a write holds the lock.
#![cfg(unix)]

use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant};

use eepview_lib::diag::{self, Code};

fn wait_for(mut check: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if check() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    check()
}

// R16.1: another thread is stuck in a log write, and a panic still leaves the marker.
#[test]
fn r16_1_marker_is_written_while_another_thread_holds_the_log_lock() {
    let dir: PathBuf = std::env::temp_dir().join(format!("eepview-diag-lock-{}", process::id()));
    diag::init(&dir);
    diag::install_panic_hook();
    let log = dir.join("eepview.log");
    fs::remove_file(&log).unwrap_or_default();
    let made = Command::new("mkfifo")
        .arg(&log)
        .status()
        .is_ok_and(|s| s.success());
    if !made {
        return; // no mkfifo on this machine: the test cannot hold the lock
    }

    // This thread opens the pipe for writing and waits for a reader, with the log lock held.
    let writer = thread::spawn(|| diag::event(Code::Startup, &[]));
    thread::sleep(Duration::from_millis(300));

    let panicker = thread::Builder::new()
        .name("gatekeeper".to_owned())
        .spawn(|| std::panic::panic_any("PRIVATE-PANIC-TEXT"));
    assert!(
        panicker
            .map(thread::JoinHandle::join)
            .is_ok_and(|r| r.is_err())
    );
    assert!(
        wait_for(|| dir.join("crashed").is_file()),
        "the crash marker must exist while another thread holds the log lock"
    );

    // Let the blocked writer finish: a reader opens the pipe.
    let reader = thread::spawn(move || {
        if let Ok(mut pipe) = File::open(&log) {
            std::io::copy(&mut pipe, &mut std::io::sink()).unwrap_or_default();
        }
    });
    wait_for(|| writer.is_finished());
    wait_for(|| reader.is_finished());
    fs::remove_dir_all(&dir).unwrap_or_default();
}
