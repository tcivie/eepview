// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R1.1 to R1.9 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! the typed event log (`eepview_lib::diag`). Public API only.

use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use eepview_lib::diag::{
    self, Code, ErrorKind, Field, Level, OpKind, Record, RefuseReason, RouterState, SourceFile,
    StoreKind, TabKind, ThreadName,
};
use proptest::prelude::*;

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// The newest record in the ring whose formatted line holds `needle`.
fn find_record(needle: &str) -> Option<Record> {
    diag::recent(2000)
        .into_iter()
        .rev()
        .find(|r| diag::format_line(r).contains(needle))
}

// R1.1: `event` records the current UTC second, the code and the fields, in order.
#[test]
fn r1_1_event_records_time_code_and_fields() {
    let before = now_secs();
    diag::event(
        Code::PageLoadFailed,
        &[Field::Count(910_001), Field::Status(502)],
    );
    let after = now_secs();
    let record = find_record("count=910001").expect("the event is in the ring");
    assert!(
        record.at >= before && record.at <= after,
        "at={}",
        record.at
    );
    assert_eq!(record.code.as_str(), "page-load-failed");
    let shown: Vec<String> = record.fields.iter().map(ToString::to_string).collect();
    assert_eq!(shown, ["count=910001", "status=502"]);
}

// R1.1: an event without fields is recorded with no fields.
#[test]
fn r1_1_event_without_fields_has_none() {
    diag::event(Code::Shutdown, &[]);
    let record = diag::recent(2000)
        .into_iter()
        .rev()
        .find(|r| r.code.as_str() == "shutdown")
        .expect("the event is in the ring");
    assert!(record.fields.is_empty());
}

// R1.2: `Field` has exactly these variants. The match has no wildcard, so a new variant
// (for example one that carries a `String`) breaks this test at compile time.
fn payload_kind(field: &Field) -> &'static str {
    match field {
        Field::Router(_)
        | Field::Refuse(_)
        | Field::Error(_)
        | Field::Tab(_)
        | Field::Store(_)
        | Field::Op(_)
        | Field::Thread(_)
        | Field::Source(_) => "enum",
        Field::Line(_) | Field::DurationMs(_) | Field::Count(_) | Field::Status(_) => "number",
        Field::Managed(_) | Field::Js(_) | Field::Ok(_) => "bool",
    }
}

// R1.2: every payload is an enum, a number or a bool. There is no text payload.
#[test]
fn r1_2_field_payloads_are_enums_numbers_or_bools() {
    let samples = [
        (Field::Router(RouterState::Ok), "enum"),
        (Field::Refuse(RefuseReason::Busy), "enum"),
        (
            Field::Error(ErrorKind::from(io::ErrorKind::TimedOut)),
            "enum",
        ),
        (Field::Tab(TabKind::Web), "enum"),
        (Field::Store(StoreKind::Sites), "enum"),
        (Field::Op(OpKind::Reload), "enum"),
        (Field::Thread(ThreadName::Main), "enum"),
        (Field::Source(SourceFile::from_path("a.rs")), "enum"),
        (Field::Line(1), "number"),
        (Field::DurationMs(1), "number"),
        (Field::Count(1), "number"),
        (Field::Status(200), "number"),
        (Field::Managed(true), "bool"),
        (Field::Js(true), "bool"),
        (Field::Ok(true), "bool"),
    ];
    for (field, kind) in samples {
        assert_eq!(payload_kind(&field), kind, "{field}");
    }
}

// R1.2 + R1.5: each variant shows as `key=value` with the key of the table.
#[test]
fn r1_2_field_display_is_key_equals_value() {
    let table = [
        (Field::Router(RouterState::NotI2p), "router=not-i2p"),
        (
            Field::Refuse(RefuseReason::LengthRequired),
            "refuse=length-required",
        ),
        (
            Field::Error(ErrorKind::from(io::ErrorKind::NotFound)),
            "error=not-found",
        ),
        (Field::Tab(TabKind::Internal), "tab=internal"),
        (Field::Store(StoreKind::Bookmarks), "store=bookmarks"),
        (Field::Op(OpKind::HardReload), "op=hard-reload"),
        (
            Field::Thread(ThreadName::RouterWatch),
            "thread=router-watch",
        ),
        (
            Field::Source(SourceFile::from_path("src/lib.rs")),
            "file=lib.rs",
        ),
        (Field::Line(42), "line=42"),
        (Field::DurationMs(1500), "ms=1500"),
        (Field::Count(7), "count=7"),
        (Field::Status(404), "status=404"),
        (Field::Managed(true), "managed=true"),
        (Field::Js(false), "js=false"),
        (Field::Ok(true), "ok=true"),
    ];
    for (field, expected) in table {
        assert_eq!(field.to_string(), expected);
    }
}

