// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R3.5, R7, R8.3, R8.5, R11.1, R11.3 and R11.4 of
//! `docs/wiki/diagnostics-and-bug-reports.md`, read from the UI sources: the report page,
//! the entry points and the settings button. They read files only; they do not run the page.
//! (The exports of `src/ui/lib/report-page.ts` are tested in `report-page.test.ts`.)

use std::fs;
use std::path::{Path, PathBuf};

/// The UI folder (`src/ui/`), next to the crate root.
fn ui() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/ui")
}

/// A UI file, or an empty text when it does not exist yet.
fn read(name: &str) -> String {
    fs::read_to_string(ui().join(name)).unwrap_or_default()
}

/// True for a `.ts` file that is not a `.test.ts` file.
fn is_source(path: &Path) -> bool {
    let is_ts = path.extension().is_some_and(|e| e == "ts");
    let is_test = path
        .file_stem()
        .is_some_and(|s| Path::new(s).extension().is_some_and(|e| e == "test"));
    is_ts && !is_test
}

/// Every `.ts` file under `dir` that is not a test, joined.
fn sources(dir: &Path) -> String {
    let mut out = String::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.push_str(&sources(&path));
        } else if is_source(&path) {
            out.push('\n');
            out.push_str(&fs::read_to_string(&path).unwrap_or_default());
        }
    }
    out
}

/// The report page and its script: the text the user sees may sit in either.
fn report_page() -> String {
    format!("{}\n{}", read("report.html"), read("report.ts"))
}

fn has_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| text.contains(n))
}

// R11.1: the page `eepview://report` is a bundled page with a script.
#[test]
fn r11_1_report_page_exists() {
    assert!(ui().join("report.html").is_file(), "src/ui/report.html");
    assert!(ui().join("report.ts").is_file(), "src/ui/report.ts");
}

// R11.1: a description box and a read-only preview.
#[test]
fn r11_1_has_a_description_box_and_a_read_only_preview() {
    let html = read("report.html");
    assert_eq!(html.matches("<textarea").count(), 2, "two text boxes");
    assert!(html.contains("readonly"), "the preview is read-only");
}

// R11.1: the checkbox "Include the diagnostics log", on by default.
#[test]
fn r11_1_has_the_include_log_checkbox_on_by_default() {
    let html = read("report.html");
    assert!(html.contains("Include the diagnostics log"));
    let checkbox = html
        .split("<input")
        .skip(1)
        .find(|tag| tag.contains("checkbox"));
    let tag = checkbox
        .and_then(|t| t.split('>').next())
        .unwrap_or_default();
    assert!(tag.contains("checked"), "the checkbox is on: {tag}");
}

// R11.1: the button.
#[test]
fn r11_1_has_the_open_a_github_issue_button() {
    assert!(read("report.html").contains("Open a GitHub issue"));
}

// R11.1: the placeholder, the GitHub note and the note after the click are on the page.
#[test]
fn r11_1_shows_placeholder_note_and_drag_note() {
    let page = report_page();
    assert!(has_any(
        &page,
        &["PLACEHOLDER", "Do not include site addresses."]
    ));
    assert!(has_any(
        &page,
        &["BROWSER_NOTE", "GitHub sees your IP address"]
    ));
    assert!(has_any(
        &page,
        &["DRAG_NOTE", "openedMessage", "Drag this file"]
    ));
}

// R7 + R11.1: the page previews with `report_preview` and sends with `report_open`, and both
// get the kind, the description and the checkbox.
#[test]
fn r7_page_asks_rust_for_the_preview_and_the_send() {
    let script = read("report.ts");
    for token in [
        "report_preview",
        "report_open",
        "includeLog",
        "description",
        "kind",
    ] {
        assert!(script.contains(token), "report.ts lacks {token}");
    }
    assert!(
        script.contains(".value"),
        "the preview box gets the text it is given"
    );
}

// R8.3 + R8.5: the page makes no network call and opens nothing itself.
#[test]
fn r8_3_report_page_has_no_way_out() {
    let page = report_page();
    for token in [
        "fetch(",
        "XMLHttpRequest",
        "sendBeacon",
        "WebSocket",
        "window.open",
        "plugin-opener",
        "openUrl",
        "revealItemInDir",
    ] {
        assert!(!page.contains(token), "the report page uses {token}");
    }
}

// R7: every report command is called from the UI.
#[test]
fn r7_ui_calls_every_report_command() {
    let all = sources(&ui());
    for command in [
        "report_preview",
        "report_open",
        "diag_crash_status",
        "diag_crash_dismiss",
        "diag_logs_delete",
    ] {
        assert!(all.contains(command), "{command} is not called from src/ui");
    }
}

// R11.3: the toolbar menu has "Report a problem", which opens `eepview://report`.
#[test]
fn r11_3_toolbar_menu_has_report_a_problem() {
    let toolbar = format!(
        "{}\n{}\n{}",
        read("toolbar.html"),
        read("toolbar.ts"),
        sources(&ui().join("toolbar"))
    );
    assert!(toolbar.contains("Report a problem"));
    assert!(toolbar.contains("eepview://report"));
}

// R3.5: the home page shows the crash banner with Report and Dismiss.
#[test]
fn r3_5_home_page_shows_the_crash_banner() {
    let home = format!("{}\n{}", read("home.html"), read("home.ts"));
    assert!(home.contains("diag_crash_status"));
    assert!(home.contains("diag_crash_dismiss"));
    assert!(has_any(
        &home,
        &["CRASH_TEXT", "eepview closed unexpectedly"]
    ));
    assert!(home.contains("Dismiss"));
    assert!(has_any(&home, &["kind=crash", "reportHref(\"crash\")"]));
}

// R11.3: the blocked and router-down pages link to `reportHref(kind)`, never with an address.
#[test]
fn r11_3_blocked_and_router_down_pages_link_with_the_kind_only() {
    for (page, kind) in [("blocked", "blocked"), ("router-down", "router-down")] {
        let text = format!(
            "{}\n{}",
            read(&format!("{page}.html")),
            read(&format!("{page}.ts"))
        );
        assert!(text.contains("Report this problem"), "{page}: link text");
        assert!(
            text.contains(&format!("reportHref(\"{kind}\")")),
            "{page}: reportHref"
        );
        assert!(
            !text.contains("report.html?"),
            "{page}: the link is built by reportHref only"
        );
    }
}

// R11.4: Settings > Privacy has the button `delete-logs`.
#[test]
fn r11_4_settings_has_the_delete_logs_button() {
    let html = read("settings.html");
    assert!(html.contains("id=\"delete-logs\""));
    assert!(html.contains("Delete diagnostics logs"));
}

// R11.4: the button calls `diag_logs_delete` and says "Diagnostics logs deleted."
#[test]
fn r11_4_delete_logs_button_calls_the_command_and_confirms() {
    let settings = format!("{}\n{}", read("settings.ts"), sources(&ui().join("lib")));
    assert!(settings.contains("delete-logs"));
    assert!(settings.contains("diag_logs_delete"));
    assert!(settings.contains("Diagnostics logs deleted."));
}
