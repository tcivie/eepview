// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Fuzz target for the gatekeeper request parser, [`eepview_lib::net::http`].
//!
//! Properties from docs/wiki/no-leak-architecture.md (layer L1): parsing and planning never
//! panic, the gatekeeper forwards, tunnels or relays an I2P host only, and the request that it
//! sends to the router names that host once and holds no bare CR or LF inside a line.

#![no_main]

use eepview_lib::net::host::is_i2p_host;
use eepview_lib::net::http::{Head, Plan, head_end, plan, plan_inner, upstream_request};
use libfuzzer_sys::fuzz_target;

/// The host of a tunnel the gatekeeper has already accepted for `CONNECT`.
const TUNNEL_HOST: &str = "stats.i2p";

/// A forwarded host is `host` or `host:port`; a port never contains a second colon.
fn assert_i2p(host: &str) {
    let name = host.split(':').next().unwrap_or_default();
    assert!(is_i2p_host(name), "forwarded a non-I2P host: {host:?}");
}

/// The bytes sent to the router: no line holds a bare CR or LF, and the one `Host` header (a
/// line after the request line) is the planned host.
fn assert_upstream(head: &Head, method: &str, host: &str, path: &str) {
    let bytes = upstream_request(method, host, path, head);
    let text = String::from_utf8(bytes).unwrap_or_default();
    assert!(!text.is_empty(), "the upstream request is not UTF-8");
    let mut hosts = Vec::new();
    for (index, line) in text.split("\r\n").enumerate() {
        assert!(
            !line.contains(['\r', '\n']),
            "bare CR or LF in a line: {text:?}"
        );
        match line.split_once(':').filter(|_| index > 0) {
            Some((name, value)) if name.trim().eq_ignore_ascii_case("host") => {
                hosts.push(value.trim());
            }
            _ => {}
        }
    }
    assert_eq!(hosts, [host], "wrong Host headers: {text:?}");
}

fn assert_plan(head: &Head, plan: &Plan) {
    match plan {
        Plan::Http { method, host, path } => {
            assert_i2p(host);
            assert_upstream(head, method, host, path);
        }
        Plan::Terminate { host, .. } | Plan::Relay { host } => {
            assert!(is_i2p_host(host), "{host:?}");
        }
        Plan::Refuse(_) => {}
    }
}

fuzz_target!(|data: &[u8]| {
    let _ = head_end(data);
    let Some(head) = Head::parse(data) else {
        return;
    };
    let _ = head.body_length();
    assert_plan(&head, &plan(&head, true));
    assert_plan(&head, &plan(&head, false));
    assert_plan(&head, &plan_inner(&head, TUNNEL_HOST, 80));
    assert_plan(&head, &plan_inner(&head, TUNNEL_HOST, 8080));
});
