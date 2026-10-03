//! Everything that calls Tauri. The browser logic is in `core`; this module turns its
//! effects into webview calls and turns engine callbacks into core calls.
//!
//! - [`chrome`]: the window and the bundled webviews (`toolbar`, `internal`, `status`).
//! - [`content`]: the only factory of remote `tab-*` webviews (ADR 0001).
//! - [`engine`]: back, forward, stop, find and zoom with page JavaScript off.
//! - [`commands`]: the IPC commands.

pub mod apply;
pub mod chrome;
pub mod commands;
pub mod content;
pub mod engine;
pub mod env;
pub mod log;
pub mod menu;
pub mod state;
pub mod view;
pub mod watch;

use std::thread;
use std::time::Duration;

use tauri::{App, Manager, RunEvent, WindowEvent};

use crate::core::{Core, Paths};
use state::{Shared, now_ms};

/// Starts the browser and blocks until it exits.
///
/// # Errors
///
/// Fails when Tauri cannot start, for example when the system web view is missing.
pub fn run() -> tauri::Result<()> {
    log::start();
    let app = tauri::Builder::default()
        .invoke_handler(handler())
        .on_menu_event(|app, event| menu::on_event(app, event.id().as_ref()))
        .setup(|app| setup(app).map_err(Into::into))
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        if let RunEvent::WindowEvent {
            event: WindowEvent::Resized(_),
            ..
        } = event
        {
            view::sync(handle);
            view::check_fullscreen(handle);
        }
    });
    Ok(())
}

/// Every IPC command of the contract.
fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::tab_new,
        commands::tab_close,
        commands::tab_select,
        commands::tab_move,
        commands::tab_list,
        commands::navigate,
        commands::go_back,
        commands::go_forward,
        commands::reload,
        commands::stop,
        commands::home,
        commands::find,
        commands::find_close,
        commands::zoom_in,
        commands::zoom_out,
        commands::zoom_reset,
        commands::site_js_set,
        commands::bookmarks_list,
        commands::bookmark_add,
        commands::bookmark_update,
        commands::bookmark_remove,
        commands::bookmark_find,
        commands::bookmarks_export,
        commands::bookmarks_export_file,
        commands::bookmarks_import,
        commands::history_query,
        commands::history_remove,
        commands::history_clear,
        commands::suggest,
        commands::settings_get,
        commands::settings_set,
        commands::router_status,
        commands::router_stats,
        commands::chrome_set_height,
        commands::toolbar_set_height,
        commands::platform,
    ]
}

fn paths(app: &App) -> Option<Paths> {
    let config = app.path().app_config_dir().ok()?;
    let data = app.path().app_data_dir().ok()?;
    Some(Paths {
        bookmarks: config.join("bookmarks.json"),
        history: data.join("history.json"),
        settings: config.join("settings.json"),
        sites: config.join("sites.json"),
    })
}

fn setup(app: &mut App) -> tauri::Result<()> {
    let mut core = Core::new(paths(app), &env::proxy_text(), now_ms());
    if env::js_forced_off() {
        core.force_js_off();
    }
    app.manage(Shared::<tauri::Wry>::new(core));
    let menu = menu::build(app.handle())?;
    app.set_menu(menu)?;
    chrome::build(app)?;
    let handle = app.handle().clone();
    for url in env::start_urls() {
        let first = { state::lock(&state::shared(&handle).core).tabs().len() == 1 };
        apply::with_core(&handle, |core| open_start(core, &url, first));
    }
    watch::start(&handle, env::proxy());
    if let Some(seconds) = env::exit_after() {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(seconds));
            handle.exit(0);
        });
    }
    apply::later(app.handle(), vec![crate::core::Effect::Layout]);
    Ok(())
}

/// The first start URL replaces the home tab; the others open new tabs.
fn open_start(core: &mut Core, url: &str, first: bool) -> Vec<crate::core::Effect> {
    let home_only = first
        && core
            .tabs()
            .active()
            .is_some_and(|t| t.url == core.home_url());
    if home_only {
        core.navigate(url).1
    } else {
        core.tab_new(Some(url), crate::tabs::Place::End).1
    }
}
