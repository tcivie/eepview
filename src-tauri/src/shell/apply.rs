//! Carries out core [`Effect`]s on the main thread. The core lock is never held while a
//! webview method runs, because engine callbacks take the same lock.

use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, Runtime, Url, Webview};

use super::content::ContentWebview;
use super::state::{lock, shared};
use super::{engine, log, view};
use crate::core::{Core, Effect, Event, Load, WebOp};
use crate::hover::HIDE_DELAY_MS;

/// The webviews that receive contract events.
const LISTENERS: [&str; 3] = ["toolbar", "internal", "status"];

/// Runs `f` on the core, then carries out its effects on the main thread.
pub fn with_core<R: Runtime>(app: &AppHandle<R>, f: impl FnOnce(&mut Core) -> Vec<Effect>) {
    let fx = f(&mut lock(&shared(app).core));
    later(app, fx);
}

/// Queues `fx` on the main thread.
pub fn later<R: Runtime>(app: &AppHandle<R>, fx: Vec<Effect>) {
    if fx.is_empty() {
        return;
    }
    let handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || apply(&handle, fx)) {
        log::error("main thread", &e.to_string());
    }
}

/// Carries out `fx` now. Call on the main thread.
pub fn apply<R: Runtime>(app: &AppHandle<R>, fx: Vec<Effect>) {
    let mut relayout = false;
    for effect in fx {
        relayout |= matches!(effect, Effect::Layout | Effect::Web(_));
        match effect {
            Effect::Emit(event) => emit(app, &event),
            Effect::Web(op) => web(app, op),
            Effect::FocusToolbar => focus_toolbar(app),
            Effect::HoverLater(generation) => hide_later(app, generation),
            Effect::Layout => {}
        }
    }
    if relayout {
        view::sync(app);
    }
}

fn focus_toolbar<R: Runtime>(app: &AppHandle<R>) {
    if let Some(toolbar) = app.get_webview("toolbar") {
        let _ = toolbar.set_focus();
    }
}

fn hide_later<R: Runtime>(app: &AppHandle<R>, generation: u64) {
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(HIDE_DELAY_MS));
        with_core(&app, |core| core.hover_expire(generation));
    });
}

/// The event name and payload of a core event.
fn payload(core: &Core, event: &Event) -> (&'static str, Value) {
    match event {
        Event::TabsChanged => ("tabs-changed", json!(core.tab_infos())),
        Event::TabUpdated(id) => ("tab-updated", json!(core.tab_info(*id))),
        Event::Find(result) => ("find-result", json!(result)),
        Event::Router => ("router-status", json!(core.router())),
        Event::Bookmarks => ("bookmarks-changed", Value::Null),
        Event::History => ("history-changed", Value::Null),
        Event::Settings => ("settings-changed", json!(core.settings())),
        Event::Shortcut(action) => ("shortcut", json!({ "action": action })),
        Event::Toast(toast) => ("toast", json!(toast)),
        Event::Hover(text) => ("link-hover", hover_payload(text.as_ref())),
    }
}

fn hover_payload(text: Option<&crate::hover::HoverText>) -> Value {
    text.map_or_else(|| json!({ "text": "", "blocked": false }), |t| json!(t))
}

fn emit<R: Runtime>(app: &AppHandle<R>, event: &Event) {
    if matches!(event, Event::TabUpdated(_) | Event::TabsChanged) {
        view::sync_stop_item(app);
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
    let label = lock(&shared(app).labels).remove(&tab);
    if let Some(webview) = label.and_then(|l| app.get_webview(&l)) {
        let _ = webview.close();
    }
}

fn load_tab<R: Runtime>(app: &AppHandle<R>, load: &Load) {
    let live = (!load.rebuild)
        .then(|| tab_webview(app, load.tab))
        .flatten();
    if let Some(webview) = live {
        if let Ok(url) = Url::parse(&load.url) {
            let _ = webview.navigate(url);
        }
        return;
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
        Err(e) => log::error("content webview", &e.to_string()),
    }
}
