// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Safe wrappers over engine APIs that Tauri does not expose. This crate is the only place in
//! eepview with `unsafe` code (ADR 0001); the app crate denies it and calls only the safe
//! functions below.
//!
//! Every function takes the [`PlatformWebview`] that `Webview::with_webview` hands out, so the
//! engine handle is alive and the call runs on the main thread. Callbacks get owned Rust data.
//!
//! | Function | macOS (`WKWebView`) | Windows (`WebView2`) | Linux (`WebKitGTK`) |
//! |---|---|---|---|
//! | [`attach_rules`] | `WKContentRuleList` | `WebResourceRequested` filter | none: no binding, engine is clean (spike S1) |
//! | [`on_hover`] | user script in a private `WKContentWorld` | `StatusBarTextChanged` | `mouse-target-changed` |
//! | [`go`] | `goBack` … `stopLoading` | `GoBack` … `Stop` | `go_back` … `stop_loading` |
//! | [`find`] | `findString` + a count in the private world | not handled | `WebKitFindController` |
//! | [`harden`] | fraud-check call-home off | autofill, password save, `SmartScreen` off | WebRTC and media capture off |

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

use std::fmt::Write as _;

pub use tauri::webview::PlatformWebview;

/// A navigation step on the engine's own list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    /// Back.
    Back,
    /// Forward.
    Forward,
    /// Reload.
    Reload,
    /// Reload, bypassing the cache.
    HardReload,
    /// Stop loading.
    Stop,
}

/// A find request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindRequest {
    /// The text.
    pub query: String,
    /// Search towards the start of the page.
    pub backwards: bool,
    /// Match case.
    pub case_sensitive: bool,
    /// A new search: count the matches.
    pub fresh: bool,
}

/// The rules of [`attach_rules`].
#[derive(Debug, Clone, Copy)]
pub struct Rules<'a> {
    /// `WKContentRuleList` JSON (macOS).
    pub json: &'a str,
    /// True for a request URL the engine may load (Windows filter).
    pub allow: fn(&str) -> bool,
}

/// The name of the private script world and of its message handler (macOS).
pub const HOVER_CHANNEL: &str = "eepviewHover";

/// The hover script. It runs in a private script world, so page scripts can neither see nor
/// call it, and it runs with page JavaScript off. It posts the `href` under the mouse, or an
/// empty string when the mouse leaves a link.
pub const HOVER_SCRIPT: &str = r#"(() => {
  let last = null;
  const post = (href) => {
    if (href === last) return;
    last = href;
    try { window.webkit.messageHandlers.eepviewHover.postMessage(href); } catch (e) {}
  };
  const linkOf = (node) => (node && node.closest ? node.closest("a[href], area[href]") : null);
  document.addEventListener("mouseover", (e) => {
    const a = linkOf(e.target);
    post(a ? String(a.href) : "");
  }, true);
  document.addEventListener("mouseout", (e) => { if (!e.relatedTarget) post(""); }, true);
})();"#;

/// Counts the matches of a query in the visible text. Runs in the private world.
#[must_use]
pub fn count_script(query: &str, case_sensitive: bool) -> String {
    let literal = js_string(query);
    let fold = if case_sensitive { "" } else { ".toLowerCase()" };
    format!(
        "(() => {{ const q = {literal}{fold}; \
         const t = (document.body ? document.body.innerText : ''){fold}; \
         let n = 0, i = 0; \
         while (q && (i = t.indexOf(q, i)) !== -1) {{ n++; i += q.length; }} \
         return n; }})()"
    )
}

/// The script that clears the find selection.
pub const CLEAR_SELECTION_SCRIPT: &str =
    "window.getSelection() && window.getSelection().removeAllRanges()";

/// A JavaScript string literal for `text`: quotes, backslashes, controls and the line
/// separators escaped.
#[must_use]
pub fn js_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '<' => out.push_str("\\u003c"),
            c if c.is_control() || c == '\u{2028}' || c == '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The hover text from a message: `None` for an empty one (the mouse left the link).
#[must_use]
pub fn hover_target(message: &str) -> Option<String> {
    let trimmed = message.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Attaches the request rules before the first load, then calls `done`. On an error the
/// caller must not load anything (fail closed).
pub fn attach_rules(
    webview: &PlatformWebview,
    rules: Rules<'_>,
    done: Box<dyn FnOnce(Result<(), String>)>,
) {
    imp::attach_rules(webview, rules, done);
}

/// Calls `callback` with the link under the mouse, or `None` when it leaves the link.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn on_hover(
    webview: &PlatformWebview,
    callback: Box<dyn Fn(Option<String>)>,
) -> Result<(), String> {
    imp::on_hover(webview, callback)
}

/// A step on the engine's own navigation list.
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn go(webview: &PlatformWebview, nav: Nav) -> Result<(), String> {
    imp::go(webview, nav)
}

/// Finds text. For a fresh search `on_count` gets the match count. Returns false when the
/// engine has no native find (the caller falls back to a script).
#[must_use]
pub fn find(
    webview: &PlatformWebview,
    request: &FindRequest,
    on_count: Box<dyn Fn(Option<u32>)>,
) -> bool {
    imp::find(webview, request, on_count)
}

/// Ends a find: clears the highlight.
pub fn find_clear(webview: &PlatformWebview) {
    imp::find_clear(webview);
}

/// Turns off engine features that call home or leak (see the table above).
///
/// # Errors
///
/// Fails when the engine handle is missing or the engine refuses the call.
pub fn harden(webview: &PlatformWebview) -> Result<(), String> {
    imp::harden(webview)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_string_escapes() {
        assert_eq!(js_string("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(js_string("</script>"), r#""\u003c/script>""#);
        assert_eq!(js_string("x\ny\u{2028}"), r#""x\u000ay\u2028""#);
    }

    #[test]
    fn count_script_embeds_the_query() {
        let s = count_script("Hi\"", false);
        assert!(s.contains(r#"const q = "Hi\"".toLowerCase();"#));
        assert!(!count_script("x", true).contains("toLowerCase"));
    }

    #[test]
    fn hover_targets() {
        assert_eq!(hover_target(""), None);
        assert_eq!(hover_target("  "), None);
        assert_eq!(
            hover_target("http://a.i2p/").as_deref(),
            Some("http://a.i2p/")
        );
    }

    #[test]
    fn js_string_keeps_plain_text_and_escapes_every_separator() {
        assert_eq!(js_string(""), r#""""#);
        assert_eq!(js_string("a.i2p/é"), r#""a.i2p/é""#);
        assert_eq!(js_string("\t\u{2029}\u{7f}"), r#""\u0009\u2029\u007f""#);
    }

    #[test]
    fn count_script_folds_case_on_both_sides() {
        let s = count_script("A", false);
        assert_eq!(s.matches(".toLowerCase()").count(), 2);
        assert!(count_script("", true).contains(r#"const q = "";"#));
    }

    #[test]
    fn clear_selection_script_removes_ranges() {
        assert!(CLEAR_SELECTION_SCRIPT.contains("removeAllRanges"));
    }

    #[test]
    fn hover_targets_are_trimmed() {
        assert_eq!(
            hover_target(" http://a.i2p/ \n").as_deref(),
            Some("http://a.i2p/")
        );
    }

    #[test]
    fn hover_script_uses_the_channel() {
        assert!(HOVER_SCRIPT.contains(HOVER_CHANNEL));
        assert!(HOVER_SCRIPT.contains("mouseover"));
    }
}
