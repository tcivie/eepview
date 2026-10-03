// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Fuzz target for the gatekeeper request parser, [`eepview_lib::net::http`].
//!
//! Properties from docs/wiki/no-leak-architecture.md (layer L1): parsing and planning never
//! panic, and the gatekeeper forwards, tunnels or relays an I2P host only.

#![no_main]

use eepview_lib::net::host::is_i2p_host;
use eepview_lib::net::http::{Head, Plan, head_end, plan, plan_inner};
use libfuzzer_sys::fuzz_target;

/// The host of a tunnel the gatekeeper has already accepted for `CONNECT`.
const TUNNEL_HOST: &str = "stats.i2p";

/// A forwarded host is `host` or `host:port`; a port never contains a second colon.
fn assert_i2p(host: &str) {
    let name = host.split(':').next().unwrap_or_default();
    assert!(is_i2p_host(name), "forwarded a non-I2P host: {host:?}");
}

fn assert_plan(plan: &Plan) {
    match plan {
        Plan::Http { host, .. } => assert_i2p(host),
        Plan::Terminate { host } | Plan::Relay { host } => assert!(is_i2p_host(host), "{host:?}"),
        Plan::Refuse(refusal) => assert!(!refusal.response().is_empty()),
    }
}

fn assert_inner(plan: &Plan) {
    if let Plan::Http { host, .. } = plan {
        assert_eq!(
            host, TUNNEL_HOST,
            "a tunnel request must keep the tunnel host"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let _ = head_end(data);
    let Some(head) = Head::parse(data) else {
        return;
    };
    let _ = head.body_length();
    assert_plan(&plan(&head, true));
    assert_plan(&plan(&head, false));
    assert_inner(&plan_inner(&head, TUNNEL_HOST));
});
