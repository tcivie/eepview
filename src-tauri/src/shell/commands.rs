// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The IPC commands of `docs/wiki/ipc-contract.md`. Only the bundled webviews may call
//! them, each one only the commands its capability names (capabilities/). Each one runs the core and queues its effects.

use serde_json::Value;
use std::path::Path;

use tauri::{AppHandle, Manager, Runtime};

use super::apply::apply;
use super::popup::Anchor;
use super::state::{lock, now_ms, shared};
use crate::core::find::Zoom;
use crate::core::{Core, Effect};
use crate::net::console::ConsoleInfo;
use crate::net::loopback::LoopbackAddr;
use crate::net::stats::RouterStats;
use crate::popup::Kind;
use crate::session::Step;
use crate::store::bookmarks::export_file_name;
use crate::store::settings::Settings;
use crate::tabs::Place;
use crate::types::{
    Bookmark, ChromeInsets, ControlResult, HistoryEntry, HistoryQuery, NavResult, NewBookmark,
    RouterStatus, Suggestion, TabInfo,
};

type Res<T> = Result<T, String>;

/// Runs blocking work on a worker thread. Every command that takes the app is `async` and
/// does its work here: a plain `fn` command runs on the main thread, and core calls may write
/// a store file, wait on a lock or reach the router.
pub(super) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())
}

/// Runs `f` on the core and queues the effects it returns with its value.
async fn run<T: Send + 'static, R: Runtime>(
    app: AppHandle<R>,
    f: impl FnOnce(&mut Core) -> (T, Vec<Effect>) + Send + 'static,
) -> Res<T> {
    blocking(move || {
        let (value, fx) = f(&mut lock(&shared(&app).core));
        queue(app, fx);
        value
    })
    .await
}

/// Queues `fx` on the main thread (never applied while the core lock is held).
fn queue<R: Runtime>(app: AppHandle<R>, fx: Vec<Effect>) {
    if fx.is_empty() {
        return;
    }
    let runner = app.clone();
    if let Err(e) = runner.run_on_main_thread(move || apply(&app, fx)) {
        crate::diag::event(
            crate::diag::Code::ThreadFailed,
            &[
                crate::diag::Field::Op(crate::diag::OpKind::MainThread),
                crate::diag::Field::Error((&e).into()),
            ],
        );
    }
}

/// Runs `f` on the core for effects only.
async fn act<R: Runtime>(
    app: AppHandle<R>,
    f: impl FnOnce(&mut Core) -> Vec<Effect> + Send + 'static,
) -> Res<()> {
    run(app, |core| ((), f(core))).await
}

/// Reads from the core.
pub(super) async fn read<T: Send + 'static, R: Runtime>(
    app: AppHandle<R>,
    f: impl FnOnce(&Core) -> T + Send + 'static,
) -> Res<T> {
    run(app, |core| (f(core), Vec::new())).await
}

/// A core result as a command result: the effects of a failed call are none.
fn split<T>(result: Result<(T, Vec<Effect>), String>) -> (Res<T>, Vec<Effect>) {
    match result {
        Ok((value, fx)) => (Ok(value), fx),
        Err(e) => (Err(e), Vec::new()),
    }
}

/// `tab_new(url?)`.
///
/// # Errors
///
/// Never in practice: the new tab always exists.
#[tauri::command]
pub async fn tab_new<R: Runtime>(app: AppHandle<R>, url: Option<String>) -> Res<TabInfo> {
    run(app, move |c| c.tab_new(url.as_deref(), Place::End))
        .await?
        .ok_or_else(|| "no tab".into())
}

/// `tab_close(id)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn tab_close<R: Runtime>(app: AppHandle<R>, id: u32) -> Res<()> {
    act(app, move |c| c.tab_close(id)).await
}

/// `tab_select(id)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn tab_select<R: Runtime>(app: AppHandle<R>, id: u32) -> Res<()> {
    act(app, move |c| c.tab_select(id)).await
}

