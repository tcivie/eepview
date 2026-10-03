// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the review additions of `docs/wiki/diagnostics-and-bug-reports.md`:
//! R2.6 (log folder without Tauri), R2.7 (unbuffered append), R6.2 (`report_lines`),
//! R6.9 (the issue form ids) and R11.0 (`report` is an internal page). Public API only.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command};
use std::sync::atomic::{AtomicUsize, Ordering};

use eepview_lib::diag::report::{Report, ReportKind, issue_url};
use eepview_lib::diag::sysinfo::{RouterKind, SystemInfo, UptimeBucket};
use eepview_lib::diag::{
    self, Code, Field, LogFiles, Record, RefuseReason, RouterState, default_log_dir,
};
use eepview_lib::nav;
use proptest::prelude::*;

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh folder name in the system temp folder, removed on drop. It does not exist yet.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        Self(std::env::temp_dir().join(format!("eepview-diag-review-{}-{n}", process::id())))
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.0.join(name)).unwrap_or_default()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap_or_default();
    }
}

// ---------------------------------------------------------------------------------------
// R2.6: the log folder works without Tauri.
// ---------------------------------------------------------------------------------------

const IDENTIFIER: &str = "io.github.tcivie.eepview";

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

// R2.6: on macOS the folder is `$HOME/Library/Logs/<identifier>`.
#[test]
fn r2_6_default_log_dir_on_macos() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let home = env_path("HOME").expect("HOME is set");
    assert_eq!(
        default_log_dir(IDENTIFIER),
        Some(home.join("Library").join("Logs").join(IDENTIFIER))
    );
}

// R2.6: on Linux the folder is `$XDG_DATA_HOME/<identifier>/logs`, else `$HOME/.local/share/...`.
#[test]
fn r2_6_default_log_dir_on_linux() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let base = env_path("XDG_DATA_HOME")
        .or_else(|| env_path("HOME").map(|h| h.join(".local").join("share")))
        .expect("HOME or XDG_DATA_HOME is set");
    assert_eq!(
        default_log_dir(IDENTIFIER),
        Some(base.join(IDENTIFIER).join("logs"))
    );
}

// R2.6: on Windows the folder is `%LOCALAPPDATA%\<identifier>\logs`.
#[test]
fn r2_6_default_log_dir_on_windows() {
    if !cfg!(windows) {
        return;
    }
    let base = env_path("LOCALAPPDATA").expect("LOCALAPPDATA is set");
    assert_eq!(
        default_log_dir(IDENTIFIER),
        Some(base.join(IDENTIFIER).join("logs"))
    );
}

// R2.6: the identifier is the folder name, so another identifier gives another folder.
#[test]
fn r2_6_default_log_dir_follows_the_identifier() {
    let one = default_log_dir("com.example.one").expect("a folder");
    let two = default_log_dir("com.example.two").expect("a folder");
    assert_ne!(one, two);
    assert!(one.to_string_lossy().contains("com.example.one"));
    assert!(one.is_absolute(), "{}", one.display());
}

// R2.6: it is the same folder as Tauri's `app_log_dir()` (checked on the mock runtime).
#[test]
fn r2_6_default_log_dir_equals_tauri_app_log_dir() {
    use tauri::Manager;
    let app = tauri::test::mock_app();
    let identifier = app.config().identifier.clone();
    let tauri_dir = app.path().app_log_dir().expect("app_log_dir");
    assert_eq!(default_log_dir(&identifier), Some(tauri_dir));
}

// ---------------------------------------------------------------------------------------
// R2.7: each append is one unbuffered write.
// ---------------------------------------------------------------------------------------

// R2.7: the line is on disk before `append` returns, while the `LogFiles` is still alive.
#[test]
fn r2_7_each_line_is_on_disk_when_append_returns() {
    let tmp = TempDir::new();
    let mut files = LogFiles::new(tmp.0.clone());
    let mut expected = String::new();
    for n in 0..50 {
        let line = format!("line-{n:04}");
        files.append(&line).expect("append");
        expected.push_str(&line);
        expected.push('\n');
        assert_eq!(tmp.read("eepview.log"), expected, "after line {n}");
    }
}