// R1.2 + R1.5: the values of the enums in the table.
#[test]
fn r1_2_enum_values_match_the_table() {
    let router = [
        (RouterState::Verifying, "verifying"),
        (RouterState::Ok, "ok"),
        (RouterState::Down, "down"),
        (RouterState::NotI2p, "not-i2p"),
        (RouterState::Paused, "paused"),
    ];
    for (value, name) in router {
        assert_eq!(value.as_str(), name);
    }
    let refuse = [
        (RefuseReason::NotI2p, "not-i2p"),
        (RefuseReason::BadRequest, "bad-request"),
        (RefuseReason::LengthRequired, "length-required"),
        (RefuseReason::Upstream, "upstream"),
        (RefuseReason::Busy, "busy"),
    ];
    for (value, name) in refuse {
        assert_eq!(value.as_str(), name);
    }
    assert_eq!(TabKind::Internal.as_str(), "internal");
    assert_eq!(TabKind::Web.as_str(), "web");
    let stores = [
        (StoreKind::Bookmarks, "bookmarks"),
        (StoreKind::History, "history"),
        (StoreKind::Settings, "settings"),
        (StoreKind::Sites, "sites"),
    ];
    for (value, name) in stores {
        assert_eq!(value.as_str(), name);
    }
}

// R1.2 + R1.5: the values of `OpKind` and `ThreadName`.
#[test]
fn r1_2_op_and_thread_values_match_the_table() {
    let ops = [
        (OpKind::Reload, "reload"),
        (OpKind::HardReload, "hard-reload"),
        (OpKind::Zoom, "zoom"),
        (OpKind::Find, "find"),
        (OpKind::FindClear, "find-clear"),
        (OpKind::Back, "back"),
        (OpKind::Forward, "forward"),
        (OpKind::Stop, "stop"),
        (OpKind::Harden, "harden"),
        (OpKind::Hover, "hover"),
        (OpKind::FirstLoad, "first-load"),
        (OpKind::EngineFilter, "engine-filter"),
        (OpKind::MainThread, "main-thread"),
        (OpKind::Spawn, "spawn"),
        (OpKind::Reveal, "reveal"),
        (OpKind::OpenUrl, "open-url"),
        (OpKind::WriteReport, "write-report"),
    ];
    for (value, name) in ops {
        assert_eq!(value.as_str(), name);
    }
    let threads = [
        (ThreadName::Main, "main"),
        (ThreadName::RouterWatch, "router-watch"),
        (ThreadName::RouterCheck, "router-check"),
        (ThreadName::Gatekeeper, "gatekeeper"),
        (ThreadName::Other, "other"),
    ];
    for (value, name) in threads {
        assert_eq!(value.as_str(), name);
    }
}

// R1.3: the name and the level of every code.
#[test]
fn r1_3_codes_have_kebab_names_and_levels() {
    let table = [
        (Code::Startup, "startup", "INFO"),
        (Code::Shutdown, "shutdown", "INFO"),
        (Code::StartFailed, "start-failed", "ERROR"),
        (Code::Panic, "panic", "ERROR"),
        (Code::VerifyPassed, "verify-passed", "INFO"),
        (Code::VerifyFailed, "verify-failed", "WARN"),
        (Code::RouterUp, "router-up", "INFO"),
        (Code::RouterDown, "router-down", "WARN"),
        (Code::GatekeeperRefused, "gatekeeper-refused", "INFO"),
        (
            Code::GatekeeperStartFailed,
            "gatekeeper-start-failed",
            "ERROR",
        ),
        (Code::PageLoadFailed, "page-load-failed", "WARN"),
        (Code::WebviewCreateFailed, "webview-create-failed", "ERROR"),
        (Code::EngineCallFailed, "engine-call-failed", "WARN"),
        (Code::FindFailed, "find-failed", "WARN"),
        (Code::ZoomFailed, "zoom-failed", "WARN"),
        (Code::StoreCorrupt, "store-corrupt", "WARN"),
        (Code::ThreadFailed, "thread-failed", "ERROR"),
        (Code::ReportOpened, "report-opened", "INFO"),
        (Code::ReportFailed, "report-failed", "WARN"),
    ];
    for (code, name, level) in table {
        assert_eq!(code.as_str(), name);
        assert_eq!(code.level().as_str(), level, "level of {name}");
    }
}

