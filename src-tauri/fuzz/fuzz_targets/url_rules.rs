// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Fuzz target for the L3 request rules and the L4 navigation guard.
//!
//! Properties from docs/wiki/no-leak-architecture.md: for any URL, the navigation guard
//! ([`eepview_lib::nav::guard`]) and the engine rule ([`eepview_lib::net::rules::engine_allows`])
//! allow an `http(s)` URL only when its host passes [`eepview_lib::net::host::is_i2p_host`], and
//! the `WebKit` rule list ([`eepview_lib::net::rules::content_rule_list`], layer L3b) allows the
//! same URLs as the guard.

#![no_main]

use std::sync::OnceLock;

use eepview_lib::nav::{guard, host_of, is_allowed};
use eepview_lib::net::host::is_i2p_host;
use eepview_lib::net::rules::{content_rule_list, engine_allows};
use libfuzzer_sys::fuzz_target;
use regex::Regex;
use url::Url;

/// One rule of the `WKContentRuleList`: its regex and whether it blocks.
struct Rule {
    filter: Regex,
    blocks: bool,
}

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let list = content_rule_list();
        let items = list.as_array().map(Vec::as_slice).unwrap_or_default();
        items
            .iter()
            .filter_map(|json| {
                let pattern = json["trigger"]["url-filter"].as_str()?;
                Some(Rule {
                    filter: Regex::new(pattern).ok()?,
                    blocks: json["action"]["type"] == "block",
                })
            })
            .collect()
    })
}

/// The rule list as `WebKit` runs it: a rule that blocks marks the URL blocked, and
/// `ignore-previous-rules` clears the mark.
fn list_allows(url: &str) -> bool {
    let step =
        |blocked: bool, rule: &Rule| [blocked, rule.blocks][usize::from(rule.filter.is_match(url))];
    !rules().iter().fold(false, step)
}

fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// The guard, the engine rule and the text check let an `http(s)` URL through only on an I2P
/// host with no user info.
fn assert_i2p_only(text: &str, url: &Url) {
    let host_ok = url.host_str().is_some_and(is_i2p_host);
    let clean = url.username().is_empty() && url.password().is_none();
    for allowed in [guard(url), engine_allows(text), is_allowed(text)] {
        assert!(
            !allowed || (host_ok && clean),
            "let a non-I2P URL through: {text:?}"
        );
    }
}

/// The `WebKit` rule list gets normalised URLs and must allow what the guard allows.
fn assert_pattern_agrees(text: &str, url: &Url) {
    assert_eq!(
        list_allows(url.as_str()),
        guard(url),
        "L3b rule list and L4 guard disagree: {text:?} as {url}"
    );
}

fuzz_target!(|text: &str| {
    // A text that does not parse is allowed nowhere.
    let Ok(url) = Url::parse(text) else {
        assert!(
            !engine_allows(text) && !is_allowed(text),
            "unparseable URL allowed: {text:?}"
        );
        return;
    };
    if is_web(&url) {
        assert_i2p_only(text, &url);
        assert_pattern_agrees(text, &url);
    }
    if let Some(host) = host_of(text) {
        assert!(
            is_i2p_host(&host),
            "host_of returned a non-I2P host: {text:?}"
        );
    }
});
