// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Carries out core [`Effect`]s on the main thread. The core lock is never held while a
//! webview method runs, because engine callbacks take the same lock.

use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, Runtime, Url, Webview};

use super::arming::Next;
use super::content::ContentWebview;
use super::state::{lock, now_ms, shared};
use super::{engine, view};
use crate::core::{Core, Effect, Event, Load, View, WebOp};
use crate::diag::{self, Code, ErrorKind, Field, OpKind, TabKind};
use crate::hover::SHOW_DELAY_MS;
use crate::{icons, net};

/// The webviews that receive contract events.
pub const LISTENERS: [&str; 4] = ["toolbar", "internal", "status", "popup"];

/// Runs `f` on the core, then carries out its effects on the main thread.
pub fn with_core<R: Runtime>(app: &AppHandle<R>, f: impl FnOnce(&mut Core) -> Vec<Effect>) {
    let fx = f(&mut lock(&shared(app).core));
    later(app, fx);
}

/// Queues `fx` on the main thread, always through the event loop: called on the main thread,
/// `run_on_main_thread` would run them inline, inside whatever engine callback or
/// `with_webview` closure is on the stack (the deadlock of `outside`).
pub fn later<R: Runtime>(app: &AppHandle<R>, fx: Vec<Effect>) {
    if fx.is_empty() {
        return;
    }
    let handle = app.clone();
    outside(move || {
        let runner = handle.clone();
        if let Err(e) = runner.run_on_main_thread(move || apply(&handle, fx)) {
            diag::event(
                Code::ThreadFailed,
                &[
                    Field::Op(OpKind::MainThread),
                    Field::Error(ErrorKind::from(&e)),
                ],
            );
        }
    });
}

/// Runs `f` on a new thread. Use it for any Tauri webview call that starts inside a
/// `with_webview` closure: Tauri holds the webview's window lock while that closure runs on
/// the main thread, so a nested call on the same webview (`navigate`, `eval`, `emit`) waits
/// for that lock forever. From another thread the call goes through the event loop instead.
pub fn outside(f: impl FnOnce() + Send + 'static) {
    if let Err(e) = thread::Builder::new().name("webview-call".into()).spawn(f) {
        diag::event(
            Code::ThreadFailed,
            &[Field::Op(OpKind::Spawn), Field::Error(ErrorKind::from(&e))],
        );
    }
}

/// Carries out `fx` now. Call on the main thread.
pub fn apply<R: Runtime>(app: &AppHandle<R>, fx: Vec<Effect>) {
    let (mut relayout, mut focus) = (false, false);
    for effect in fx {
        relayout |= matches!(effect, Effect::Layout | Effect::Web(_) | Effect::Console(_));
        match effect {
            Effect::Emit(event) => emit(app, &event),
            Effect::Web(op) => web(app, op),
            Effect::FocusToolbar => focus_toolbar(app),
            Effect::FocusContent => focus = true,
            Effect::HoverLater(generation) => show_later(app, generation),
            Effect::FetchIcon(host) => fetch_icon(app, host),
            Effect::Console(op) => super::console::run(app, &op),
            Effect::Layout => {}
        }
    }
    if relayout {
        view::sync(app);
    }
    if focus {
        // After the layout: the view that shows now is the one that takes the keys.
        focus_content(app);
    }
}

/// Gives keyboard focus to the webview the content area shows.
fn focus_content<R: Runtime>(app: &AppHandle<R>) {
    let label = match lock(&shared(app).core).view() {
        View::Web(tab) => tab_webview(app, tab).map(|w| w.label().to_owned()),
        View::Internal(_) => Some("internal".to_owned()),
        View::Console(_) => Some(super::console::CONSOLE_LABEL.to_owned()),
    };
    if let Some(webview) = label.and_then(|l| app.get_webview(&l)) {
        let _ = webview.set_focus();
    }
}

fn focus_toolbar<R: Runtime>(app: &AppHandle<R>) {
    if let Some(toolbar) = app.get_webview("toolbar") {
        let _ = toolbar.set_focus();
    }
}

fn show_later<R: Runtime>(app: &AppHandle<R>, generation: u64) {
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(SHOW_DELAY_MS));
        with_core(&app, |core| core.hover_expire(generation));
    });
}

/// Asks `host` for its icon on a worker thread, through the gatekeeper only. Without a
/// gatekeeper nothing is sent, and the attempt ends as a failure.
fn fetch_icon<R: Runtime>(app: &AppHandle<R>, host: String) {
    let gate = lock(&shared(app).gate).clone();
    let (worker, name) = (app.clone(), host.clone());
    let spawned = thread::Builder::new()
        .name("site-icon".into())
        .spawn(move || {
            let outcome = gate.map(|g| net::icons::fetch(&g, &host));
            with_core(&worker, |core| icon_done(core, &host, outcome));
        });
    if let Err(e) = spawned {
        diag::event(
            Code::ThreadFailed,
            &[Field::Op(OpKind::Spawn), Field::Error(ErrorKind::from(&e))],
        );
        with_core(app, |core| core.icon_unreached(&name, now_ms()));
    }
}

