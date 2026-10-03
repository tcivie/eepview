// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The typed vocabulary of the diagnostics log: event codes and field values. No type here
//! holds free text, so a log line cannot carry an address, a name or a message.

use std::fmt;
use std::io;

/// How serious an event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Normal operation.
    Info,
    /// Something failed, and eepview went on.
    Warn,
    /// Something failed that the user may notice.
    Error,
}

impl Level {
    /// `INFO`, `WARN` or `ERROR`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        ["INFO", "WARN", "ERROR"][self as usize]
    }
}

/// Every event eepview records. Closed: a new event needs a new variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// The app started.
    Startup,
    /// The app is about to exit.
    Shutdown,
    /// Tauri could not start.
    StartFailed,
    /// A thread panicked.
    Panic,
    /// VERIFY passed after it had not.
    VerifyPassed,
    /// VERIFY failed after it had passed (or at the first check).
    VerifyFailed,
    /// The router became usable.
    RouterUp,
    /// The router stopped being usable.
    RouterDown,
    /// The gatekeeper refused a request.
    GatekeeperRefused,
    /// The gatekeeper could not start.
    GatekeeperStartFailed,
    /// The router answered 5xx, or a forwarded request failed.
    PageLoadFailed,
    /// A tab webview could not be built.
    WebviewCreateFailed,
    /// An engine call failed.
    EngineCallFailed,
    /// A find in page failed.
    FindFailed,
    /// A zoom change failed.
    ZoomFailed,
    /// A store file was broken.
    StoreCorrupt,
    /// A thread could not start, or the main thread queue failed.
    ThreadFailed,
    /// A report was written and the issue page opened.
    ReportOpened,
    /// A report could not be written or opened.
    ReportFailed,
}

/// The kebab-case names of [`Code`], in variant order.
const CODE_NAMES: [&str; 19] = [
    "startup",
    "shutdown",
    "start-failed",
    "panic",
    "verify-passed",
    "verify-failed",
    "router-up",
    "router-down",
    "gatekeeper-refused",
    "gatekeeper-start-failed",
    "page-load-failed",
    "webview-create-failed",
    "engine-call-failed",
    "find-failed",
    "zoom-failed",
    "store-corrupt",
    "thread-failed",
    "report-opened",
    "report-failed",
];

impl Code {
    /// The kebab-case name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        CODE_NAMES[self as usize]
    }

    /// How serious the event is.
    #[must_use]
    pub fn level(self) -> Level {
        match self {
            Self::StartFailed
            | Self::Panic
            | Self::GatekeeperStartFailed
            | Self::WebviewCreateFailed
            | Self::ThreadFailed => Level::Error,
            Self::VerifyFailed
            | Self::RouterDown
            | Self::PageLoadFailed
            | Self::EngineCallFailed
            | Self::FindFailed
            | Self::ZoomFailed
            | Self::StoreCorrupt
            | Self::ReportFailed => Level::Warn,
            _ => Level::Info,
        }
    }
}

/// Declares a field enum whose variants map one to one to names.
macro_rules! named_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $text:literal,)+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($(#[$vmeta])* $variant,)+
        }

        impl $name {
            /// The name in a log line.
            #[must_use]
            pub fn as_str(self) -> &'static str {
                [$($text,)+][self as usize]
            }
        }
    };
}

named_enum!(
    /// The router state, as the contract names it.
    RouterState {
        /// VERIFY has not answered yet.
        Verifying => "verifying",
        /// Verified, and the gatekeeper runs.
        Ok => "ok",
        /// Nothing answers, or the gatekeeper could not start.
        Down => "down",
        /// Something answers, but not an I2P router.
        NotI2p => "not-i2p",
        /// The user paused the connection.
        Paused => "paused",
    }
);

impl RouterState {
    /// The state of a contract state string; unknown strings count as down.
    #[must_use]
    pub fn from_contract(state: &str) -> Self {
        match state {
            "verifying" => Self::Verifying,
            "ok" => Self::Ok,
            "not-i2p" => Self::NotI2p,
            "paused" => Self::Paused,
            _ => Self::Down,
        }
    }
}

named_enum!(
    /// Why the gatekeeper refused a request.
    RefuseReason {
        /// The host is not an I2P name.
        NotI2p => "not-i2p",
        /// The request head was broken.
        BadRequest => "bad-request",
        /// A body without a length.
        LengthRequired => "length-required",
        /// The router is not verified or did not answer.
        Upstream => "upstream",
        /// Too many connections.
        Busy => "busy",
    }
);