// R1.3: the three level names.
#[test]
fn r1_3_level_names() {
    assert_eq!(Level::Info.as_str(), "INFO");
    assert_eq!(Level::Warn.as_str(), "WARN");
    assert_eq!(Level::Error.as_str(), "ERROR");
}

// R1.4: an `io::ErrorKind` maps to the kebab-case kind of the table.
#[test]
fn r1_4_io_error_kinds_map_by_kind() {
    let table = [
        (io::ErrorKind::NotFound, "not-found"),
        (io::ErrorKind::PermissionDenied, "permission-denied"),
        (io::ErrorKind::ConnectionRefused, "connection-refused"),
        (io::ErrorKind::ConnectionReset, "connection-reset"),
        (io::ErrorKind::ConnectionAborted, "connection-aborted"),
        (io::ErrorKind::BrokenPipe, "broken-pipe"),
        (io::ErrorKind::TimedOut, "timed-out"),
        (io::ErrorKind::AddrInUse, "addr-in-use"),
        (io::ErrorKind::AddrNotAvailable, "addr-not-available"),
        (io::ErrorKind::InvalidInput, "invalid-input"),
        (io::ErrorKind::InvalidData, "invalid-data"),
        (io::ErrorKind::UnexpectedEof, "unexpected-eof"),
        (io::ErrorKind::WriteZero, "write-zero"),
        (io::ErrorKind::Interrupted, "interrupted"),
        (io::ErrorKind::Unsupported, "unsupported"),
        (io::ErrorKind::OutOfMemory, "out-of-memory"),
    ];
    for (kind, name) in table {
        assert_eq!(ErrorKind::from(kind).as_str(), name);
    }
}

// R1.4: an `io::ErrorKind` with no row is `other`.
#[test]
fn r1_4_unlisted_io_kind_is_other() {
    for kind in [
        io::ErrorKind::AlreadyExists,
        io::ErrorKind::Other,
        io::ErrorKind::WouldBlock,
    ] {
        assert_eq!(ErrorKind::from(kind).as_str(), "other", "{kind:?}");
    }
}

// R1.4: `From<&io::Error>` maps by the kind and never by the message.
#[test]
fn r1_4_io_error_maps_by_kind_not_message() {
    let err = io::Error::new(io::ErrorKind::ConnectionRefused, "dial forum.i2p:80 failed");
    let kind = ErrorKind::from(&err);
    assert_eq!(kind.as_str(), "connection-refused");
}

// R1.4: a JSON error is `parse`.
#[test]
fn r1_4_json_error_is_parse() {
    let err = serde_json::from_str::<u8>("not json").unwrap_err();
    assert_eq!(ErrorKind::from(&err).as_str(), "parse");
}

// R1.4: a tauri error maps by its I/O kind, and anything else is `webview`.
#[test]
fn r1_4_tauri_error_maps_io_by_kind_else_webview() {
    let io_err = tauri::Error::Io(io::Error::from(io::ErrorKind::PermissionDenied));
    assert_eq!(ErrorKind::from(&io_err).as_str(), "permission-denied");
    let other = tauri::Error::WindowNotFound;
    assert_eq!(ErrorKind::from(&other).as_str(), "webview");
}

// R1.4: the `platform` kind exists with its table name.
#[test]
fn r1_4_platform_kind_name() {
    assert_eq!(ErrorKind::Platform.as_str(), "platform");
}

// R1.6: `SourceFile` keeps the part after the last separator and only safe characters.
#[test]
fn r1_6_source_file_keeps_file_name_only() {
    let table = [
        ("src/shell/report.rs", "report.rs"),
        ("C:\\proj\\src\\main.rs", "main.rs"),
        ("a/b\\c.rs", "c.rs"),
        ("/Users/someone/work/x.rs", "x.rs"),
        ("weird name$%.rs", "weirdname.rs"),
    ];
    for (path, expected) in table {
        assert_eq!(SourceFile::from_path(path).as_str(), expected, "{path}");
    }
}

// R1.6: at most 64 characters.
#[test]
fn r1_6_source_file_is_at_most_64_characters() {
    let long = format!("src/{}.rs", "a".repeat(100));
    let file = SourceFile::from_path(&long);
    assert_eq!(file.as_str().chars().count(), 64);
}

