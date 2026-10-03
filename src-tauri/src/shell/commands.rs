// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The IPC commands of `docs/ipc.md`. Only the `toolbar` and `internal` webviews may call
//! them (capabilities/). Each one runs the core and queues its effects.

use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::apply::apply;
use super::state::{lock, now_ms, shared};
use crate::core::Core;
use crate::core::find::Zoom;
use crate::net::stats::RouterStats;
use crate::session::Step;
use crate::store::bookmarks::export_file_name;
use crate::store::settings::Settings;
use crate::tabs::Place;
use crate::types::{
    Bookmark, ControlResult, HistoryEntry, HistoryQuery, NavResult, NewBookmark, RouterStatus,
    Suggestion, TabInfo,
};

type Res<T> = Result<T, String>;

/// Runs `f` on the core and queues the effects it returns with its value.
fn run<T>(app: AppHandle, f: impl FnOnce(&mut Core) -> (T, Vec<crate::core::Effect>)) -> T {
    let (value, fx) = f(&mut lock(&shared(&app).core));
    queue(app, fx);
    value
}

/// Queues `fx` on the main thread (never applied while the core lock is held).
fn queue(app: AppHandle, fx: Vec<crate::core::Effect>) {
    if fx.is_empty() {
        return;
    }
    let runner = app.clone();
    if let Err(e) = runner.run_on_main_thread(move || apply(&app, fx)) {
        super::log::error("main thread", &e.to_string());
    }
}

/// Runs `f` on the core for effects only.
fn act(app: AppHandle, f: impl FnOnce(&mut Core) -> Vec<crate::core::Effect>) {
    run(app, |core| ((), f(core)));
}

/// Reads from the core.
fn read<T>(app: AppHandle, f: impl FnOnce(&Core) -> T) -> T {
    run(app, |core| (f(core), Vec::new()))
}

/// `tab_new(url?)`.
///
/// # Errors
///
/// Never in practice: the new tab always exists.
#[tauri::command]
pub fn tab_new(app: AppHandle, url: Option<String>) -> Res<TabInfo> {
    run(app, move |c| c.tab_new(url.as_deref(), Place::End)).ok_or_else(|| "no tab".into())
}

/// `tab_close(id)`.
#[tauri::command]
pub fn tab_close(app: AppHandle, id: u32) {
    act(app, move |c| c.tab_close(id));
}

/// `tab_select(id)`.
#[tauri::command]
pub fn tab_select(app: AppHandle, id: u32) {
    act(app, move |c| c.tab_select(id));
}

/// `tab_move(id, index)`.
#[tauri::command]
pub fn tab_move(app: AppHandle, id: u32, index: usize) {
    act(app, move |c| c.tab_move(id, index));
}

/// `tab_list()`.
#[tauri::command]
#[must_use]
pub fn tab_list(app: AppHandle) -> Vec<TabInfo> {
    read(app, Core::tab_infos)
}

/// `navigate(input)`.
#[tauri::command]
#[must_use]
pub fn navigate(app: AppHandle, input: String) -> NavResult {
    run(app, move |c| c.navigate(&input))
}

/// `go_back()`.
#[tauri::command]
pub fn go_back(app: AppHandle) {
    act(app, move |c| c.step(Step::Back));
}

/// `go_forward()`.
#[tauri::command]
pub fn go_forward(app: AppHandle) {
    act(app, move |c| c.step(Step::Forward));
}

/// `reload(hard?)`.
#[tauri::command]
pub fn reload(app: AppHandle, hard: Option<bool>) {
    act(app, move |c| c.reload(hard.unwrap_or(false)));
}

/// `stop()`.
#[tauri::command]
pub fn stop(app: AppHandle) {
    act(app, Core::stop);
}

/// `home()`.
#[tauri::command]
pub fn home(app: AppHandle) {
    act(app, Core::home);
}

/// `find(query, forward, matchCase)`.
#[tauri::command]
pub fn find(app: AppHandle, query: String, forward: bool, match_case: bool) {
    act(app, move |c| c.find(&query, forward, match_case));
}

/// `find_close()`.
#[tauri::command]
pub fn find_close(app: AppHandle) {
    act(app, Core::find_close);
}

/// `zoom_in()`.
#[tauri::command]
pub fn zoom_in(app: AppHandle) {
    act(app, move |c| c.zoom(Zoom::In));
}

/// `zoom_out()`.
#[tauri::command]
pub fn zoom_out(app: AppHandle) {
    act(app, move |c| c.zoom(Zoom::Out));
}

/// `zoom_reset()`.
#[tauri::command]
pub fn zoom_reset(app: AppHandle) {
    act(app, move |c| c.zoom(Zoom::Reset));
}

/// `site_js_set(host, on)`.
#[tauri::command]
pub fn site_js_set(app: AppHandle, host: String, on: bool) {
    act(app, move |c| c.site_js_set(&host, on));
}

/// `bookmarks_list()`.
#[tauri::command]
#[must_use]
pub fn bookmarks_list(app: AppHandle) -> Vec<Bookmark> {
    read(app, Core::bookmarks_list)
}

/// `bookmark_add({bookmark})`.
///
/// # Errors
///
/// Fails for a URL that is not an I2P site or an internal page.
#[tauri::command]
pub fn bookmark_add(app: AppHandle, bookmark: NewBookmark) -> Res<Bookmark> {
    run(app, move |c| match c.bookmark_add(&bookmark, now_ms()) {
        Ok((b, fx)) => (Ok(b), fx),
        Err(e) => (Err(e), Vec::new()),
    })
}

