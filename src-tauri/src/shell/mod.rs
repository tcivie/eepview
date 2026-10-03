// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Everything that calls Tauri. The browser logic is in `core`; this module turns its
//! effects into webview calls and turns engine callbacks into core calls.
//!
//! - [`chrome`]: the window and the bundled webviews (`toolbar`, `internal`, `status`,
//!   `popup`).
//! - [`popup`]: places and shows the toolbar popups.
//! - [`content`]: the only factory of remote `tab-*` webviews (ADR 0001).
//! - [`console`]: the only factory of the `console` webview (the router's own pages).
//! - [`engine`]: back, forward, stop, find and zoom with page JavaScript off.
//! - [`commands`]: the IPC commands.

pub mod apply;
pub mod chrome;
pub mod commands;
pub mod console;
pub mod content;
pub mod engine;
pub mod env;
pub mod log;
pub mod menu;
pub mod popup;
pub mod state;
#[cfg(test)]
mod testing;
pub mod view;
pub mod watch;

use std::thread;
use std::time::Duration;

use tauri::{App, AppHandle, Manager, RunEvent, Runtime, WindowEvent};

use crate::core::{Core, Effect, Paths};
use crate::net::loopback::LoopbackAddr;
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
        if let RunEvent::WindowEvent { event, .. } = event {
            on_window_event(handle, &event);
        }
    });
    Ok(())
}

/// A window resize lays the webviews out again and closes the open popup.
fn on_window_event<R: Runtime>(handle: &AppHandle<R>, event: &WindowEvent) {
    match event {
        WindowEvent::Resized(_) => {
            popup::close(handle, None, false);
            view::sync(handle);
            view::check_fullscreen(handle);
            view::place_buttons(handle);
        }
        WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Focused(_) => {
            view::place_buttons(handle);
        }
        _ => {}
    }
}

/// Every IPC command of the contract, as one `generate_handler!` list.
macro_rules! contract_handler {
    () => {
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
            commands::connection_pause,
            commands::connection_resume,
            commands::router_control,
            commands::console_status,
            commands::console_detect,
            commands::console_open,
            commands::chrome_set_height,
            commands::platform,
            commands::chrome_insets,
            commands::window_fullscreen,
            commands::popup_open,
            commands::popup_size,
            commands::popup_close,
        ]
    };
}

/// Every IPC command of the contract.
fn handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    contract_handler!()
}

fn paths<R: Runtime>(app: &App<R>) -> Option<Paths> {
    let config = app.path().app_config_dir().ok()?;
    let data = app.path().app_data_dir().ok()?;
    Some(Paths {
        bookmarks: config.join("bookmarks.json"),
        history: data.join("history.json"),
        settings: config.join("settings.json"),
        sites: config.join("sites.json"),
        icons: data.join("icons"),
    })
}

fn setup<R: Runtime>(app: &mut App<R>) -> tauri::Result<()> {
    let core = new_core(paths(app), &env::proxy_text(), env::js_forced_off());
    app.manage(Shared::<R>::new(core));
    let menu = menu::build(app.handle())?;
    app.set_menu(menu)?;
    start(app, Inputs::from_env())
}

/// The process inputs that [`start`] acts on.
struct Inputs {
    urls: Vec<String>,
    proxy: Result<LoopbackAddr, String>,
    exit_after: Option<u64>,
}

impl Inputs {
    fn from_env() -> Self {
        Self {
            urls: env::start_urls(),
            proxy: env::proxy(),
            exit_after: env::exit_after(),
        }
    }
}

/// The core, with page JavaScript off everywhere when `js_off`.
fn new_core(paths: Option<Paths>, proxy: &str, js_off: bool) -> Core {
    let mut core = Core::new(paths, proxy, now_ms());
    if js_off {
        core.force_js_off();
    }
    core
}

/// Builds the window, opens the start URLs and starts the router watcher.
fn start<R: Runtime>(app: &mut App<R>, inputs: Inputs) -> tauri::Result<()> {
    chrome::build(app)?;
    let handle = app.handle().clone();
    view::place_buttons(&handle);
    open_start_urls(&handle, &inputs.urls);
    watch::start(&handle, inputs.proxy);
    if let Some(seconds) = inputs.exit_after {
        exit_later(handle, seconds);
    }
    apply::later(app.handle(), vec![Effect::Layout]);
    Ok(())
}

fn open_start_urls<R: Runtime>(handle: &AppHandle<R>, urls: &[String]) {
    for url in urls {
        let first = state::lock(&state::shared(handle).core).tabs().len() == 1;
        apply::with_core(handle, |core| open_start(core, url, first));
    }
}

/// Quits after `seconds` (`EEPVIEW_EXIT_AFTER`).
fn exit_later<R: Runtime>(handle: AppHandle<R>, seconds: u64) {
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(seconds));
        handle.exit(0);
    });
}

/// The first start URL replaces the home tab; the others open new tabs.
fn open_start(core: &mut Core, url: &str, first: bool) -> Vec<Effect> {
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

#[cfg(test)]
mod tests {
    use tauri::PhysicalSize;

    use super::*;
    use crate::shell::testing::{bare, core, wait_for};

    fn inputs(urls: &[&str]) -> Inputs {
        Inputs {
            urls: urls.iter().map(|u| (*u).to_owned()).collect(),
            proxy: Err("EEPVIEW_PROXY: test".into()),
            exit_after: Some(86_400),
        }
    }

    #[test]
    fn start_builds_the_window_and_opens_the_urls() {
        let mut app = bare();
        start(&mut app, inputs(&["http://a.i2p/", "http://b.i2p/"])).unwrap();
        assert!(app.get_webview("toolbar").is_some());
        let urls: Vec<String> = core(&app).tabs().iter().map(|t| t.url.clone()).collect();
        assert_eq!(urls, ["http://a.i2p/", "http://b.i2p/"]);
        assert!(wait_for(|| core(&app).router().state == "down"));
    }

    #[test]
    fn the_first_url_replaces_only_the_home_tab() {
        let mut core = Core::new(None, env::DEFAULT_PROXY, 0);
        open_start(&mut core, "http://a.i2p/", true);
        assert_eq!(core.tabs().len(), 1);
        open_start(&mut core, "http://b.i2p/", true);
        assert_eq!(core.tabs().len(), 2);
        open_start(&mut core, "http://c.i2p/", false);
        assert_eq!(core.tabs().len(), 3);
    }

    #[test]
    fn new_core_can_force_javascript_off() {
        for js_off in [false, true] {
            let core = new_core(None, "127.0.0.1:4445", js_off);
            assert_eq!(core.router().proxy, "127.0.0.1:4445");
        }
        let env = Inputs::from_env();
        assert_eq!(env.exit_after, env::exit_after());
    }

    #[test]
    fn stores_live_in_the_app_folders() {
        let app = bare();
        let p = paths(&app).unwrap();
        let names = [p.bookmarks, p.history, p.settings, p.sites]
            .map(|path| path.file_name().unwrap().to_owned());
        assert_eq!(
            names,
            [
                "bookmarks.json",
                "history.json",
                "settings.json",
                "sites.json"
            ]
        );
    }

    #[test]
    fn resizes_lay_the_webviews_out() {
        let app = bare();
        let resized = WindowEvent::Resized(PhysicalSize::new(800, 600));
        on_window_event(app.handle(), &resized);
        on_window_event(app.handle(), &WindowEvent::Focused(true));
    }
}
