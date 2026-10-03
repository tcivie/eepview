// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Diagnostics: the only logging path of eepview (`docs/wiki/diagnostics-and-bug-reports.md`).
//!
//! [`event`] takes a closed [`Code`] and typed [`Field`]s, never text, so a log line cannot
//! hold an address, a title, a name or an error message. Events go to an in-memory ring and
//! to rotating files in the app log folder. Nothing is ever sent.
//!
//! - [`types`]: codes, fields and the line format.
//! - [`store`]: the ring and the log files.
//! - [`scrub`]: the second layer, on the final report text.
//! - [`sysinfo`] and [`report`]: what a bug report holds.

pub mod report;
pub mod scrub;
pub mod store;
pub mod sysinfo;
pub mod types;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError, TryLockError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub use scrub::{REMOVED, scrub, scrub_with};
pub use store::{LogFiles, Ring};
pub use types::{
    Code, ErrorKind, Field, Level, OpKind, Record, RefuseReason, RouterState, SourceFile,
    StoreKind, TabKind, ThreadName, format_line, utc,
};

/// The name of the crash marker file in the log folder.
pub const CRASH_MARKER: &str = "crashed";

struct State {
    ring: Ring,
    files: Option<LogFiles>,
}

fn cell() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(State {
            ring: Ring::new(Ring::CAPACITY),
            files: None,
        })
    })
}

fn state() -> MutexGuard<'static, State> {
    cell().lock().unwrap_or_else(PoisonError::into_inner)
}

/// The state, unless this thread already holds it (a panic inside [`event`]).
fn try_state() -> Option<MutexGuard<'static, State>> {
    match cell().try_lock() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(p)) => Some(p.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

static CRASHED: AtomicBool = AtomicBool::new(false);

/// The time of the first diagnostics call, for the uptime.
fn started() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// Seconds since the first diagnostics call.
#[must_use]
pub fn uptime_secs() -> u64 {
    started().elapsed().as_secs()
}

/// The current Unix time in seconds.
#[must_use]
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Records one event.
pub fn event(code: Code, fields: &[Field]) {
    started();
    let record = Record {
        at: now_secs(),
        code,
        fields: fields.to_vec(),
    };
    let mut state = state();
    if let Some(files) = state.files.as_mut() {
        // A log that cannot be written must not stop the browser; the ring still has it.
        let _ = files.append(&format_line(&record));
    }
    state.ring.push(record);
}

/// Starts writing events to the log files in `log_dir`, and takes the crash marker.
pub fn init(log_dir: &Path) {
    started();
    CRASHED.store(take_crash_marker(log_dir), Ordering::SeqCst);
    let mut files = LogFiles::new(log_dir.to_path_buf());
    let backlog: Vec<String> = state()
        .ring
        .last(Ring::CAPACITY)
        .iter()
        .map(format_line)
        .collect();
    for line in &backlog {
        let _ = files.append(line);
    }
    state().files = Some(files);
}

/// The last `n` events, oldest first.
#[must_use]
pub fn recent(n: usize) -> Vec<Record> {
    state().ring.last(n)
}

/// Empties the ring and deletes the log files.
///
/// # Errors
///
/// Fails when a log file cannot be removed.
pub fn delete_logs() -> io::Result<()> {
    let mut state = state();
    state.ring.clear();
    let Some(files) = state.files.as_ref() else {
        return Ok(());
    };
    let _ = take_crash_marker(files.dir());
    files.delete_all()
}

/// Report lines: `+<minutes>m <LEVEL> <code>` and the fields, the minutes counted from the
/// oldest record. A report holds no clock time.
#[must_use]
pub fn report_lines(records: &[Record]) -> Vec<String> {
    let first = records.iter().map(|r| r.at).min().unwrap_or_default();
    records
        .iter()
        .map(|record| {
            let line = format_line(record);
            let rest = line.split_once(' ').map_or("", |(_, rest)| rest);
            format!("+{}m {rest}", (record.at - first) / 60)
        })
        .collect()
}

/// The app log folder without Tauri: the folder `app_log_dir()` gives for `identifier`.
#[must_use]
pub fn default_log_dir(identifier: &str) -> Option<PathBuf> {
    let var = |key: &str| {
        std::env::var_os(key)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    log_dir_from(identifier, var("HOME"), var(LOG_BASE_VAR))
}

#[cfg(target_os = "macos")]
const LOG_BASE_VAR: &str = "HOME";
#[cfg(windows)]
const LOG_BASE_VAR: &str = "LOCALAPPDATA";
#[cfg(not(any(target_os = "macos", windows)))]
const LOG_BASE_VAR: &str = "XDG_DATA_HOME";

/// [`default_log_dir`] from the home folder and the system base folder.
#[must_use]
pub fn log_dir_from(
    identifier: &str,
    home: Option<PathBuf>,
    base: Option<PathBuf>,
) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library").join("Logs").join(identifier));
    }
    let base = if cfg!(windows) {
        base
    } else {
        base.or_else(|| home.map(|h| h.join(".local").join("share")))
    };
    base.map(|b| b.join(identifier).join("logs"))
}

/// The log folder, after [`init`].
#[must_use]
pub fn log_dir() -> Option<PathBuf> {
    state().files.as_ref().map(|f| f.dir().to_path_buf())
}

/// True when the last run left a crash marker, until [`dismiss_crash`].
#[must_use]
pub fn crashed_last_run() -> bool {
    CRASHED.load(Ordering::SeqCst)
}

/// Forgets the crash of the last run.
pub fn dismiss_crash() {
    CRASHED.store(false, Ordering::SeqCst);
}

/// Creates the empty crash marker in `dir`.
///
/// # Errors
///
/// Fails when the folder or the file cannot be written.
pub fn write_crash_marker(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(dir.join(CRASH_MARKER), b"")
}

/// Whether the crash marker was in `dir`; removes it.
#[must_use]
pub fn take_crash_marker(dir: &Path) -> bool {
    fs::remove_file(dir.join(CRASH_MARKER)).is_ok()
}

/// The fields of a panic: file name, line and thread. Never the message.
#[must_use]
pub fn panic_fields(file: &str, line: u32, thread: Option<&str>) -> Vec<Field> {
    vec![
        Field::Source(SourceFile::from_path(file)),
        Field::Line(line),
        Field::Thread(ThreadName::of(thread)),
    ]
}

/// Records a panic and writes the crash marker. The panic message is never read.
fn on_panic(info: &std::panic::PanicHookInfo<'_>) {
    let (file, line) = info.location().map_or(("", 0), |l| (l.file(), l.line()));
    let thread = std::thread::current();
    let fields = panic_fields(file, line, thread.name());
    let record = Record {
        at: now_secs(),
        code: Code::Panic,
        fields,
    };
    let Some(mut state) = try_state() else {
        return;
    };
    if let Some(files) = state.files.as_mut() {
        let _ = files.append(&format_line(&record));
        let _ = write_crash_marker(files.dir());
    }
    state.ring.push(record);
}

/// Sets the panic hook of R3.1. It replaces the default hook, so no panic message is printed.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(on_panic));
}
