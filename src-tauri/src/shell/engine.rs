//! Engine calls on a `tab-*` webview that work with page JavaScript off.
//!
//! - `WebKitGTK`: the native API through `with_webview` (back, forward, stop, reload without
//!   cache, `WebKitFindController` with counted matches, link hover).
//! - `WKWebView` and `WebView2`: Tauri gives no safe native handle (only raw pointers), and the
//!   crate denies `unsafe`. So these engines get app-injected scripts (`evaluateJavaScript`,
//!   `ExecuteScript`), which run even when page JavaScript is off. Reload and zoom use the
//!   native Tauri calls on every engine.

use tauri::{Manager, Runtime, Webview};

use super::apply::with_core;
use crate::core::{EngineOp, FindOp};

/// Carries out one engine call.
pub fn run<R: Runtime>(webview: &Webview<R>, tab: u32, op: &EngineOp) {
    let result = match op {
        EngineOp::Reload => webview.reload(),
        EngineOp::Zoom(level) => webview.set_zoom(*level),
        EngineOp::Find(find) => {
            find_text(webview, tab, find);
            Ok(())
        }
        other => platform(webview, other),
    };
    if let Err(e) = result {
        super::log::error("engine call", &e.to_string());
    }
}

/// The find script for engines without a native find API with counts. Returns the match
/// count of a fresh search, or -1.
#[must_use]
pub fn find_script(find: &FindOp) -> String {
    let query = serde_json::to_string(&find.query).unwrap_or_else(|_| "\"\"".into());
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

/// The script that clears a find selection.
pub const FIND_CLEAR_SCRIPT: &str =
    "window.getSelection() && window.getSelection().removeAllRanges()";

/// Parses the result of [`find_script`]: `Some(count)` for a fresh search.
#[must_use]
pub fn parse_count(result: &str) -> Option<Option<u32>> {
    let n: i64 = result.trim().parse().ok()?;
    if n < 0 {
        return None;
    }
    Some(u32::try_from(n).ok())
}

#[cfg(not(target_os = "linux"))]
fn platform<R: Runtime>(webview: &Webview<R>, op: &EngineOp) -> tauri::Result<()> {
    let script = match op {
        EngineOp::Back => "history.back()",
        EngineOp::Forward => "history.forward()",
        EngineOp::HardReload => "location.reload()",
        EngineOp::Stop => "window.stop()",
        _ => FIND_CLEAR_SCRIPT,
    };
    webview.eval(script)
}

#[cfg(not(target_os = "linux"))]
fn find_text<R: Runtime>(webview: &Webview<R>, tab: u32, find: &FindOp) {
    let app = webview.app_handle().clone();
    let result = webview.eval_with_callback(find_script(find), move |out| {
        if let Some(count) = parse_count(&out) {
            with_core(&app, |core| core.find_counted(tab, count));
        }
    });
    if let Err(e) = result {
        super::log::error("find", &e.to_string());
    }
}

/// Engine settings after the webview exists. Only `WebKitGTK` has switches for WebRTC and
/// media capture, and a link-hover signal.
#[cfg(not(target_os = "linux"))]
pub fn harden<R: Runtime>(_webview: &Webview<R>, _tab: u32) {}

#[cfg(target_os = "linux")]
fn platform<R: Runtime>(webview: &Webview<R>, op: &EngineOp) -> tauri::Result<()> {
    use webkit2gtk::{FindControllerExt, WebViewExt};
    let op = op.clone();
    webview.with_webview(move |platform| {
        let view = platform.inner();
        match op {
            EngineOp::Back => view.go_back(),
            EngineOp::Forward => view.go_forward(),
            EngineOp::HardReload => view.reload_bypass_cache(),
            EngineOp::Stop => view.stop_loading(),
            _ => view
                .find_controller()
                .iter()
                .for_each(FindControllerExt::search_finish),
        }
    })
}

#[cfg(target_os = "linux")]
fn find_text<R: Runtime>(webview: &Webview<R>, _tab: u32, find: &FindOp) {
    use webkit2gtk::{FindControllerExt, FindOptions, WebViewExt};
    let find = find.clone();
    let result = webview.with_webview(move |platform| {
        let Some(controller) = platform.inner().find_controller() else {
            return;
        };
        let mut options = FindOptions::WRAP_AROUND;
        if !find.match_case {
            options |= FindOptions::CASE_INSENSITIVE;
        }
        if find.fresh {
            controller.count_matches(&find.query, options.bits(), u32::MAX);
            controller.search(&find.query, options.bits(), u32::MAX);
        } else if find.forward {
            controller.search_next();
        } else {
            controller.search_previous();
        }
    });
    if let Err(e) = result {
        super::log::error("find", &e.to_string());
    }
}

/// WebKitGTK: WebRTC and media capture off in the engine (L5), counted find results, and the
/// link under the mouse for the status bubble.
#[cfg(target_os = "linux")]
pub fn harden<R: Runtime>(webview: &Webview<R>, tab: u32) {
    use webkit2gtk::{FindControllerExt, HitTestResultExt, SettingsExt, WebViewExt};
    let app = webview.app_handle().clone();
    let result = webview.with_webview(move |platform| {
        let view = platform.inner();
        if let Some(settings) = WebViewExt::settings(&view) {
            settings.set_enable_webrtc(false);
            settings.set_enable_media_stream(false);
        }
        let count_app = app.clone();
        if let Some(controller) = view.find_controller() {
            controller.connect_counted_matches(move |_, n| {
                with_core(&count_app, |core| core.find_counted(tab, Some(n)));
            });
        }
        view.connect_mouse_target_changed(move |_, hit, _| {
            let link = hit.context_is_link().then(|| hit.link_uri()).flatten();
            let link = link.as_ref().map(|s| s.as_str().to_owned());
            with_core(&app, |core| core.hover_link(tab, link.as_deref()));
        });
    });
    if let Err(e) = result {
        super::log::error("harden", &e.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_script_escapes_the_query() {
        let op = FindOp {
            query: "a\"b</script>".into(),
            forward: true,
            match_case: false,
            fresh: true,
        };
        let script = find_script(&op);
        assert!(script.contains(r#"("a\"b</script>", true, false, true)"#));
    }

    #[test]
    fn counts() {
        assert_eq!(parse_count("3"), Some(Some(3)));
        assert_eq!(parse_count(" 0 "), Some(Some(0)));
        assert_eq!(parse_count("-1"), None);
        assert_eq!(parse_count("null"), None);
    }
}
