// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Engine calls on a `tab-*` webview. They work with page JavaScript off: back, forward,
//! stop, hard reload and find go through the native engine API in `eepview-platform`;
//! reload and zoom use Tauri. Find never runs a script in the page's own JavaScript world:
//! there a page could replace `window.find`, read every query and fake the match count.

use eepview_platform::{FindRequest, LoadFailure, Nav, PlatformWebview};
use tauri::{AppHandle, Manager, Runtime, Webview};

use super::apply::with_core;
use super::state::{lock, shared};
use crate::core::{EngineOp, FindOp};
use crate::diag::{self, Code, ErrorKind, Field, OpKind};

/// Carries out one engine call.
pub fn run<R: Runtime>(webview: &Webview<R>, tab: u32, op: &EngineOp) {
    let result = match op {
        EngineOp::Reload => webview.reload(),
        EngineOp::Zoom(level) => webview.set_zoom(*level),
        EngineOp::Find(find) => find_text(webview, tab, find),
        EngineOp::FindClear => webview.with_webview(|p| eepview_platform::find_clear(&p)),
        EngineOp::Back => go(webview, Nav::Back),
        EngineOp::Forward => go(webview, Nav::Forward),
        EngineOp::HardReload => go(webview, Nav::HardReload),
        EngineOp::Stop => go(webview, Nav::Stop),
    };
    if let Err(e) = result {
        let (code, kind) = failure(op);
        diag::event(code, &[Field::Op(kind), Field::Error(ErrorKind::from(&e))]);
    }
}

/// The event code and operation kind of a failed engine call.
#[must_use]
pub fn failure(op: &EngineOp) -> (Code, OpKind) {
    match op {
        EngineOp::Reload => (Code::EngineCallFailed, OpKind::Reload),
        EngineOp::HardReload => (Code::EngineCallFailed, OpKind::HardReload),
        EngineOp::Back => (Code::EngineCallFailed, OpKind::Back),
        EngineOp::Forward => (Code::EngineCallFailed, OpKind::Forward),
        EngineOp::Stop => (Code::EngineCallFailed, OpKind::Stop),
        EngineOp::Zoom(_) => (Code::ZoomFailed, OpKind::Zoom),
        EngineOp::Find(_) => (Code::FindFailed, OpKind::Find),
        EngineOp::FindClear => (Code::FindFailed, OpKind::FindClear),
    }
}

/// The operation kind of a bridge navigation.
fn nav_op(nav: Nav) -> OpKind {
    match nav {
        Nav::Back => OpKind::Back,
        Nav::Forward => OpKind::Forward,
        Nav::Reload => OpKind::Reload,
        Nav::HardReload => OpKind::HardReload,
        Nav::Stop => OpKind::Stop,
    }
}

/// Records a failed call of the platform bridge. Its error text is never logged.
fn bridge_failed(op: OpKind) {
    diag::event(
        Code::EngineCallFailed,
        &[Field::Op(op), Field::Error(ErrorKind::Platform)],
    );
}

fn go<R: Runtime>(webview: &Webview<R>, nav: Nav) -> tauri::Result<()> {
    webview.with_webview(move |platform| {
        if eepview_platform::go(&platform, nav).is_err() {
            bridge_failed(nav_op(nav));
        }
    })
}

/// Hardens the engine, hooks the link under the mouse to the status bubble, and hooks the
/// input (links, Esc, mouse buttons, context menus).
pub fn native_hooks<R: Runtime>(
    platform: &PlatformWebview,
    app: &AppHandle<R>,
    tab: u32,
    label: &str,
) {
    super::input::install(app, platform, super::input::Source::Tab(tab));
    if eepview_platform::harden(platform).is_err() {
        bridge_failed(OpKind::Harden);
    }
    if eepview_platform::on_hover(platform, hover_callback(app, tab)).is_err() {
        bridge_failed(OpKind::Hover);
    }
    if eepview_platform::on_load_failed(platform, fail_callback(app, tab, label)).is_err() {
        bridge_failed(OpKind::LoadFailed);
    }
}

