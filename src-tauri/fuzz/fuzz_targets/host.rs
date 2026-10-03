// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Fuzz target for [`eepview_lib::net::host::is_i2p_host`].
//!
//! Properties from the doc comment of the predicate: it never panics, and an accepted host is
//! a normalised name that ends in `.i2p`: lower-case ASCII, no empty label, no port, no user
//! info and no IDN label.

#![no_main]

use eepview_lib::net::host::is_i2p_host;
use libfuzzer_sys::fuzz_target;

fn assert_normalised(host: &str) {
    assert!(
        host.ends_with(".i2p"),
        "accepted host must end in .i2p: {host:?}"
    );
    assert!(host.is_ascii(), "accepted host must be ASCII: {host:?}");
    assert!(
        !host.bytes().any(|b| b.is_ascii_uppercase()),
        "accepted host must be lower-case: {host:?}"
    );
    assert!(
        !host.contains([':', '@', ' ', '/']),
        "accepted host has no port or user info: {host:?}"
    );
    assert!(
        host.split('.')
            .all(|label| !label.is_empty() && !label.starts_with("xn--")),
        "accepted host has no empty and no IDN label: {host:?}"
    );
}

fuzz_target!(|host: &str| {
    if is_i2p_host(host) {
        assert_normalised(host);
    }
});