/// `tab_move(id, index)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn tab_move<R: Runtime>(app: AppHandle<R>, id: u32, index: usize) -> Res<()> {
    act(app, move |c| c.tab_move(id, index)).await
}

/// `tab_list()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn tab_list<R: Runtime>(app: AppHandle<R>) -> Res<Vec<TabInfo>> {
    read(app, Core::tab_infos).await
}

/// `navigate(input)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn navigate<R: Runtime>(app: AppHandle<R>, input: String) -> Res<NavResult> {
    run(app, move |c| c.navigate(&input)).await
}

/// `go_back()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn go_back<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    act(app, move |c| c.step(Step::Back)).await
}

/// `go_forward()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn go_forward<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    act(app, move |c| c.step(Step::Forward)).await
}

/// `reload(hard?)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn reload<R: Runtime>(app: AppHandle<R>, hard: Option<bool>) -> Res<()> {
    act(app, move |c| c.reload(hard.unwrap_or(false))).await
}

/// `stop()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn stop<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    act(app, Core::stop).await
}

/// `home()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn home<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    act(app, Core::home).await
}

/// `find(query, forward, matchCase)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn find<R: Runtime>(
    app: AppHandle<R>,
    query: String,
    forward: bool,
    match_case: bool,
) -> Res<()> {
    act(app, move |c| c.find(&query, forward, match_case)).await
}

/// `find_close()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn find_close<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    act(app, Core::find_close).await
}

/// `zoom_in()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn zoom_in<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    act(app, move |c| c.zoom(Zoom::In)).await
}

/// `zoom_out()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn zoom_out<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    act(app, move |c| c.zoom(Zoom::Out)).await
}

/// `zoom_reset()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn zoom_reset<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    act(app, move |c| c.zoom(Zoom::Reset)).await
}

/// `site_js_set(host, on)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn site_js_set<R: Runtime>(app: AppHandle<R>, host: String, on: bool) -> Res<()> {
    act(app, move |c| c.site_js_set(&host, on)).await
}

/// `bookmarks_list()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn bookmarks_list<R: Runtime>(app: AppHandle<R>) -> Res<Vec<Bookmark>> {
    read(app, Core::bookmarks_list).await
}

/// `bookmark_add({bookmark})`.
///
/// # Errors
///
/// Fails for a URL that is not an I2P site or an internal page.
#[tauri::command]
pub async fn bookmark_add<R: Runtime>(app: AppHandle<R>, bookmark: NewBookmark) -> Res<Bookmark> {
    run(app, move |c| split(c.bookmark_add(&bookmark, now_ms()))).await?
}

/// `bookmark_update({bookmark})`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn bookmark_update<R: Runtime>(app: AppHandle<R>, bookmark: Bookmark) -> Res<()> {
    act(app, move |c| c.bookmark_update(&bookmark)).await
}

/// `bookmark_remove(id)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn bookmark_remove<R: Runtime>(app: AppHandle<R>, id: String) -> Res<()> {
    act(app, move |c| c.bookmark_remove(&id)).await
}

/// `bookmark_find(url)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn bookmark_find<R: Runtime>(app: AppHandle<R>, url: String) -> Res<Option<Bookmark>> {
    read(app, move |c| c.bookmark_find(&url)).await
}

/// `bookmarks_export()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn bookmarks_export<R: Runtime>(app: AppHandle<R>) -> Res<String> {
    read(app, Core::bookmarks_export).await
}

/// `bookmarks_export_file()`: writes the export to the Downloads folder, returns the path.
///
/// # Errors
///
/// Fails when there is no Downloads folder or the file cannot be written.
#[tauri::command]
pub async fn bookmarks_export_file<R: Runtime>(app: AppHandle<R>) -> Res<String> {
    let dir = app.path().download_dir().map_err(|e| e.to_string())?;
    let text = read(app, Core::bookmarks_export).await?;
    blocking(move || write_export(&dir, &text, now_ms())).await?
}