named_enum!(
    /// The kind of an error. The error text is never logged.
    ErrorKind {
        /// A file or entry is missing.
        NotFound => "not-found",
        /// Access denied.
        PermissionDenied => "permission-denied",
        /// The connection was refused.
        ConnectionRefused => "connection-refused",
        /// The connection was reset.
        ConnectionReset => "connection-reset",
        /// The connection was aborted.
        ConnectionAborted => "connection-aborted",
        /// The other side closed.
        BrokenPipe => "broken-pipe",
        /// A timeout.
        TimedOut => "timed-out",
        /// The address is in use.
        AddrInUse => "addr-in-use",
        /// The address is not available.
        AddrNotAvailable => "addr-not-available",
        /// A bad input value.
        InvalidInput => "invalid-input",
        /// Bad data.
        InvalidData => "invalid-data",
        /// The data ended early.
        UnexpectedEof => "unexpected-eof",
        /// A write wrote nothing.
        WriteZero => "write-zero",
        /// Interrupted.
        Interrupted => "interrupted",
        /// Not supported here.
        Unsupported => "unsupported",
        /// Out of memory.
        OutOfMemory => "out-of-memory",
        /// A parse error.
        Parse => "parse",
        /// A Tauri or webview error.
        Webview => "webview",
        /// An error of the platform bridge.
        Platform => "platform",
        /// Anything else.
        Other => "other",
    }
);

impl From<io::ErrorKind> for ErrorKind {
    fn from(kind: io::ErrorKind) -> Self {
        use io::ErrorKind as K;
        match kind {
            K::NotFound => Self::NotFound,
            K::PermissionDenied => Self::PermissionDenied,
            K::ConnectionRefused => Self::ConnectionRefused,
            K::ConnectionReset => Self::ConnectionReset,
            K::ConnectionAborted => Self::ConnectionAborted,
            K::BrokenPipe => Self::BrokenPipe,
            K::TimedOut => Self::TimedOut,
            K::AddrInUse => Self::AddrInUse,
            K::AddrNotAvailable => Self::AddrNotAvailable,
            K::InvalidInput => Self::InvalidInput,
            K::InvalidData => Self::InvalidData,
            K::UnexpectedEof => Self::UnexpectedEof,
            K::WriteZero => Self::WriteZero,
            K::Interrupted => Self::Interrupted,
            K::Unsupported => Self::Unsupported,
            K::OutOfMemory => Self::OutOfMemory,
            _ => Self::Other,
        }
    }
}

impl From<&io::Error> for ErrorKind {
    fn from(error: &io::Error) -> Self {
        error.kind().into()
    }
}

impl From<&tauri::Error> for ErrorKind {
    fn from(error: &tauri::Error) -> Self {
        match error {
            tauri::Error::Io(e) => e.into(),
            _ => Self::Webview,
        }
    }
}

impl From<&serde_json::Error> for ErrorKind {
    fn from(_: &serde_json::Error) -> Self {
        Self::Parse
    }
}

named_enum!(
    /// What a tab shows.
    TabKind {
        /// A bundled `eepview://` page.
        Internal => "internal",
        /// An I2P site.
        Web => "web",
    }
);

named_enum!(
    /// A JSON store.
    StoreKind {
        /// `bookmarks.json`.
        Bookmarks => "bookmarks",
        /// `history.json`.
        History => "history",
        /// `settings.json`.
        Settings => "settings",
        /// `sites.json`.
        Sites => "sites",
    }
);

named_enum!(
    /// The operation that failed.
    OpKind {
        /// Reload.
        Reload => "reload",
        /// Reload without the cache.
        HardReload => "hard-reload",
        /// Zoom.
        Zoom => "zoom",
        /// Find in page.
        Find => "find",
        /// Clear the find highlights.
        FindClear => "find-clear",
        /// Back.
        Back => "back",
        /// Forward.
        Forward => "forward",
        /// Stop.
        Stop => "stop",
        /// Engine hardening.
        Harden => "harden",
        /// The hover hook.
        Hover => "hover",
        /// The first load of a tab.
        FirstLoad => "first-load",
        /// The engine request filter.
        EngineFilter => "engine-filter",
        /// The main thread queue.
        MainThread => "main-thread",
        /// A thread start.
        Spawn => "spawn",
        /// Show the report file.
        Reveal => "reveal",
        /// Open the issue page.
        OpenUrl => "open-url",
        /// Write the report file.
        WriteReport => "write-report",
    }
);

named_enum!(
    /// A thread of eepview, or `Other` for any thread it did not name.
    ThreadName {
        /// The main thread.
        Main => "main",
        /// The router watcher.
        RouterWatch => "router-watch",
        /// A one-off router check.
        RouterCheck => "router-check",
        /// The gatekeeper accept loop.
        Gatekeeper => "gatekeeper",
        /// Any other thread.
        Other => "other",
    }
);

