//! The engine request rules (ADR 0001, layer L3). Pure: builds the policy text only.
//!
//! Two forms of one rule set, "block everything, then allow `.i2p`":
//!
//! - [`csp_header`]: a `Content-Security-Policy` that the gatekeeper adds to every response.
//!   Every engine enforces it inside the page, for every request kind, with JS on or off.
//!   On macOS this is what stops the `WKWebView` loopback bypass of the proxy (spike S1): an
//!   `<img>`, `<iframe>`, `fetch` or `WebSocket` to `127.0.0.1` never leaves the engine.
//! - [`content_rule_list`]: the same rule set as `WKContentRuleList` JSON, for an engine-level
//!   filter once a platform bridge may attach it (see ADR 0001, "Implementation notes").

use serde_json::{Value, json};

/// The allow pattern of ADR 0001: an `http(s)` URL on a `.i2p` host.
pub const I2P_URL_PATTERN: &str = r"^https?://([a-z0-9-]+\.)*[a-z0-9-]+\.i2p(:[0-9]+)?(/|$)";

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
/// I2P URLs. The `WebKit` regex subset has no look-ahead, so it takes two rules.
#[must_use]
pub fn content_rule_list() -> Value {
    json!([
        { "trigger": { "url-filter": ".*" }, "action": { "type": "block" } },
        {
            "trigger": { "url-filter": I2P_URL_PATTERN, "url-filter-is-case-sensitive": true },
            "action": { "type": "ignore-previous-rules" }
        }
    ])
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

    #[test]
    fn rule_list_blocks_then_allows_i2p() {
        let rules = content_rule_list();
        let list = rules.as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["action"]["type"], "block");
        assert_eq!(list[0]["trigger"]["url-filter"], ".*");
        assert_eq!(list[1]["action"]["type"], "ignore-previous-rules");
        assert_eq!(list[1]["trigger"]["url-filter"], I2P_URL_PATTERN);
    }
}