/// Hands a fetch result to the core. No gatekeeper, or no request that reached the site,
/// does not count as an attempt; any other failure does.
fn icon_done(
    core: &mut Core,
    host: &str,
    outcome: Option<Result<Vec<u8>, net::icons::FetchError>>,
) -> Vec<Effect> {
    match outcome {
        None => core.icon_unreached(host, now_ms()),
        Some(Err(e)) if !e.reached_site() => core.icon_unreached(host, now_ms()),
        Some(result) => {
            let icon = result.ok().and_then(|body| icons::sanitize(&body).ok());
            core.icon_fetched(host, icon, now_ms())
        }
    }
}

/// The event name and payload of a core event.
fn payload(core: &Core, event: &Event) -> (&'static str, Value) {
    match event {
        Event::TabsChanged => ("tabs-changed", json!(core.tab_infos())),
        Event::TabUpdated(id) => ("tab-updated", json!(core.tab_info(*id))),
        Event::Find(result) => ("find-result", json!(result)),
        Event::Router => ("router-status", json!(core.router_report())),
        Event::Bookmarks => ("bookmarks-changed", Value::Null),
        Event::History => ("history-changed", Value::Null),
        Event::Settings => ("settings-changed", json!(core.settings())),
        Event::Shortcut(action) => ("shortcut", json!({ "action": action })),
        Event::Toast(toast) => ("toast", json!(toast)),
        Event::Hover(text) => ("link-hover", hover_payload(text.as_ref())),
        Event::Icons => ("icons-changed", Value::Null),
    }
}

fn hover_payload(text: Option<&crate::hover::HoverText>) -> Value {
    text.map_or_else(|| json!({ "text": "", "blocked": false }), |t| json!(t))
}

fn emit<R: Runtime>(app: &AppHandle<R>, event: &Event) {
    if matches!(event, Event::TabUpdated(_) | Event::TabsChanged) {
        view::sync_stop_item(app);
    }
    if matches!(event, Event::Settings) {
        super::surface::paint_window(app);
    }
    if let Event::Hover(text) = event {
        view::status(app, text.as_ref());
    }
    let (name, body) = payload(&lock(&shared(app).core), event);
    for label in LISTENERS {
        if app.get_webview(label).is_some() {
            let _ = app.emit_to(label, name, body.clone());
        }
    }
}

fn web<R: Runtime>(app: &AppHandle<R>, op: WebOp) {
    match op {
        WebOp::Load(load) => load_tab(app, &load),
        WebOp::Destroy(tab) => destroy(app, tab),
        WebOp::Engine(tab, op) => {
            if let Some(webview) = tab_webview(app, tab) {
                engine::run(&webview, tab, &op);
            }
        }
    }
}

/// The live webview of a tab.
pub fn tab_webview<R: Runtime>(app: &AppHandle<R>, tab: u32) -> Option<Webview<R>> {
    let label = lock(&shared(app).labels).get(&tab).cloned()?;
    app.get_webview(&label)
}

fn destroy<R: Runtime>(app: &AppHandle<R>, tab: u32) {
    let Some(label) = lock(&shared(app).labels).remove(&tab) else {
        return;
    };
    lock(&shared(app).arming).forget(&label);
    if let Some(webview) = app.get_webview(&label) {
        let _ = webview.close();
    }
}

/// Loads a page in a tab. The live webview loads it only when its engine filter is on (L3b).
/// Before that the URL waits for the filter; after a failed filter a new webview is built.
fn load_tab<R: Runtime>(app: &AppHandle<R>, load: &Load) {
    let live = (!load.rebuild)
        .then(|| tab_webview(app, load.tab))
        .flatten();
    if let Some(webview) = live {
        let Ok(url) = Url::parse(&load.url) else {
            return;
        };
        let next = lock(&shared(app).arming).load(webview.label(), &url);
        match next {
            Next::Navigate => {
                let _ = webview.navigate(url);
                return;
            }
            Next::Wait => return,
            Next::Rebuild => {}
        }
    }
    destroy(app, load.tab);
    create(app, load);
}

/// Builds a tab webview. Without a gatekeeper there is none: fail closed.
fn create<R: Runtime>(app: &AppHandle<R>, load: &Load) {
    let state = shared(app);
    let Some(gate) = lock(&state.gate).clone() else {
        return;
    };
    let Some(window) = app.get_window("main") else {
        return;
    };
    let label = state.next_label(load.tab);
    match ContentWebview::create(&gate, &window, &label, load, view::content_rect(app)) {
        Ok(webview) => {
            let _ = webview.hide();
            lock(&state.labels).insert(load.tab, label);
            view::raise_chrome(app, &window);
        }
        Err(e) => {
            lock(&state.arming).forget(&label);
            diag::event(
                Code::WebviewCreateFailed,
                &[Field::Tab(TabKind::Web), Field::Error(ErrorKind::from(&e))],
            );
        }
    }
}

#[cfg(test)]
mod tests;
