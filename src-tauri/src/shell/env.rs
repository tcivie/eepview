//! Process inputs: environment variables and command-line URLs.
//!
//! - `EEPVIEW_PROXY`: the router HTTP proxy, a loopback `host:port` (default `127.0.0.1:4444`).
//!   It still has to pass VERIFY, and pages still go through the gatekeeper.
//! - `EEPVIEW_START_URL`: a URL to open at start, through the `navigate` rules.
//! - `EEPVIEW_EXIT_AFTER`: quit after this many seconds with code 0 (tests).
//! - `EEPVIEW_JS=off`: page JavaScript off for every site (default on).
//! - `EEPVIEW_ROUTER_STATUS` + `EEPVIEW_ROUTER_STATUS_TOKEN`: the router helper status
//!   endpoint and the path of its token file.

use std::env;

use crate::net::loopback::LoopbackAddr;

/// The default router proxy.
pub const DEFAULT_PROXY: &str = "127.0.0.1:4444";

/// The router proxy address, or why `EEPVIEW_PROXY` is not usable.
///
/// # Errors
///
/// Fails when the value is not a loopback `host:port`.
pub fn proxy() -> Result<LoopbackAddr, String> {
    let text = env::var("EEPVIEW_PROXY").unwrap_or_else(|_| DEFAULT_PROXY.to_owned());
    LoopbackAddr::parse(&text).map_err(|e| format!("EEPVIEW_PROXY: {e}"))
}

/// The proxy text for the status line.
#[must_use]
pub fn proxy_text() -> String {
    env::var("EEPVIEW_PROXY").unwrap_or_else(|_| DEFAULT_PROXY.to_owned())
}

/// URLs to open at start: `EEPVIEW_START_URL` first, then the command-line arguments.
#[must_use]
pub fn start_urls() -> Vec<String> {
    let mut urls: Vec<String> = env::var("EEPVIEW_START_URL").into_iter().collect();
    urls.extend(env::args().skip(1).filter(|a| !a.starts_with('-')));
    urls
}

/// Seconds until the app quits by itself.
#[must_use]
pub fn exit_after() -> Option<u64> {
    env::var("EEPVIEW_EXIT_AFTER").ok()?.trim().parse().ok()
}

/// The router helper address and token, when both are set and readable.
#[must_use]
pub fn router_helper() -> Option<(LoopbackAddr, String)> {
    let addr = LoopbackAddr::parse(&env::var("EEPVIEW_ROUTER_STATUS").ok()?).ok()?;
    let token = std::fs::read_to_string(env::var("EEPVIEW_ROUTER_STATUS_TOKEN").ok()?).ok()?;
    Some((addr, token.trim().to_owned()))
}

/// True when `EEPVIEW_JS=off`.
#[must_use]
pub fn js_forced_off() -> bool {
    env::var("EEPVIEW_JS").is_ok_and(|v| v.eq_ignore_ascii_case("off"))
}