/// Writes a bookmark export into `dir`, returns the path.
fn write_export(dir: &Path, text: &str, now: u64) -> Res<String> {
    let path = dir.join(export_file_name(now));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// `bookmarks_import(json)`.
///
/// # Errors
///
/// Fails when the text is not bookmark JSON.
#[tauri::command]
pub async fn bookmarks_import<R: Runtime>(app: AppHandle<R>, json: String) -> Res<usize> {
    run(app, move |c| split(c.bookmarks_import(&json, now_ms()))).await?
}

/// `history_query({query})`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn history_query<R: Runtime>(
    app: AppHandle<R>,
    query: Option<HistoryQuery>,
) -> Res<Vec<HistoryEntry>> {
    read(app, move |c| c.history_query(&query.unwrap_or_default())).await
}

/// `history_remove(id)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn history_remove<R: Runtime>(app: AppHandle<R>, id: String) -> Res<()> {
    act(app, move |c| c.history_remove(&id)).await
}

/// `history_clear(range)`.
///
/// # Errors
///
/// Fails for an unknown range.
#[tauri::command]
pub async fn history_clear<R: Runtime>(app: AppHandle<R>, range: String) -> Res<()> {
    run(app, move |c| {
        split(c.history_clear(&range, now_ms()).map(|fx| ((), fx)))
    })
    .await?
}

/// `suggest(input)`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn suggest<R: Runtime>(app: AppHandle<R>, input: String) -> Res<Vec<Suggestion>> {
    read(app, move |c| c.suggest(&input, now_ms())).await
}

/// `settings_get()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn settings_get<R: Runtime>(app: AppHandle<R>) -> Res<Settings> {
    read(app, move |c| c.settings().clone()).await
}

/// `settings_set({patch})`.
///
/// # Errors
///
/// Fails for a bad patch.
#[tauri::command]
pub async fn settings_set<R: Runtime>(app: AppHandle<R>, patch: Value) -> Res<Settings> {
    run(app, move |c| split(c.settings_set(&patch))).await?
}

/// `router_status()`.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn router_status<R: Runtime>(app: AppHandle<R>) -> Res<RouterStatus> {
    read(app, move |c| c.router().clone()).await
}

/// `router_stats()`: the figures of the last round of the stats sampler (`shell::sampler`),
/// all `null` before the first one. `history` holds the bandwidth of the last 10 minutes.
/// It sends no request.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn router_stats<R: Runtime>(app: AppHandle<R>) -> Res<RouterStats> {
    blocking(move || current_stats(&app)).await
}

/// The `router_stats()` answer: the latest figures of the sampler and the bandwidth of the
/// last 10 minutes. No request to the helper or the console, and no sample.
pub fn current_stats<R: Runtime>(app: &AppHandle<R>) -> RouterStats {
    lock(&shared(app).core).stats_answer(now_ms())
}

/// The router helper statistics, all `null` without a helper.
pub fn helper_stats(helper: Option<(LoopbackAddr, String)>) -> RouterStats {
    helper.map_or_else(RouterStats::default, |(addr, token)| {
        crate::net::stats::fetch(addr, &token)
    })
}

/// `connection_pause()`: closes the gatekeeper and every tab webview until resume.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn connection_pause<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    let gate = app.clone();
    act(app, Core::pause).await?;
    blocking(move || super::watch::close_gate(&gate)).await
}

/// `connection_resume()`: VERIFY again; the gatekeeper opens only when it passes.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn connection_resume<R: Runtime>(app: AppHandle<R>) -> Res<()> {
    resume(app, super::env::proxy()).await
}