/// The callback that gets a load the engine failed in `tab`. A cancelled load is no failure,
/// and a failure from a webview that no longer belongs to the tab is ignored (F8).
fn fail_callback<R: Runtime>(
    app: &AppHandle<R>,
    tab: u32,
    label: &str,
) -> Box<dyn Fn(LoadFailure)> {
    let (app, label) = (app.clone(), label.to_owned());
    Box::new(move |failure: LoadFailure| {
        let current = lock(&shared(&app).labels).get(&tab) == Some(&label);
        let Some(reason) = failure.reason().filter(|_| current) else {
            return;
        };
        let code = failure.code_text();
        with_core(&app, |core| {
            core.load_failed(tab, failure.url.as_deref(), reason.as_str(), &code)
        });
    })
}

/// The callback that gets the link under the mouse in `tab`.
fn hover_callback<R: Runtime>(app: &AppHandle<R>, tab: u32) -> Box<dyn Fn(Option<String>)> {
    let app = app.clone();
    Box::new(move |link: Option<String>| {
        with_core(&app, |core| core.hover_link(tab, link.as_deref()));
    })
}

/// The callback that gets the match count of a find in `tab`.
fn count_callback<R: Runtime>(app: &AppHandle<R>, tab: u32) -> Box<dyn Fn(Option<u32>)> {
    let app = app.clone();
    Box::new(move |count: Option<u32>| {
        with_core(&app, |core| core.find_counted(tab, count));
    })
}

/// The bridge request of a find.
#[must_use]
pub fn find_request(find: &FindOp) -> FindRequest {
    FindRequest {
        query: find.query.clone(),
        backwards: !find.forward,
        case_sensitive: find.match_case,
        fresh: find.fresh,
    }
}

fn find_text<R: Runtime>(webview: &Webview<R>, tab: u32, find: &FindOp) -> tauri::Result<()> {
    let request = find_request(find);
    let app = webview.app_handle().clone();
    webview.with_webview(move |platform| {
        if !eepview_platform::find(&platform, &request, count_callback(&app, tab)) {
            find_failed(&app, tab, request.fresh);
        }
    })
}

/// The engine could not search. No script stands in for it, because a page could read the
/// query. A fresh search reports no count.
fn find_failed<R: Runtime>(app: &AppHandle<R>, tab: u32, fresh: bool) {
    diag::event(
        Code::FindFailed,
        &[Field::Op(OpKind::Find), Field::Error(ErrorKind::Platform)],
    );
    if fresh {
        with_core(app, |core| core.find_counted(tab, None));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::testing::{app, core};

    fn op(query: &str) -> FindOp {
        FindOp {
            query: query.into(),
            forward: true,
            match_case: false,
            fresh: true,
        }
    }

    #[test]
    fn every_engine_call_runs() {
        let app = app();
        let webview = app.get_webview("status").unwrap();
        for call in [
            EngineOp::Reload,
            EngineOp::Zoom(1.5),
            EngineOp::Find(op("x")),
            EngineOp::FindClear,
            EngineOp::Back,
            EngineOp::Forward,
            EngineOp::HardReload,
            EngineOp::Stop,
        ] {
            run(&webview, 1, &call);
        }
    }

    #[test]
    fn engine_callbacks_reach_the_core() {
        let app = app();
        let tab = core(&app).tabs().active().unwrap().id;
        hover_callback(app.handle(), tab)(Some("http://a.i2p/".into()));
        hover_callback(app.handle(), tab)(None);
        count_callback(app.handle(), tab)(Some(2));
        find_failed(app.handle(), tab, true);
        find_failed(app.handle(), tab, false);
    }

    #[test]
    fn failures_name_their_operation() {
        assert_eq!(failure(&EngineOp::Zoom(1.0)).0, Code::ZoomFailed);
        assert_eq!(failure(&EngineOp::FindClear).0, Code::FindFailed);
        assert_eq!(
            failure(&EngineOp::Stop),
            (Code::EngineCallFailed, OpKind::Stop)
        );
        for nav in [
            Nav::Back,
            Nav::Forward,
            Nav::Reload,
            Nav::HardReload,
            Nav::Stop,
        ] {
            bridge_failed(nav_op(nav));
        }
    }

    #[test]
    fn find_request_maps_direction_and_case() {
        let op = FindOp {
            query: "x".into(),
            forward: false,
            match_case: true,
            fresh: false,
        };
        let request = find_request(&op);
        assert!(request.backwards && request.case_sensitive && !request.fresh);
        assert_eq!(request.query, "x");
    }
}