// R1.7: the four known thread names map to their variant; anything else is `other`.
#[test]
fn r1_7_thread_names_map_known_names_only() {
    for known in ["main", "router-watch", "router-check", "gatekeeper"] {
        assert_eq!(ThreadName::of(Some(known)).as_str(), known);
    }
    for unknown in [
        Some("tokio-runtime-worker"),
        Some(""),
        Some("alice-laptop"),
        None,
    ] {
        assert_eq!(ThreadName::of(unknown).as_str(), "other", "{unknown:?}");
    }
}

// R1.8: the line is `<utc> <LEVEL> <code>` and ` key=value` per field, in order.
#[test]
fn r1_8_format_line_matches_the_example() {
    let record = Record {
        at: 1_791_028_800,
        code: Code::GatekeeperRefused,
        fields: vec![Field::Refuse(RefuseReason::NotI2p)],
    };
    assert_eq!(
        diag::format_line(&record),
        "2026-10-03T12:00:00Z INFO gatekeeper-refused refuse=not-i2p"
    );
}

// R1.8: no fields gives no trailing text; several fields keep their order.
#[test]
fn r1_8_format_line_without_and_with_several_fields() {
    let bare = Record {
        at: 0,
        code: Code::Startup,
        fields: vec![],
    };
    assert_eq!(
        diag::format_line(&bare),
        "1970-01-01T00:00:00Z INFO startup"
    );
    let many = Record {
        at: 0,
        code: Code::PageLoadFailed,
        fields: vec![Field::Status(502), Field::DurationMs(15), Field::Ok(false)],
    };
    assert_eq!(
        diag::format_line(&many),
        "1970-01-01T00:00:00Z WARN page-load-failed status=502 ms=15 ok=false"
    );
}

// R1.9: `utc` formats Unix seconds, also on leap days and century edges.
#[test]
fn r1_9_utc_formats_unix_seconds() {
    let table = [
        (0, "1970-01-01T00:00:00Z"),
        (946_684_799, "1999-12-31T23:59:59Z"),
        (1_709_164_800, "2024-02-29T00:00:00Z"),
        (1_791_028_800, "2026-10-03T12:00:00Z"),
        (4_107_542_400, "2100-03-01T00:00:00Z"),
    ];
    for (secs, expected) in table {
        assert_eq!(diag::utc(secs), expected);
    }
}

fn any_field() -> impl Strategy<Value = Field> {
    prop_oneof![
        any::<u32>().prop_map(Field::Line),
        any::<u64>().prop_map(Field::DurationMs),
        any::<u64>().prop_map(Field::Count),
        any::<u16>().prop_map(Field::Status),
        any::<bool>().prop_map(Field::Managed),
        any::<bool>().prop_map(Field::Js),
        any::<bool>().prop_map(Field::Ok),
        any::<String>().prop_map(|p| Field::Source(SourceFile::from_path(&p))),
        proptest::option::of("\\PC{0,20}")
            .prop_map(|n| Field::Thread(ThreadName::of(n.as_deref()))),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    // R1.6: whatever the path, the file part has only safe characters, no separator, 64 at most.
    #[test]
    fn r1_6_source_file_is_always_safe(path in "\\PC{0,200}") {
        let file = SourceFile::from_path(&path);
        let text = file.as_str();
        prop_assert!(text.chars().count() <= 64);
        prop_assert!(text.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c)));
    }

    // R1: a log line is one line and has one token per field, whatever the values.
    #[test]
    fn r1_8_format_line_is_one_line_with_one_token_per_field(
        at in 0u64..4_000_000_000,
        fields in proptest::collection::vec(any_field(), 0..6),
    ) {
        let count = fields.len();
        let record = Record { at, code: Code::PageLoadFailed, fields };
        let line = diag::format_line(&record);
        prop_assert!(!line.contains('\n'));
        prop_assert_eq!(line.split(' ').count(), 3 + count);
    }

    // R1.9: the time text has a fixed shape for any second.
    #[test]
    fn r1_9_utc_has_a_fixed_shape(secs in 0u64..4_000_000_000) {
        let text = diag::utc(secs);
        prop_assert_eq!(text.len(), 20);
        prop_assert!(text.ends_with('Z'));
        prop_assert_eq!(text.as_bytes()[10], b'T');
    }
}