// R2.7: a long line is on disk in full, so no buffer of any size holds a part of it.
#[test]
fn r2_7_a_long_line_is_on_disk_in_full() {
    let tmp = TempDir::new();
    let mut files = LogFiles::new(tmp.0.clone());
    let line = "x".repeat(100_000);
    files.append(&line).expect("append");
    assert_eq!(tmp.read("eepview.log").len(), line.len() + 1);
}

// R2.7: the line is on disk even when the `LogFiles` is never dropped, so a drop cannot flush it.
#[test]
fn r2_7_a_line_does_not_wait_for_a_drop() {
    let tmp = TempDir::new();
    let mut files = LogFiles::new(tmp.0.clone());
    files.append("never-dropped").expect("append");
    assert_eq!(tmp.read("eepview.log"), "never-dropped\n");
    std::mem::forget(files);
}

const CHILD_DIR: &str = "EEPVIEW_DIAG_R27_DIR";

// R2.7: the child half of the abort test. It does nothing unless the parent started it.
#[test]
fn r2_7_child_appends_then_aborts() {
    let Some(dir) = env_path(CHILD_DIR) else {
        return;
    };
    let mut files = LogFiles::new(dir);
    files.append("written-before-abort").expect("append");
    process::abort();
}

// R2.7: a line is on disk when the process ends with an abort (`panic = "abort"`).
#[test]
fn r2_7_a_line_survives_an_abort() {
    let tmp = TempDir::new();
    let exe = std::env::current_exe().expect("test binary");
    let status = Command::new(exe)
        .args([
            "--exact",
            "r2_7_child_appends_then_aborts",
            "--test-threads=1",
        ])
        .env(CHILD_DIR, &tmp.0)
        .status()
        .expect("run the child");
    assert!(!status.success(), "the child aborts");
    assert_eq!(tmp.read("eepview.log"), "written-before-abort\n");
}

// ---------------------------------------------------------------------------------------
// R6.2 and R13.1: `report_lines` holds no time at all, only the order of the events.
// ---------------------------------------------------------------------------------------

const T0: u64 = 1_791_028_800; // 2026-10-03T12:00:00Z

fn record(at: u64, code: Code, fields: Vec<Field>) -> Record {
    Record { at, code, fields }
}

// R13.1: `<LEVEL> <code>` and ` key=value` per field, in record order.
#[test]
fn r13_1_report_lines_are_level_code_and_fields() {
    let records = [
        record(T0, Code::Startup, vec![]),
        record(
            T0 + 120,
            Code::GatekeeperStartFailed,
            vec![Field::Refuse(RefuseReason::NotI2p), Field::Count(3)],
        ),
        record(
            T0 + 3600,
            Code::RouterDown,
            vec![Field::Router(RouterState::Down)],
        ),
    ];
    assert_eq!(
        diag::report_lines(&records),
        [
            "INFO startup",
            "ERROR gatekeeper-start-failed refuse=not-i2p count=3",
            "WARN router-down router=down",
        ]
    );
}

// R13.1: no records, no lines. One line per record, in record order.
#[test]
fn r13_1_report_lines_one_per_record_in_order() {
    assert!(diag::report_lines(&[]).is_empty());
    let records: Vec<Record> = (0..7)
        .map(|i| record(T0 + i, Code::Startup, vec![Field::Count(i)]))
        .collect();
    let lines = diag::report_lines(&records);
    assert_eq!(lines.len(), 7);
    for (i, line) in lines.iter().enumerate() {
        assert_eq!(line, &format!("INFO startup count={i}"));
    }
}

