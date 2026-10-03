// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Engine calls on a `tab-*` webview. They work with page JavaScript off: back, forward,
//! stop, hard reload and find go through the native engine API in `eepview-platform`;
//! reload and zoom use Tauri. Only `WebView2` lacks a native find with counts, so it gets an
//! app-injected script (`ExecuteScript` runs with page JavaScript off).

use eepview_platform::{FindRequest, Nav, PlatformWebview};
use tauri::{AppHandle, Manager, Runtime, Webview};

use super::apply::with_core;
use crate::core::{EngineOp, FindOp};

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
        super::log::error("engine call", &e.to_string());
    }
}

fn go<R: Runtime>(webview: &Webview<R>, nav: Nav) -> tauri::Result<()> {
    webview.with_webview(move |platform| {
        if let Err(e) = eepview_platform::go(&platform, nav) {
            super::log::error("engine call", &e);
        }
    })
}

/// Hardens the engine and hooks the link under the mouse to the status bubble.
pub fn native_hooks<R: Runtime>(platform: &PlatformWebview, app: &AppHandle<R>, tab: u32) {
    if let Err(e) = eepview_platform::harden(platform) {
        super::log::error("harden", &e);
    }
    if let Err(e) = eepview_platform::on_hover(platform, hover_callback(app, tab)) {
        super::log::error("hover", &e);
    }
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
    let (live, find) = (webview.clone(), find.clone());
    let app = webview.app_handle().clone();
    webview.with_webview(move |platform| {
        if !eepview_platform::find(&platform, &request, count_callback(&app, tab)) {
            find_by_script(&live, tab, &find);
        }
    })
}

/// The find script for engines without a native find API with counts. Returns the match
/// count of a fresh search, or -1.
#[must_use]
pub fn find_script(find: &FindOp) -> String {
    let query = eepview_platform::js_string(&find.query);
    format!(
        "(function(q, fwd, cs, fresh) {{\
           if (fresh) {{ var s = window.getSelection(); if (s) s.removeAllRanges(); }}\
           window.find(q, cs, !fwd, true, false, true, false);\
           if (!fresh) return -1;\
           var t = document.body ? document.body.innerText : '';\
           if (!cs) {{ t = t.toLowerCase(); q = q.toLowerCase(); }}\
           var n = 0, i = 0;\
           while (q && (i = t.indexOf(q, i)) !== -1) {{ n++; i += q.length; }}\
           return n;\
         }})({query}, {}, {}, {})",
        find.forward, find.match_case, find.fresh
    )
}

/// Parses the result of [`find_script`]: `Some(count)` for a fresh search.
#[must_use]
pub fn parse_count(result: &str) -> Option<Option<u32>> {
    let n: i64 = result.trim().parse().ok()?;
    if n < 0 {
        return None;
    }
    Some(u32::try_from(n).ok())
}

/// The result of the find script: a fresh search reports its count.
fn script_counted<R: Runtime>(app: &AppHandle<R>, tab: u32, out: &str) {
    if let Some(count) = parse_count(out) {
        with_core(app, |core| core.find_counted(tab, count));
    }
}

fn find_by_script<R: Runtime>(webview: &Webview<R>, tab: u32, find: &FindOp) {
    let app = webview.app_handle().clone();
    let result = webview.eval_with_callback(find_script(find), move |out| {
        script_counted(&app, tab, &out);
    });
    if let Err(e) = result {
        super::log::error("find", &e.to_string());
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
        find_by_script(&webview, 1, &op("x"));
    }

    #[test]
    fn engine_callbacks_reach_the_core() {
        let app = app();
        let tab = core(&app).tabs().active().unwrap().id;
        hover_callback(app.handle(), tab)(Some("http://a.i2p/".into()));
        hover_callback(app.handle(), tab)(None);
        count_callback(app.handle(), tab)(Some(2));
        script_counted(app.handle(), tab, "4");
        script_counted(app.handle(), tab, "-1");
    }

    #[test]
    fn find_script_escapes_the_query() {
        let op = FindOp {
            query: "a\"b</script>".into(),
            forward: true,
            match_case: false,
            fresh: true,
        };
        let script = find_script(&op);
        assert!(script.contains(r#"("a\"b\u003c/script>", true, false, true)"#));
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

    #[test]
    fn counts() {
        assert_eq!(parse_count("3"), Some(Some(3)));
        assert_eq!(parse_count(" 0 "), Some(Some(0)));
        assert_eq!(parse_count("-1"), None);
        assert_eq!(parse_count("null"), None);
    }
}
