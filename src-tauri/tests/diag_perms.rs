// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R14.1 and R14.2 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! on Unix the log folder is `0700`, the log files and the crash marker are `0600`, and a
//! symbolic link in place of a file makes the write fail. Public API only.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use eepview_lib::diag::{self, Code, Field, LogFiles};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh folder name in the system temp folder, removed on drop. It does not exist yet.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        Self(std::env::temp_dir().join(format!("eepview-diag-perms-{}-{n}", process::id())))
    }

    /// Makes the folder, with the given mode.
    fn make(&self, mode: u32) {
        fs::create_dir_all(&self.0).unwrap_or_default();
        fs::set_permissions(&self.0, fs::Permissions::from_mode(mode)).unwrap_or_default();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700)).unwrap_or_default();
        fs::remove_dir_all(&self.0).unwrap_or_default();
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o777)
        .unwrap_or_default()
}

// R14.1: a log folder that `append` creates has mode 0700, and the log file 0600.
#[test]
fn r14_1_new_log_folder_is_0700_and_log_file_is_0600() {
    let tmp = TempDir::new();
    let dir = tmp.0.join("logs");
    let mut files = LogFiles::new(dir.clone());
    files.append("line").expect("append");
    assert_eq!(mode(&dir), 0o700, "folder");
    assert_eq!(mode(&dir.join("eepview.log")), 0o600, "log file");
}

// R14.1: every rotated log file keeps mode 0600.
#[test]
fn r14_1_rotated_files_are_0600() {
    let tmp = TempDir::new();
    let mut files = LogFiles::with_limit(tmp.0.clone(), 25);
    for n in 1..=7 {
        files.append(&format!("line-{n:04}")).expect("append");
    }
    for name in ["eepview.log", "eepview.1.log", "eepview.2.log"] {
        assert_eq!(mode(&tmp.0.join(name)), 0o600, "{name}");
    }
}

// R14.1: the crash marker is 0600.
#[test]
fn r14_1_crash_marker_is_0600() {
    let tmp = TempDir::new();
    tmp.make(0o700);
    diag::write_crash_marker(&tmp.0).expect("write");
    assert_eq!(mode(&tmp.0.join("crashed")), 0o600);
}

// R14.1: `init` sets 0700 on a log folder that exists, and the files in it are 0600.
// `init` is global, so this is the only test of this file that calls it.
#[test]
fn r14_1_init_sets_0700_on_an_existing_folder() {
    let tmp = TempDir::new();
    tmp.make(0o755);
    diag::init(&tmp.0);
    assert_eq!(mode(&tmp.0), 0o700, "folder");
    diag::event(Code::Startup, &[Field::Count(970_001)]);
    let log = tmp.0.join("eepview.log");
    let end = Instant::now() + Duration::from_secs(3);
    while !log.exists() && Instant::now() < end {
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(mode(&log), 0o600, "log file");
}

// R14.2: a symbolic link in place of the log file makes the write fail, and the target stays as it is.
#[test]
fn r14_2_log_file_symlink_is_not_followed() {
    let tmp = TempDir::new();
    tmp.make(0o700);
    let target = tmp.0.join("victim.txt");
    fs::write(&target, "precious\n").expect("target");
    symlink(&target, tmp.0.join("eepview.log")).expect("link");
    let mut files = LogFiles::new(tmp.0.clone());
    assert!(files.append("line").is_err(), "the write must fail");
    assert_eq!(
        fs::read_to_string(&target).unwrap_or_default(),
        "precious\n"
    );
}

// R14.2: a dangling link does not make eepview create the file it points to.
#[test]
fn r14_2_dangling_symlink_creates_nothing() {
    let tmp = TempDir::new();
    tmp.make(0o700);
    let target = tmp.0.join("not-yet.txt");
    symlink(&target, tmp.0.join("eepview.log")).expect("link");
    let mut files = LogFiles::new(tmp.0.clone());
    assert!(files.append("line").is_err(), "the write must fail");
    assert!(!target.exists(), "the link target was created");
}

// R14.2: a symbolic link in place of the crash marker makes the write fail.
#[test]
fn r14_2_crash_marker_symlink_is_not_followed() {
    let tmp = TempDir::new();
    tmp.make(0o700);
    let target = tmp.0.join("victim.txt");
    fs::write(&target, "precious\n").expect("target");
    symlink(&target, tmp.0.join("crashed")).expect("link");
    assert!(
        diag::write_crash_marker(&tmp.0).is_err(),
        "the write must fail"
    );
    assert_eq!(
        fs::read_to_string(&target).unwrap_or_default(),
        "precious\n"
    );
}

// R14.2: a rotated file that is a link is not followed either.
#[test]
fn r14_2_rotation_does_not_write_through_a_link() {
    let tmp = TempDir::new();
    tmp.make(0o700);
    let target = tmp.0.join("victim.txt");
    fs::write(&target, "precious\n").expect("target");
    let mut files = LogFiles::with_limit(tmp.0.clone(), 25);
    for n in 1..=2 {
        files.append(&format!("line-{n:04}")).expect("append");
    }
    symlink(&target, tmp.0.join("eepview.1.log")).expect("link");
    for n in 3..=6 {
        // The result is not the point: a failure or a rotation is fine, a write through the link is not.
        files.append(&format!("line-{n:04}")).unwrap_or_default();
    }
    assert_eq!(
        fs::read_to_string(&target).unwrap_or_default(),
        "precious\n"
    );
}