/// `bookmark_update({bookmark})`.
#[tauri::command]
pub fn bookmark_update(app: AppHandle, bookmark: Bookmark) {
    act(app, move |c| c.bookmark_update(&bookmark));
}

/// `bookmark_remove(id)`.
#[tauri::command]
pub fn bookmark_remove(app: AppHandle, id: String) {
    act(app, move |c| c.bookmark_remove(&id));
}

/// `bookmark_find(url)`.
#[tauri::command]
#[must_use]
pub fn bookmark_find(app: AppHandle, url: String) -> Option<Bookmark> {
    read(app, move |c| c.bookmark_find(&url))
}

/// `bookmarks_export()`.
#[tauri::command]
#[must_use]
pub fn bookmarks_export(app: AppHandle) -> String {
    read(app, Core::bookmarks_export)
}

/// `bookmarks_export_file()`: writes the export to the Downloads folder, returns the path.
///
/// # Errors
///
/// Fails when there is no Downloads folder or the file cannot be written.
#[tauri::command]
pub fn bookmarks_export_file(app: AppHandle) -> Res<String> {
    let dir = app.path().download_dir().map_err(|e| e.to_string())?;
    let path = dir.join(export_file_name(now_ms()));
    let text = read(app, Core::bookmarks_export);
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// `bookmarks_import(json)`.
///
/// # Errors
///
/// Fails when the text is not bookmark JSON.
#[tauri::command]
pub fn bookmarks_import(app: AppHandle, json: String) -> Res<usize> {
    run(app, move |c| match c.bookmarks_import(&json, now_ms()) {
        Ok((n, fx)) => (Ok(n), fx),
        Err(e) => (Err(e), Vec::new()),
    })
}

/// `history_query({query})`.
#[tauri::command]
#[must_use]
pub fn history_query(app: AppHandle, query: Option<HistoryQuery>) -> Vec<HistoryEntry> {
    read(app, move |c| c.history_query(&query.unwrap_or_default()))
}

/// `history_remove(id)`.
#[tauri::command]
pub fn history_remove(app: AppHandle, id: String) {
    act(app, move |c| c.history_remove(&id));
}

/// `history_clear(range)`.
///
/// # Errors
///
/// Fails for an unknown range.
#[tauri::command]
pub fn history_clear(app: AppHandle, range: String) -> Res<()> {
    run(app, move |c| match c.history_clear(&range, now_ms()) {
        Ok(fx) => (Ok(()), fx),
        Err(e) => (Err(e), Vec::new()),
    })
}

/// `suggest(input)`.
#[tauri::command]
#[must_use]
pub fn suggest(app: AppHandle, input: String) -> Vec<Suggestion> {
    read(app, move |c| c.suggest(&input, now_ms()))
}

/// `settings_get()`.
#[tauri::command]
#[must_use]
pub fn settings_get(app: AppHandle) -> Settings {
    read(app, move |c| c.settings().clone())
}

/// `settings_set({patch})`.
///
/// # Errors
///
/// Fails for a bad patch.
#[tauri::command]
pub fn settings_set(app: AppHandle, patch: Value) -> Res<Settings> {
    run(app, move |c| match c.settings_set(&patch) {
        Ok((s, fx)) => (Ok(s), fx),
        Err(e) => (Err(e), Vec::new()),
    })
}

/// `router_status()`.
#[tauri::command]
#[must_use]
pub fn router_status(app: AppHandle) -> RouterStatus {
    read(app, move |c| c.router().clone())
}

/// `router_stats()`: from the router helper named by `EEPVIEW_ROUTER_STATUS` and
/// `EEPVIEW_ROUTER_STATUS_TOKEN`; all fields `null` without one. `history` holds the
/// bandwidth of the last 10 minutes.
#[tauri::command]
#[must_use]
pub fn router_stats(app: AppHandle) -> RouterStats {
    let mut stats = super::env::router_helper()
        .map_or_else(RouterStats::default, |(addr, token)| {
            crate::net::stats::fetch(addr, &token)
        });
    stats.history = read(app, Core::stats_history);
    stats
}

/// `connection_pause()`: closes the gatekeeper and every tab webview until resume.
#[tauri::command]
pub fn connection_pause(app: AppHandle) {
    let gate = app.clone();
    act(app, Core::pause);
    super::watch::close_gate(&gate);
}

/// `connection_resume()`: VERIFY again; the gatekeeper opens only when it passes.
#[tauri::command]
pub fn connection_resume(app: AppHandle) {
    let check = app.clone();
    act(app, Core::resume);
    super::watch::check_now(&check, super::env::proxy());
}

/// `router_control(action)`: `stop`, `start` or `restart`. eepview does not run the router
/// yet, so every action answers `{ ok: false, reason: "external" }`.
///
/// # Errors
///
/// Fails for an unknown action.
#[tauri::command]
pub fn router_control(action: &str) -> Res<ControlResult> {
    match action {
        "stop" | "start" | "restart" => Ok(ControlResult {
            ok: false,
            reason: Some("external"),
        }),
        other => Err(format!("unknown action: {other}")),
    }
}

/// `chrome_set_height(px)`: the toolbar grows over the content while a popup is open; 0 ends it.
#[tauri::command]
pub fn chrome_set_height(app: AppHandle, px: f64) {
    act(app, move |c| c.set_toolbar_request(px));
}

/// `platform()`: `macos`, `windows` or `linux`.
#[tauri::command]
#[must_use]
pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
}
