// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The report commands (`docs/wiki/diagnostics-and-bug-reports.md`, R7). Only the
//! `internal` webview may call them (`capabilities/report.json`).
//!
//! `report_open` is the one clearnet action of eepview, and it runs only after a click: it
//! opens a fixed GitHub URL prefix in the user's own browser. eepview itself sends nothing.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

use super::commands::{blocking, read};
use crate::core::Core;
use crate::diag::report::{self, PREVIEW_EVENTS, Report, ReportKind};
use crate::diag::sysinfo::{self, RouterKind, SystemInfo, UptimeBucket};
use crate::diag::{self, Code, ErrorKind, Field, OpKind, RouterState};

/// The result of `report_open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Opened {
    /// The report file name, no folder.
    pub file: String,
    /// The issue URL holds fewer log lines than the file.
    pub trimmed: bool,
}

/// The short git commit of this build.
const COMMIT: &str = match option_env!("EEPVIEW_COMMIT") {
    Some(commit) => commit,
    None => "unknown",
};

/// Opens URLs and shows files. The app uses the system; tests use a fake.
pub trait Launcher {
    /// Opens `url` in the default browser.
    ///
    /// # Errors
    ///
    /// Fails when the system cannot open it.
    fn open(&self, url: &str) -> Result<(), ErrorKind>;
    /// Shows `path` in the file manager.
    ///
    /// # Errors
    ///
    /// Fails when the system cannot show it.
    fn reveal(&self, path: &Path) -> Result<(), ErrorKind>;
}

/// The system browser and file manager, through `tauri-plugin-opener`.
struct System<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Launcher for System<R> {
    fn open(&self, url: &str) -> Result<(), ErrorKind> {
        self.0
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|_| ErrorKind::Platform)
    }

    fn reveal(&self, path: &Path) -> Result<(), ErrorKind> {
        self.0
            .opener()
            .reveal_item_in_dir(path)
            .map_err(|_| ErrorKind::Platform)
    }
}

/// What a report needs from the core.
#[derive(Debug, Clone, Copy)]
struct CoreFacts {
    state: RouterState,
    managed: bool,
    js_default: bool,
    tabs: usize,
}

/// The router state, managed flag, JS default and tab count of the core.
fn core_facts(core: &Core) -> CoreFacts {
    let router = core.router();
    let state = if router.paused {
        RouterState::Paused
    } else {
        RouterState::from_contract(router.state)
    };
    CoreFacts {
        state,
        managed: router.managed,
        js_default: core.settings().js_default,
        tabs: core.tabs().len(),
    }
}

/// The system facts of a report.
fn info(facts: CoreFacts) -> SystemInfo {
    let version = super::commands::helper_stats(super::env::router_helper()).version;
    SystemInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: COMMIT.to_owned(),
        os: sysinfo::os(),
        arch: std::env::consts::ARCH,
        engine: sysinfo::engine(),
        router_kind: version
            .as_deref()
            .map_or(RouterKind::Unknown, RouterKind::from_text),
        router_version: version.as_deref().and_then(sysinfo::clean_version),
        router_state: facts.state,
        managed: facts.managed,
        js_default: facts.js_default,
        tabs: facts.tabs,
        uptime: UptimeBucket::from_secs(diag::uptime_secs()),
    }
}

/// The report for the page's inputs, with the last [`PREVIEW_EVENTS`] events.
async fn build<R: Runtime>(
    app: AppHandle<R>,
    kind: String,
    description: String,
    include_log: bool,
) -> Result<Report, String> {
    let facts = read(app, core_facts).await?;
    blocking(move || Report {
        kind: ReportKind::from_param(&kind),
        description,
        info: info(facts),
        log: diag::report_lines(&diag::recent(PREVIEW_EVENTS)),
        include_log,
    })
    .await
}

/// `report_preview(kind, description, includeLog)`: exactly the text that goes out.
///
/// # Errors
///
/// Fails only when the worker thread fails.
#[tauri::command]
pub async fn report_preview<R: Runtime>(
    app: AppHandle<R>,
    kind: String,
    description: String,
    include_log: bool,
) -> Result<String, String> {
    let report = build(app, kind, description, include_log).await?;
    blocking(move || report::text(&report)).await
}

/// `report_open(kind, description, includeLog)`: writes the report file to Downloads, shows
/// it, and opens the prefilled issue in the system browser.
///
/// # Errors
///
/// Fails when there is no Downloads folder, or the file cannot be written or opened.
#[tauri::command]
pub async fn report_open<R: Runtime>(
    app: AppHandle<R>,
    kind: String,
    description: String,
    include_log: bool,
) -> Result<Opened, String> {
    let downloads = app
        .path()
        .download_dir()
        .map_err(|_| failed(OpKind::WriteReport, ErrorKind::NotFound))?;
    let report = build(app.clone(), kind, description, include_log).await?;
    blocking(move || send(&System(app), &downloads, &report, diag::now_secs())).await?
}

/// Records `ReportFailed` and gives the page its error text.
fn failed(op: OpKind, kind: ErrorKind) -> String {
    diag::event(Code::ReportFailed, &[Field::Op(op), Field::Error(kind)]);
    format!("The report could not be sent ({}).", kind.as_str())
}

