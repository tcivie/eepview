// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R2.1 to R2.3 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! the ring buffer and the rotating log files. Public API only.

use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};

use eepview_lib::diag::{Code, LogFiles, Record, Ring};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh folder in the system temp folder, removed on drop. It does not exist yet.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        Self(std::env::temp_dir().join(format!("eepview-diag-storage-{}-{n}", process::id())))
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.0.join(name)).unwrap_or_default()
    }

    fn exists(&self, name: &str) -> bool {
        self.0.join(name).exists()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap_or_default();
    }
}

fn record(at: u64) -> Record {
    Record {
        at,
        code: Code::Startup,
        fields: vec![],
    }
}

fn times(records: &[Record]) -> Vec<u64> {
    records.iter().map(|r| r.at).collect()
}

/// A 9-character line: with the newline it takes 10 bytes.
fn line(n: usize) -> String {
    format!("line-{n:04}")
}

// R2.1: the capacity constant.
#[test]
fn r2_1_ring_capacity_is_2000() {
    assert_eq!(Ring::CAPACITY, 2000);
}

// R2.1: a new ring is empty; push grows it.
#[test]
fn r2_1_ring_starts_empty_and_grows() {
    let mut ring = Ring::new(5);
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
    ring.push(record(1));
    ring.push(record(2));
    assert!(!ring.is_empty());
    assert_eq!(ring.len(), 2);
}

// R2.1: `last(n)` gives the newest n records, oldest first.
#[test]
fn r2_1_last_is_oldest_first() {
    let mut ring = Ring::new(10);
    for at in 1..=5 {
        ring.push(record(at));
    }
    assert_eq!(times(&ring.last(3)), [3, 4, 5]);
    assert_eq!(times(&ring.last(99)), [1, 2, 3, 4, 5]);
    assert!(ring.last(0).is_empty());
}

// R2.1: a full ring drops its oldest record.
#[test]
fn r2_1_full_ring_drops_the_oldest() {
    let mut ring = Ring::new(3);
    for at in 1..=5 {
        ring.push(record(at));
    }
    assert_eq!(ring.len(), 3);
    assert_eq!(times(&ring.last(10)), [3, 4, 5]);
}

// R2.1: the default capacity holds 2000 records and drops the 2001st's oldest.
#[test]
fn r2_1_ring_at_default_capacity_keeps_the_newest_2000() {
    let mut ring = Ring::new(Ring::CAPACITY);
    for at in 0..2005 {
        ring.push(record(at));
    }
    assert_eq!(ring.len(), 2000);
    assert_eq!(ring.last(1)[0].at, 2004);
    assert_eq!(ring.last(2000)[0].at, 5);
}

// R2.1: `clear` empties the ring.
#[test]
fn r2_1_clear_empties_the_ring() {
    let mut ring = Ring::new(4);
    ring.push(record(1));
    ring.clear();
    assert!(ring.is_empty());
    assert!(ring.last(10).is_empty());
}

// R2.2: the size and file count constants.
#[test]
fn r2_2_log_file_constants() {
    assert_eq!(LogFiles::MAX_BYTES, 524_288);
    assert_eq!(LogFiles::FILES, 3);
}

// R2.2: `append` writes the line and `\n` to `eepview.log`, and creates a missing folder.
#[test]
fn r2_2_append_writes_line_and_creates_the_folder() {
    let tmp = TempDir::new();
    let nested = tmp.path().join("a").join("b");
    let mut files = LogFiles::new(nested.clone());
    files.append("hello").expect("append");
    files.append("world").expect("append");
    let text = fs::read_to_string(nested.join("eepview.log")).expect("log file");
    assert_eq!(text, "hello\nworld\n");
}

// R2.3: a line that makes the file exactly as large as the limit does not rotate.
#[test]
fn r2_3_no_rotation_up_to_the_limit() {
    let tmp = TempDir::new();
    let mut files = LogFiles::with_limit(tmp.path().to_path_buf(), 20);
    files.append(&line(1)).expect("append");
    files.append(&line(2)).expect("append");
    assert_eq!(tmp.read("eepview.log"), "line-0001\nline-0002\n");
    assert!(!tmp.exists("eepview.1.log"));
}

// R2.3: a line that would exceed the limit starts a new `eepview.log`.
#[test]
fn r2_3_rotation_moves_the_log_to_the_first_backup() {
    let tmp = TempDir::new();
    let mut files = LogFiles::with_limit(tmp.path().to_path_buf(), 25);
    for n in 1..=3 {
        files.append(&line(n)).expect("append");
    }
    assert_eq!(tmp.read("eepview.log"), "line-0003\n");
    assert_eq!(tmp.read("eepview.1.log"), "line-0001\nline-0002\n");
    assert!(!tmp.exists("eepview.2.log"));
}

// R2.3: the oldest file is lost, and there are never more than 3 log files.
#[test]
fn r2_3_third_rotation_drops_the_oldest_file() {
    let tmp = TempDir::new();
    let mut files = LogFiles::with_limit(tmp.path().to_path_buf(), 25);
    for n in 1..=7 {
        files.append(&line(n)).expect("append");
    }
    assert_eq!(tmp.read("eepview.log"), "line-0007\n");
    assert_eq!(tmp.read("eepview.1.log"), "line-0005\nline-0006\n");
    assert_eq!(tmp.read("eepview.2.log"), "line-0003\nline-0004\n");
    let count = fs::read_dir(tmp.path()).expect("dir").count();
    assert_eq!(count, 3, "never more than 3 log files");
}

// R2.3: a line larger than the limit goes into an empty file without rotating.
#[test]
fn r2_3_oversized_line_in_an_empty_file_does_not_rotate() {
    let tmp = TempDir::new();
    let mut files = LogFiles::with_limit(tmp.path().to_path_buf(), 5);
    files
        .append("this line is longer than five bytes")
        .expect("append");
    assert!(tmp.read("eepview.log").starts_with("this line"));
    assert!(!tmp.exists("eepview.1.log"));
}

// R2.3: `paths` lists the existing files, newest first.
#[test]
fn r2_3_paths_list_existing_files_newest_first() {
    let tmp = TempDir::new();
    let mut files = LogFiles::with_limit(tmp.path().to_path_buf(), 25);
    files.append(&line(1)).expect("append");
    let one: Vec<_> = files
        .paths()
        .iter()
        .filter_map(|p| p.file_name())
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(one, ["eepview.log"]);
    for n in 2..=7 {
        files.append(&line(n)).expect("append");
    }
    let all: Vec<_> = files
        .paths()
        .iter()
        .filter_map(|p| p.file_name())
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(all, ["eepview.log", "eepview.1.log", "eepview.2.log"]);
}

// R2.3: `delete_all` removes every log file.
#[test]
fn r2_3_delete_all_removes_every_file() {
    let tmp = TempDir::new();
    let mut files = LogFiles::with_limit(tmp.path().to_path_buf(), 25);
    for n in 1..=7 {
        files.append(&line(n)).expect("append");
    }
    files.delete_all().expect("delete");
    assert!(files.paths().is_empty());
    for name in ["eepview.log", "eepview.1.log", "eepview.2.log"] {
        assert!(!tmp.exists(name), "{name} is gone");
    }
}
