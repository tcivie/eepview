// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The engine request rules (ADR 0001, layer L3). Pure: builds the policy text only.
//!
//! Two forms of one rule set, "block everything, then allow `.i2p`":
//!
//! - [`csp_header`]: a `Content-Security-Policy` that the gatekeeper adds to every response.
//!   Every engine enforces it inside the page, for every request kind, with JS on or off.
//!   On macOS this is what stops the `WKWebView` loopback bypass of the proxy (spike S1): an
//!   `<img>`, `<iframe>`, `fetch` or `WebSocket` to `127.0.0.1` never leaves the engine.
//! - [`content_rule_list`]: the same rule set as `WKContentRuleList` JSON, for an engine-level
//!   filter (L3b). The `eepview-platform` bridge attaches it to every content webview before
//!   its first load. [`engine_allows`] is the same rule for the `WebView2` request filter.

use serde_json::{Value, json};
use tauri::Url;

use crate::nav;

/// The identifier of the compiled content rule list (macOS).
pub const CONTENT_RULES_ID: &str = "eepview-i2p-only";

/// URLs that never touch the network: the blank page and inline data. One rule each: the
/// `WebKit` regex subset has no `|`.
pub const LOCAL_URL_PATTERNS: [&str; 3] = ["^about:", "^data:", "^blob:"];

/// The allow pattern of ADR 0001: an `http(s)` URL on a `.i2p` host. `WebKit` hands the
/// filter normalized URLs, so the path always starts with `/` after the host.
pub const I2P_URL_PATTERN: &str = r"^https?://([a-z0-9-]+\.)*[a-z0-9-]+\.i2p(:[0-9]+)?/";

/// The refusals that follow the allow pattern, in order. `WebKit` rule regexes have no look-ahead
/// and no `|`, so one pattern cannot say "an I2P host, but not ...". Each entry is a rule that
/// blocks again (`false`) or allows again (`true`) what [`I2P_URL_PATTERN`] let through, and
/// together they accept exactly the hosts of [`crate::net::host::is_i2p_host`] on a port other
/// than 0.
pub const I2P_URL_EXCEPTIONS: [(&str, bool); 4] = [
    // An IDN label (`xn--`).
    (
        r"^https?://([a-z0-9-]+\.)*xn--[a-z0-9-]*(\.[a-z0-9-]+)*\.i2p(:[0-9]+)?/",
        false,
    ),
    // Every `b32.i2p` host: the bare name and every `*.b32.i2p` name ...
    (r"^https?://([a-z0-9-]+\.)*b32\.i2p(:[0-9]+)?/", false),
    // ... then the one form that is valid: a single base32 label of 52 characters or more.
    (
        concat!(
            "^https?://",
            "[a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7]",
            "[a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7]",
            "[a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7]",
            "[a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7]",
            "[a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7]",
            "[a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7][a-z2-7]",
            "[a-z2-7][a-z2-7][a-z2-7][a-z2-7]",
            "[a-z2-7]*\\.b32\\.i2p(:[0-9]+)?/"
        ),
        true,
    ),
    // Port 0.
    (r"^https?://[a-z0-9.-]+:0/", false),
];

/// Sources a page may use: I2P sites over http(s), any port.
const I2P_SOURCES: &str = "http://*.i2p:* https://*.i2p:*";

/// The `Content-Security-Policy` value the gatekeeper adds to every response.
///
/// `data:` and `blob:` stay allowed for images, fonts and media, which never touch the
/// network; frames, forms and workers may only use I2P sources.
#[must_use]
pub fn csp_header() -> &'static str {
    concat!(
        "default-src http://*.i2p:* https://*.i2p:* 'unsafe-inline' 'unsafe-eval' data: blob:; ",
        "connect-src http://*.i2p:* https://*.i2p:*; ",
        "frame-src http://*.i2p:* https://*.i2p:*; ",
        "worker-src http://*.i2p:* https://*.i2p:*; ",
        "form-action http://*.i2p:* https://*.i2p:*; ",
        "object-src http://*.i2p:* https://*.i2p:*"
    )
}