// R13.1: the lines hold no time and no time offset: no UTC text, no Unix second, no `+Nm`.
#[test]
fn r13_1_report_lines_hold_no_time() {
    let records = [
        record(T0, Code::Startup, vec![]),
        record(T0 + 5400, Code::Shutdown, vec![]),
    ];
    for (line, record) in diag::report_lines(&records).iter().zip(&records) {
        assert!(!line.contains(&diag::utc(record.at)), "{line}");
        assert!(!line.contains("2026"), "{line}");
        assert!(!line.contains(&record.at.to_string()), "{line}");
        assert!(!line.contains(':'), "{line}");
        assert!(!line.starts_with('+'), "{line}");
        assert!(!line.contains("+0m") && !line.contains("+90m"), "{line}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    // R13.1: the lines do not depend on the times of the records at all.
    #[test]
    fn r13_1_report_lines_do_not_depend_on_time(
        start in 0u64..4_000_000_000,
        other in 0u64..4_000_000_000,
        gaps in proptest::collection::vec(0u64..20_000, 1..30),
    ) {
        let build = |base: u64| {
            let mut at = base;
            let mut records = vec![record(at, Code::Startup, vec![])];
            for gap in &gaps {
                at += gap;
                records.push(record(at, Code::RouterUp, vec![Field::Router(RouterState::Ok)]));
            }
            records
        };
        let one = diag::report_lines(&build(start));
        let two = diag::report_lines(&build(other));
        prop_assert_eq!(&one, &two);
        for (line, record) in one.iter().zip(build(start)) {
            prop_assert!(!line.contains(&diag::utc(record.at)), "{}", line);
            prop_assert!(!line.contains(&record.at.to_string()), "{}", line);
        }
    }
}

// ---------------------------------------------------------------------------------------
// R6.9: the issue URL uses the field ids of `.github/ISSUE_TEMPLATE/bug.yml`.
// ---------------------------------------------------------------------------------------

/// One field of the issue form: its type, id, `required` and `render`.
#[derive(Debug, Default)]
struct FormField {
    kind: String,
    id: String,
    required: bool,
    render: Option<String>,
}

/// Reads one line of the form into the field it belongs to.
fn read_line(field: &mut FormField, line: &str) {
    if let Some(id) = line.strip_prefix("id:") {
        id.trim().clone_into(&mut field.id);
    } else if let Some(required) = line.strip_prefix("required:") {
        field.required = required.trim() == "true";
    } else if let Some(render) = line.strip_prefix("render:") {
        field.render = Some(render.trim().to_owned());
    }
}

/// The fields of `bug.yml`, read from the file (a plain line scan: the form is flat).
/// An unreadable file gives no fields, so every test on the form fails.
fn form_fields() -> Vec<FormField> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.github/ISSUE_TEMPLATE/bug.yml");
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut fields: Vec<FormField> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(kind) = line.strip_prefix("- type:") {
            fields.push(FormField {
                kind: kind.trim().to_owned(),
                ..FormField::default()
            });
        } else if let Some(field) = fields.last_mut() {
            read_line(field, line);
        }
    }
    fields
}

/// The field with this id, or an empty field when the form has none.
fn field(id: &str) -> FormField {
    form_fields()
        .into_iter()
        .find(|f| f.id == id)
        .unwrap_or_default()
}

fn sample_report() -> Report {
    Report {
        kind: ReportKind::General,
        description: "the page did not load".to_owned(),
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
        log: vec!["INFO startup".to_owned()],
        include_log: true,
    }
}

/// The `key=value` pairs of the query of the issue URL, in order.
fn query_keys(url: &str) -> Vec<String> {
    let query = url.split_once('?').map_or("", |(_, q)| q);
    query
        .split('&')
        .filter_map(|pair| pair.split_once('=').map(|(k, _)| k.to_owned()))
        .collect()
}

fn query_value(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query
        .split('&')
        .find_map(|p| p.strip_prefix(&format!("{key}=")))
        .map(str::to_owned)
}

