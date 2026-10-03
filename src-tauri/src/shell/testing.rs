// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Shell test fixtures: an app on the Tauri mock runtime with the shared state, the IPC
//! handler and the bundled webviews, plus helpers to drive it.

use std::sync::{Arc, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, Webview};

use super::env::DEFAULT_PROXY;
use super::state::{Shared, lock, shared};
use crate::core::{Core, Load};
use crate::net::gatekeeper::Gatekeeper;
use crate::net::testing::FakeRouter;

/// The mock runtime.
pub type Mock = MockRuntime;

/// The origin of the bundled pages.
#[cfg(windows)]
const ORIGIN: &str = "http://tauri.localhost";
#[cfg(not(windows))]
const ORIGIN: &str = "tauri://localhost";

/// An app with the shared state and the IPC handler, but no window.
pub fn bare() -> App<Mock> {
    let app = mock_builder()
        .invoke_handler(super::handler())
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(Shared::<Mock>::new(Core::new(None, DEFAULT_PROXY, 0)));
    app
}

/// An app with the main window and the `toolbar`, `internal` and `status` webviews.
pub fn app() -> App<Mock> {
    let mut app = bare();
    super::chrome::build(&mut app).unwrap();
    app
}

/// The core, locked.
pub fn core(app: &App<Mock>) -> MutexGuard<'_, Core> {
    lock(&shared(app.handle()).inner().core)
}

/// Starts a gatekeeper in front of `router`, as a passing VERIFY does.
pub fn open_gate(app: &App<Mock>, router: &FakeRouter) {
    let gate = Gatekeeper::start(&router.verified()).unwrap();
    *lock(&shared(app.handle()).inner().gate) = Some(Arc::new(gate));
}

/// True while a gatekeeper runs.
pub fn gate_open(app: &App<Mock>) -> bool {
    lock(&shared(app.handle()).inner().gate).is_some()
}

/// The webview label of a tab.
pub fn label(app: &App<Mock>, tab: u32) -> Option<String> {
    lock(&shared(app.handle()).inner().labels)
        .get(&tab)
        .cloned()
}

/// A load of `url` in `tab`.
pub fn load(tab: u32, url: &str) -> Load {
    Load {
        tab,
        url: url.to_owned(),
        js: true,
        rebuild: false,
        private: false,
        zoom: 1.0,
    }
}

/// Calls an IPC command from the `toolbar` webview, as the UI does.
pub fn invoke(app: &App<Mock>, cmd: &str, args: Value) -> Result<Value, Value> {
    let webview = app.get_webview("toolbar").unwrap();
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: ORIGIN.parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: tauri::http::HeaderMap::default(),
        invoke_key: INVOKE_KEY.to_owned(),
    };
    get_ipc_response(&Caller(webview), request).map(|b| b.deserialize::<Value>().unwrap())
}

/// A webview as the IPC test helper takes it.
struct Caller(Webview<Mock>);

impl AsRef<Webview<Mock>> for Caller {
    fn as_ref(&self) -> &Webview<Mock> {
        &self.0
    }
}

/// Waits up to 10 s for `done`.
pub fn wait_for(mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while !done() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    done()
}
