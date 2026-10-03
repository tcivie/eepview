// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Fuzz target for the address-bar parser, [`eepview_lib::nav::classify`].
//!
//! Properties from docs/wiki/browser-shell.md: any input is classified without a panic; a web
//! target is an `http(s)` URL on an I2P host; an internal target is a known bundled page; a
//! search is one word with no dot and no colon.

#![no_main]

use eepview_lib::nav::{INTERNAL_PAGES, INTERNAL_SCHEME, Target, classify};
use eepview_lib::net::host::is_i2p_host;
use libfuzzer_sys::fuzz_target;
use url::Url;

fn assert_web(url: &Url) {
    assert!(matches!(url.scheme(), "http" | "https"), "scheme: {url}");
    assert!(
        url.host_str().is_some_and(is_i2p_host),
        "not an I2P host: {url}"
    );
    assert!(
        url.username().is_empty() && url.password().is_none(),
        "user info: {url}"
    );
}

fn assert_internal(page: &str) {
    let rest = page.strip_prefix(&format!("{INTERNAL_SCHEME}://"));
    let name = rest.and_then(|r| r.split('?').next());
    assert!(
        name.is_some_and(|n| INTERNAL_PAGES.contains(&n)),
        "unknown page: {page}"
    );
}

fn assert_search(text: &str) {
    assert!(!text.is_empty(), "empty search");
    assert!(
        !text.chars().any(char::is_whitespace),
        "search has a space: {text:?}"
    );
    assert!(
        !text.contains(['.', ':']),
        "search has a dot or colon: {text:?}"
    );
}

fuzz_target!(|input: &str| {
    match classify(input) {
        Target::Web(url) => assert_web(&url),
        Target::Internal(page) => assert_internal(&page),
        Target::Search(text) => assert_search(&text),
        Target::Refused(reason) => assert!(matches!(reason.as_str(), "not-i2p" | "invalid")),
    }
});