/// Resumes the core, then VERIFYs `proxy` off the main thread.
async fn resume<R: Runtime>(app: AppHandle<R>, proxy: Result<LoopbackAddr, String>) -> Res<()> {
    let check = app.clone();
    act(app, Core::resume).await?;
    super::watch::check_now(&check, proxy);
    Ok(())
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

/// `console_status()`: the stored router console detection; no probe.
#[tauri::command]
pub async fn console_status<R: Runtime>(app: AppHandle<R>) -> ConsoleInfo {
    super::console::info_of(super::console::current(&app).as_ref())
}

/// `console_detect()`: probes for the router console now, off the main thread.
#[tauri::command]
pub async fn console_detect<R: Runtime>(app: AppHandle<R>) -> ConsoleInfo {
    let detect = tauri::async_runtime::spawn_blocking(move || super::console::detect_now(&app));
    detect.await.unwrap_or_else(|_| ConsoleInfo::none())
}

/// `console_open()`: opens the home page of the detected router console in the console
/// tab (R12, R24). Async, so the webview is never built inside a main-thread command.
///
/// # Errors
///
/// Fails when the console webview cannot be built.
#[tauri::command]
pub async fn console_open<R: Runtime>(app: AppHandle<R>) -> Res<ControlResult> {
    open_console(&app)
}

/// Opens the stored console in the console tab: `no-console` without one.
///
/// # Errors
///
/// Fails when the console webview cannot be built.
pub fn open_console<R: Runtime>(app: &AppHandle<R>) -> Res<ControlResult> {
    let Some(console) = super::console::current(app) else {
        return Ok(ControlResult {
            ok: false,
            reason: Some("no-console"),
        });
    };
    super::console::ConsoleWebview::open(app, &console).map_err(|e| e.to_string())?;
    Ok(ControlResult {
        ok: true,
        reason: None,
    })
}

/// `chrome_set_height(px)`: the toolbar reports its find bar (124 or more: open). The toolbar
/// never takes another height than 84 or 124; popups show in the `popup` webview.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn chrome_set_height<R: Runtime>(app: AppHandle<R>, px: f64) -> Res<()> {
    act(app, move |c| c.set_toolbar_request(px)).await
}

/// `popup_open({kind, anchor, data?}) -> id`: opens a toolbar popup under `anchor`.
///
/// # Errors
///
/// Fails for an anchor that is not finite or has a negative side.
#[tauri::command]
pub async fn popup_open<R: Runtime>(
    app: AppHandle<R>,
    kind: Kind,
    anchor: Anchor,
    data: Option<Value>,
) -> Res<u64> {
    let rect = anchor.rect().ok_or_else(|| "bad anchor".to_owned())?;
    let data = data.unwrap_or(Value::Null);
    blocking(move || super::popup::open(&app, kind, rect, &data)).await
}

/// `popup_size({id, width, height})`: the popup page measured its card.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn popup_size<R: Runtime>(
    app: AppHandle<R>,
    id: u64,
    width: f64,
    height: f64,
) -> Res<()> {
    blocking(move || super::popup::sized(&app, id, (width, height))).await
}

/// `popup_close({id, refocus?})`: closes popup `id`; a stale id does nothing.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn popup_close<R: Runtime>(app: AppHandle<R>, id: u64, refocus: Option<bool>) -> Res<()> {
    let refocus = refocus.unwrap_or(false);
    blocking(move || super::popup::close(&app, Some(id), refocus)).await
}

/// `chrome_insets()`: the space the tab strip leaves for the macOS window buttons (0 in
/// full screen and on Windows and Linux).
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn chrome_insets<R: Runtime>(app: AppHandle<R>) -> Res<ChromeInsets> {
    blocking(move || super::view::chrome_insets(&app)).await
}

/// `window_fullscreen()`: whether the window is in full screen now.
///
/// # Errors
///
/// Fails only when the worker thread that runs the command panics.
#[tauri::command]
pub async fn window_fullscreen<R: Runtime>(app: AppHandle<R>) -> Res<bool> {
    blocking(move || {
        let window = app.get_window("main");
        window.is_some_and(|w| w.is_fullscreen().unwrap_or(false))
    })
    .await
}

/// `platform()`: `macos`, `windows` or `linux`.
#[tauri::command]
#[must_use]
pub fn platform() -> &'static str {
    PLATFORM
}

#[cfg(target_os = "macos")]
const PLATFORM: &str = "macos";
#[cfg(windows)]
const PLATFORM: &str = "windows";
#[cfg(not(any(target_os = "macos", windows)))]
const PLATFORM: &str = "linux";

#[cfg(test)]
mod tests;