// R6.9: the form has these ids, with these types.
#[test]
fn r6_9_form_has_the_ids_with_their_types() {
    assert_eq!(field("version").kind, "input");
    assert_eq!(field("os").kind, "input");
    assert_eq!(field("router").kind, "dropdown");
    assert_eq!(field("what-happened").kind, "textarea");
    assert_eq!(field("steps").kind, "textarea");
    assert_eq!(field("diagnostics").kind, "textarea");
}

// R6.9: `what-happened` is required; `router`, `steps` and `diagnostics` are not.
#[test]
fn r6_9_form_requires_what_happened_only_among_the_long_fields() {
    assert!(field("what-happened").required);
    assert!(!field("router").required);
    assert!(!field("steps").required);
    assert!(!field("diagnostics").required);
}

// R6.9: `diagnostics` is rendered as text.
#[test]
fn r6_9_diagnostics_renders_as_text() {
    assert_eq!(field("diagnostics").render.as_deref(), Some("text"));
}

// R6.9: every field the URL fills is a field of the form.
#[test]
fn r6_9_every_url_parameter_is_a_form_id() {
    let ids: BTreeSet<String> = form_fields().into_iter().map(|f| f.id).collect();
    let (url, _) = issue_url(&sample_report());
    for key in query_keys(&url) {
        if ["template", "labels", "title"].contains(&key.as_str()) {
            continue;
        }
        assert!(
            ids.contains(&key),
            "the URL fills `{key}`, which is not an id in bug.yml: {ids:?}"
        );
    }
}

// R6.9: no required field of the form is left empty by the URL.
#[test]
fn r6_9_url_fills_every_required_field() {
    let (url, _) = issue_url(&sample_report());
    for form_field in form_fields().into_iter().filter(|f| f.required) {
        let value = query_value(&url, &form_field.id);
        assert!(
            value.is_some_and(|v| !v.is_empty()),
            "the URL leaves the required field `{}` empty",
            form_field.id
        );
    }
}

// R6.9: the URL fills `diagnostics`, `version`, `os` and `what-happened`.
#[test]
fn r6_9_url_fills_the_four_fields_of_r6_4() {
    let (url, _) = issue_url(&sample_report());
    for id in ["version", "os", "what-happened", "diagnostics"] {
        assert!(query_value(&url, id).is_some_and(|v| !v.is_empty()), "{id}");
    }
}

// R6.9: the URL names the template file that exists, and a label the template has.
#[test]
fn r6_9_url_names_the_template_and_label_of_the_form() {
    let (url, _) = issue_url(&sample_report());
    let template = query_value(&url, "template").expect("template parameter");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../.github/ISSUE_TEMPLATE")
        .join(&template);
    assert!(path.is_file(), "{} does not exist", path.display());
    let text = fs::read_to_string(path).expect("template");
    let label = query_value(&url, "labels").expect("labels parameter");
    assert!(text.contains(&label), "the form has no label `{label}`");
}

// ---------------------------------------------------------------------------------------
// R11.0: `report` is an internal page.
// ---------------------------------------------------------------------------------------

// R11.0: `report` is in `INTERNAL_PAGES`.
#[test]
fn r11_0_report_is_in_internal_pages() {
    assert!(nav::INTERNAL_PAGES.contains(&"report"));
}

// R11.0: `eepview://report` is a known internal page, with or without the kind.
#[test]
fn r11_0_eepview_report_is_a_known_internal_page() {
    for address in ["eepview://report", "eepview://report?kind=crash"] {
        let url = tauri::Url::parse(address).expect("a URL");
        assert_eq!(
            nav::internal_from_url(&url).as_deref(),
            Some(address),
            "{address}"
        );
    }
}

// R11.0: the bundled file of the page is `report.html`.
#[test]
fn r11_0_report_renders_from_report_html() {
    let file = nav::internal_file("eepview://report").expect("a bundled file");
    assert!(file.contains("report.html"), "{file}");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/ui/report.html");
    assert!(root.is_file(), "src/ui/report.html");
}
