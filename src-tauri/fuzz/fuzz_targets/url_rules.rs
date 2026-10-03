// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Fuzz target for the L3 request rule and the L4 navigation guard.
//!
//! Property from docs/wiki/no-leak-architecture.md: for any URL, the engine rule
//! ([`eepview_lib::net::rules::engine_allows`]), the navigation guard
//! ([`eepview_lib::nav::guard`]) and the text check ([`eepview_lib::nav::is_allowed`]) allow an
//! `http(s)` URL only when its host passes [`eepview_lib::net::host::is_i2p_host`], and they
//! agree with each other.

#![no_main]

use eepview_lib::nav::{guard, host_of, is_allowed};
use eepview_lib::net::host::is_i2p_host;
use eepview_lib::net::rules::engine_allows;
use libfuzzer_sys::fuzz_target;
use url::Url;

fn is_web(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn assert_agree(text: &str, url: &Url) {
    let host_ok = url.host_str().is_some_and(is_i2p_host);
    let guarded = guard(url);
    let engine = engine_allows(text);
    if guarded && is_web(url) {
        assert!(host_ok, "the guard let a non-I2P host through: {text:?}");
        assert!(
            url.username().is_empty() && url.password().is_none(),
            "user info: {text:?}"
        );
    }
    if is_web(url) {
        assert_eq!(guarded, engine, "L3 and L4 disagree: {text:?}");
        assert_eq!(
            guarded,
            is_allowed(text),
            "guard and is_allowed disagree: {text:?}"
        );
        assert!(
            host_ok || !engine,
            "the engine rule let a non-I2P host through: {text:?}"
        );
    }
    if let Some(host) = host_of(text) {
        assert!(
            is_i2p_host(&host),
            "host_of returned a non-I2P host: {text:?}"
        );
    }
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
    assert_agree(text, &url);
});