/// Writes the report into `downloads`, shows it, and opens the issue URL.
///
/// # Errors
///
/// Fails when the file cannot be written, the URL or the file is outside its allowed place,
/// or the system cannot open them.
pub fn send(
    launcher: &impl Launcher,
    downloads: &Path,
    report: &Report,
    now: u64,
) -> Result<Opened, String> {
    let file = report::file_name(now);
    let path = downloads.join(&file);
    std::fs::write(&path, report::text(report))
        .map_err(|e| failed(OpKind::WriteReport, ErrorKind::from(&e)))?;
    let (url, trimmed) = report::issue_url(report);
    if !report::is_issue_url(&url) {
        return Err(failed(OpKind::OpenUrl, ErrorKind::InvalidInput));
    }
    if !in_folder(&path, downloads) {
        return Err(failed(OpKind::Reveal, ErrorKind::InvalidInput));
    }
    launcher
        .reveal(&path)
        .map_err(|kind| failed(OpKind::Reveal, kind))?;
    launcher
        .open(&url)
        .map_err(|kind| failed(OpKind::OpenUrl, kind))?;
    diag::event(Code::ReportOpened, &[Field::Ok(trimmed)]);
    Ok(Opened { file, trimmed })
}

/// True when `path` sits directly in `folder`.
fn in_folder(path: &Path, folder: &Path) -> bool {
    path.parent() == Some(folder) && path.file_name().is_some()
}

/// `diag_crash_status()`: the last run crashed and the banner was not dismissed.
#[tauri::command]
#[must_use]
pub fn diag_crash_status() -> bool {
    diag::crashed_last_run()
}

/// `diag_crash_dismiss()`.
#[tauri::command]
pub fn diag_crash_dismiss() {
    diag::dismiss_crash();
}

/// `diag_logs_delete()`: empties the log and deletes its files.
///
/// # Errors
///
/// Fails when a log file cannot be removed.
#[tauri::command]
pub fn diag_logs_delete() -> Result<(), String> {
    diag::delete_logs().map_err(|e| {
        let kind = ErrorKind::from(&e);
        format!("The logs could not be deleted ({}).", kind.as_str())
    })
}

/// The log folder of the app.
#[must_use]
pub fn log_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_log_dir().ok()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::json;

    use super::*;
    use crate::shell::testing::{app, invoke};
    use crate::store::testdir;

    #[derive(Default)]
    struct Fake {
        opened: RefCell<Vec<String>>,
        shown: RefCell<Vec<PathBuf>>,
        fail: Option<ErrorKind>,
    }

    impl Launcher for Fake {
        fn open(&self, url: &str) -> Result<(), ErrorKind> {
            self.opened.borrow_mut().push(url.to_owned());
            self.fail.map_or(Ok(()), Err)
        }

        fn reveal(&self, path: &Path) -> Result<(), ErrorKind> {
            self.shown.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }

    fn sample(app: &tauri::App<crate::shell::testing::Mock>) -> Report {
        let build = build(
            app.handle().clone(),
            "crash".into(),
            "it broke at http://a.i2p/x".into(),
            true,
        );
        tauri::async_runtime::block_on(build).unwrap()
    }

    #[test]
    fn send_writes_shows_and_opens() {
        let app = app();
        let dir = testdir::fresh("report-send");
        let fake = Fake::default();
        let opened = send(&fake, &dir, &sample(&app), 1_791_028_800).unwrap();
        assert_eq!(opened.file, "eepview-report-2026-10-03T12-00-00Z.txt");
        let text = std::fs::read_to_string(dir.join(&opened.file)).unwrap();
        assert!(!text.contains("a.i2p") && text.contains("[removed]"));
        assert!(report::is_issue_url(&fake.opened.borrow()[0]));
        assert_eq!(fake.shown.borrow()[0], dir.join(&opened.file));
    }

    #[test]
    fn send_fails_closed() {
        let app = app();
        let dir = testdir::fresh("report-fail");
        let fake = Fake {
            fail: Some(ErrorKind::Platform),
            ..Fake::default()
        };
        assert!(send(&fake, &dir, &sample(&app), 0).is_err());
        let missing = dir.join("missing");
        assert!(send(&Fake::default(), &missing, &sample(&app), 0).is_err());
        assert!(!in_folder(&dir, &missing));
    }

    #[test]
    fn commands_answer() {
        let app = app();
        let args = json!({ "kind": "blocked", "description": "", "includeLog": false });
        let text = invoke(&app, "report_preview", args).unwrap();
        assert!(text.as_str().unwrap().contains("(not given)"));
        assert_eq!(invoke(&app, "diag_crash_status", json!({})).unwrap(), false);
        invoke(&app, "diag_crash_dismiss", json!({})).unwrap();
        invoke(&app, "diag_logs_delete", json!({})).unwrap();
        assert!(log_dir(app.handle()).is_some());
        assert!(!failed(OpKind::Reveal, ErrorKind::Other).is_empty());
    }
}