impl ThreadName {
    /// The variant of a thread name.
    #[must_use]
    pub fn of(name: Option<&str>) -> Self {
        match name {
            Some("main") => Self::Main,
            Some("router-watch") => Self::RouterWatch,
            Some("router-check") => Self::RouterCheck,
            Some("gatekeeper") => Self::Gatekeeper,
            _ => Self::Other,
        }
    }
}

/// Most characters of a [`SourceFile`].
const MAX_SOURCE: usize = 64;

/// A source file name with no folder, for a panic location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFile {
    bytes: [u8; MAX_SOURCE],
    len: usize,
}

impl SourceFile {
    /// The part of `path` after the last `/` or `\`, with only `A-Z a-z 0-9 _ . -`, at most
    /// 64 characters.
    #[must_use]
    pub fn from_path(path: &str) -> Self {
        let name = path.rsplit(['/', '\\']).next().unwrap_or_default();
        let mut bytes = [0u8; MAX_SOURCE];
        let mut len = 0;
        for b in name.bytes().filter(|b| is_name_byte(*b)).take(MAX_SOURCE) {
            bytes[len] = b;
            len += 1;
        }
        Self { bytes, len }
    }

    /// The file name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or_default()
    }
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"_.-".contains(&b)
}

/// One typed value of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// `router=`.
    Router(RouterState),
    /// `refuse=`.
    Refuse(RefuseReason),
    /// `error=`.
    Error(ErrorKind),
    /// `tab=`.
    Tab(TabKind),
    /// `store=`.
    Store(StoreKind),
    /// `op=`.
    Op(OpKind),
    /// `thread=`.
    Thread(ThreadName),
    /// `file=`.
    Source(SourceFile),
    /// `line=`.
    Line(u32),
    /// `ms=`.
    DurationMs(u64),
    /// `count=`.
    Count(u64),
    /// `status=`.
    Status(u16),
    /// `managed=`.
    Managed(bool),
    /// `js=`.
    Js(bool),
    /// `ok=`.
    Ok(bool),
}

impl Field {
    /// The key in a log line.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Router(_) => "router",
            Self::Refuse(_) => "refuse",
            Self::Error(_) => "error",
            Self::Tab(_) => "tab",
            Self::Store(_) => "store",
            Self::Op(_) => "op",
            Self::Thread(_) => "thread",
            Self::Source(_) => "file",
            Self::Line(_) => "line",
            Self::DurationMs(_) => "ms",
            Self::Count(_) => "count",
            Self::Status(_) => "status",
            Self::Managed(_) => "managed",
            Self::Js(_) => "js",
            Self::Ok(_) => "ok",
        }
    }

    fn write_value(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Router(v) => f.write_str(v.as_str()),
            Self::Refuse(v) => f.write_str(v.as_str()),
            Self::Error(v) => f.write_str(v.as_str()),
            Self::Tab(v) => f.write_str(v.as_str()),
            Self::Store(v) => f.write_str(v.as_str()),
            Self::Op(v) => f.write_str(v.as_str()),
            Self::Thread(v) => f.write_str(v.as_str()),
            Self::Source(v) => f.write_str(v.as_str()),
            Self::Line(n) => write!(f, "{n}"),
            Self::DurationMs(n) | Self::Count(n) => write!(f, "{n}"),
            Self::Status(n) => write!(f, "{n}"),
            Self::Managed(b) | Self::Js(b) | Self::Ok(b) => write!(f, "{b}"),
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}=", self.key())?;
        self.write_value(f)
    }
}

/// One recorded event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Unix seconds.
    pub at: u64,
    /// The event.
    pub code: Code,
    /// Its typed values.
    pub fields: Vec<Field>,
}

/// `<utc> <LEVEL> <code>` and ` key=value` per field.
#[must_use]
pub fn format_line(record: &Record) -> String {
    let mut line = format!(
        "{} {} {}",
        utc(record.at),
        record.code.level().as_str(),
        record.code.as_str()
    );
    for field in &record.fields {
        line.push(' ');
        line.push_str(&field.to_string());
    }
    line
}

/// The date and time parts of a Unix time: (year, month, day, hour, minute, second).
#[must_use]
pub fn civil(secs: u64) -> (i64, u32, u32, u64, u64, u64) {
    let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX / 2);
    let rest = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    (y, m, d, rest / 3600, rest % 3600 / 60, rest % 60)
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn utc(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = civil(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Days since 1970-01-01 to a proleptic Gregorian date (H. Hinnant, `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_convert() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc(1_791_028_800), "2026-10-03T12:00:00Z");
    }

    #[test]
    fn every_name_is_kebab_case() {
        for name in CODE_NAMES {
            assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '-'));
        }
        assert_eq!(Code::ReportFailed.as_str(), "report-failed");
        assert_eq!(RouterState::from_contract("x"), RouterState::Down);
        assert_eq!(RouterState::from_contract("paused"), RouterState::Paused);
    }
}