/// The rule set as `WKContentRuleList` JSON: block every URL, then ignore that rule for
/// I2P and local URLs, then block again what [`is_i2p_host`](crate::net::host::is_i2p_host) and
/// the guard refuse ([`I2P_URL_EXCEPTIONS`]). The `WebKit` regex subset has no look-ahead and no
/// `|`, so each form is a rule of its own, and the order of the rules matters.
#[must_use]
pub fn content_rule_list() -> Value {
    let mut rules = vec![
        json!({ "trigger": { "url-filter": ".*" }, "action": { "type": "block" } }),
        json!({
            "trigger": { "url-filter": I2P_URL_PATTERN, "url-filter-is-case-sensitive": true },
            "action": { "type": "ignore-previous-rules" }
        }),
    ];
    rules.extend(I2P_URL_EXCEPTIONS.iter().map(|(pattern, allow)| {
        let action = if *allow {
            "ignore-previous-rules"
        } else {
            "block"
        };
        json!({
            "trigger": { "url-filter": pattern, "url-filter-is-case-sensitive": true },
            "action": { "type": action }
        })
    }));
    rules.extend(LOCAL_URL_PATTERNS.iter().map(|pattern| {
        json!({
            "trigger": { "url-filter": pattern },
            "action": { "type": "ignore-previous-rules" }
        })
    }));
    Value::Array(rules)
}

/// True for a request URL the engine may load: an I2P URL or a local one (`about:`,
/// `data:`, `blob:`). The `WebView2` request filter asks this for every request.
#[must_use]
pub fn engine_allows(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| nav::guard(&u) || matches!(u.scheme(), "about" | "data" | "blob"))
}

/// The I2P sources of the policy, for tests and docs.
#[must_use]
pub fn i2p_sources() -> &'static str {
    I2P_SOURCES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_limits_every_fetch_directive_to_i2p() {
        let csp = csp_header();
        for directive in [
            "default-src",
            "connect-src",
            "frame-src",
            "form-action",
            "worker-src",
        ] {
            let part = csp
                .split(';')
                .map(str::trim)
                .find(|p| p.starts_with(directive))
                .unwrap();
            assert!(part.contains(i2p_sources()), "{directive}");
            assert!(
                !part.contains(" *;") && !part.contains("http: "),
                "{directive}"
            );
        }
        assert!(!csp.contains("127.0.0.1") && !csp.contains("localhost"));
    }

    #[test]
    fn frames_cannot_use_data_or_blob() {
        let frame = csp_header()
            .split(';')
            .find(|p| p.contains("frame-src"))
            .unwrap();
        assert!(!frame.contains("data:") && !frame.contains("blob:"));
    }

    fn assert_rule(rule: &Value, pattern: &str, allow: bool) {
        let action = if allow {
            "ignore-previous-rules"
        } else {
            "block"
        };
        assert_eq!(rule["action"]["type"], action, "{pattern}");
        assert_eq!(rule["trigger"]["url-filter"], pattern);
    }

    #[test]
    fn rule_list_blocks_then_allows_i2p() {
        let rules = content_rule_list();
        let list = rules.as_array().unwrap();
        assert_eq!(list.len(), 5 + I2P_URL_EXCEPTIONS.len());
        assert_rule(&list[0], ".*", false);
        assert_rule(&list[1], I2P_URL_PATTERN, true);
        let (exceptions, local) = list[2..].split_at(I2P_URL_EXCEPTIONS.len());
        for (rule, (pattern, allow)) in exceptions.iter().zip(I2P_URL_EXCEPTIONS) {
            assert_rule(rule, pattern, allow);
        }
        for (rule, pattern) in local.iter().zip(LOCAL_URL_PATTERNS) {
            assert_rule(rule, pattern, true);
        }
        assert!(
            !rules.to_string().contains('|'),
            "WebKit rule regexes have no disjunction"
        );
    }

    #[test]
    fn engine_allows_i2p_and_local_urls_only() {
        for url in [
            "http://stats.i2p/",
            "https://a.b.i2p:8443/x",
            "about:blank",
            "data:image/png;base64,AA",
            "blob:http://stats.i2p/1",
        ] {
            assert!(engine_allows(url), "{url}");
        }
        for url in [
            "http://127.0.0.1:7657/",
            "http://example.com/",
            "https://stats.i2p.example.com/",
            "file:///etc/passwd",
            "ws://stats.i2p/",
            "not a url",
        ] {
            assert!(!engine_allows(url), "{url}");
        }
    }
}
