// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R3.2 to R3.4 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! panic fields, the crash marker, and `init` taking the marker.

use std::fs;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

use eepview_lib::diag::{self, Field};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh folder in the system temp folder, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("eepview-diag-crash-{}-{n}", process::id()));
        fs::create_dir_all(&dir).unwrap_or_default();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap_or_default();
    }
}

fn shown(fields: &[Field]) -> Vec<String> {
    fields.iter().map(ToString::to_string).collect()
}

// R3.2: the panic fields are the file name, the line and the thread name.
#[test]
fn r3_2_panic_fields_are_file_line_thread() {
    let fields = diag::panic_fields("src/net/gatekeeper.rs", 42, Some("gatekeeper"));
    assert_eq!(
        shown(&fields),
        ["file=gatekeeper.rs", "line=42", "thread=gatekeeper"]
    );
}

// R1.6 + R3.2: a source file that is not one of eepview's own is reported as `other`.
#[test]
fn r3_2_unknown_source_file_is_other() {
    let fields = diag::panic_fields("/Users/zqalice/work/zq-secret.rs", 7, None);
    assert_eq!(shown(&fields)[0], "file=other");
}

// R3.2: a panic in an unnamed or unknown thread reports the thread as `other`.
#[test]
fn r3_2_unknown_threads_are_other() {
    let none = diag::panic_fields("a.rs", 1, None);
    assert_eq!(shown(&none)[2], "thread=other");
    let odd = diag::panic_fields("a.rs", 1, Some("worker-for-alice"));
    assert_eq!(shown(&odd)[2], "thread=other");
}

// R3.2: the folder of the source path is not in the fields.
#[test]
fn r3_2_panic_fields_hold_no_folder() {
    let fields = diag::panic_fields("/Users/someone/work/eepview/src/lib.rs", 7, None);
    let text = shown(&fields).join(" ");
    assert!(!text.contains("someone"), "{text}");
    assert!(!text.contains('/'), "{text}");
}

// R3.3: the marker file name.
#[test]
fn r3_3_marker_name_is_crashed() {
    assert_eq!(diag::CRASH_MARKER, "crashed");
}

// R3.3: `write_crash_marker` creates an empty `crashed` file.
#[test]
fn r3_3_write_creates_an_empty_marker() {
    let tmp = TempDir::new();
    diag::write_crash_marker(&tmp.0).expect("write");
    let marker = tmp.0.join("crashed");
    assert!(marker.is_file());
    assert_eq!(fs::metadata(&marker).expect("metadata").len(), 0);
}

// R3.3: `take_crash_marker` says whether the marker existed, and removes it.
#[test]
fn r3_3_take_reports_and_removes_the_marker() {
    let tmp = TempDir::new();
    assert!(!diag::take_crash_marker(&tmp.0), "no marker yet");
    diag::write_crash_marker(&tmp.0).expect("write");
    assert!(diag::take_crash_marker(&tmp.0), "the marker existed");
    assert!(!tmp.0.join("crashed").exists(), "the marker is removed");
    assert!(!diag::take_crash_marker(&tmp.0), "taken only once");
}

// R3.4: `init` takes the marker. `crashed_last_run` is true until `dismiss_crash`.
#[test]
fn r3_4_init_takes_the_marker_and_dismiss_clears_it() {
    let tmp = TempDir::new();
    diag::write_crash_marker(&tmp.0).expect("write");
    diag::init(&tmp.0);
    assert!(diag::crashed_last_run(), "the last run crashed");
    assert!(diag::crashed_last_run(), "it stays true until dismissed");
    assert!(!tmp.0.join("crashed").exists(), "init removed the marker");
    diag::dismiss_crash();
    assert!(!diag::crashed_last_run(), "dismissed");
}
