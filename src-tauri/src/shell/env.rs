// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

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
    proxy_from(var("EEPVIEW_PROXY"))
}

/// The router proxy address for an `EEPVIEW_PROXY` value.
///
/// # Errors
///
/// Fails when the value is not a loopback `host:port`.
pub fn proxy_from(value: Option<String>) -> Result<LoopbackAddr, String> {
    LoopbackAddr::parse(&proxy_text_from(value)).map_err(|e| format!("EEPVIEW_PROXY: {e}"))
}

/// The proxy text for the status line.
#[must_use]
pub fn proxy_text() -> String {
    proxy_text_from(var("EEPVIEW_PROXY"))
}

/// The proxy text for an `EEPVIEW_PROXY` value: the value, or the default.
#[must_use]
pub fn proxy_text_from(value: Option<String>) -> String {
    value.unwrap_or_else(|| DEFAULT_PROXY.to_owned())
}

/// URLs to open at start: `EEPVIEW_START_URL` first, then the command-line arguments.
#[must_use]
pub fn start_urls() -> Vec<String> {
    start_urls_from(var("EEPVIEW_START_URL"), env::args().skip(1))
}

/// URLs to open at start: `start` first, then the arguments that are not flags.
#[must_use]
pub fn start_urls_from(start: Option<String>, args: impl Iterator<Item = String>) -> Vec<String> {
    let mut urls: Vec<String> = start.into_iter().collect();
    urls.extend(args.filter(|a| !a.starts_with('-')));
    urls
}

/// Seconds until the app quits by itself.
#[must_use]
pub fn exit_after() -> Option<u64> {
    exit_after_from(var("EEPVIEW_EXIT_AFTER"))
}

/// Seconds until the app quits by itself, for an `EEPVIEW_EXIT_AFTER` value.
#[must_use]
pub fn exit_after_from(value: Option<String>) -> Option<u64> {
    value?.trim().parse().ok()
}

/// The router helper address and token, when both are set and readable.
#[must_use]
pub fn router_helper() -> Option<(LoopbackAddr, String)> {
    router_helper_from(
        var("EEPVIEW_ROUTER_STATUS"),
        var("EEPVIEW_ROUTER_STATUS_TOKEN"),
    )
}

/// The router helper for an address and the path of its token file.
#[must_use]
pub fn router_helper_from(
    addr: Option<String>,
    token_file: Option<String>,
) -> Option<(LoopbackAddr, String)> {
    let addr = LoopbackAddr::parse(&addr?).ok()?;
    let token = std::fs::read_to_string(token_file?).ok()?;
    Some((addr, token.trim().to_owned()))
}

/// True when `EEPVIEW_JS=off`.
#[must_use]
pub fn js_forced_off() -> bool {
    js_forced_off_from(var("EEPVIEW_JS"))
}

/// True for an `EEPVIEW_JS` value of `off`, in any case.
#[must_use]
pub fn js_forced_off_from(value: Option<String>) -> bool {
    value.is_some_and(|v| v.eq_ignore_ascii_case("off"))
}

/// A variable, or `None` when it is unset or not Unicode.
fn var(name: &str) -> Option<String> {
    env::var(name).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_defaults_to_the_router_port() {
        assert_eq!(proxy_text_from(None), DEFAULT_PROXY);
        assert_eq!(proxy_from(None).unwrap().to_string(), DEFAULT_PROXY);
        assert_eq!(proxy_text_from(Some("127.0.0.1:1".into())), "127.0.0.1:1");
    }

    #[test]
    fn proxy_must_be_loopback() {
        assert_eq!(
            proxy_from(Some("127.0.0.1:4445".into()))
                .unwrap()
                .to_string(),
            "127.0.0.1:4445"
        );
        let error = proxy_from(Some("10.0.0.1:4444".into())).unwrap_err();
        assert!(error.starts_with("EEPVIEW_PROXY: "), "{error}");
    }

    #[test]
    fn start_urls_skip_flags() {
        let args = ["-x", "http://b.i2p/", "--y"].map(String::from).into_iter();
        assert_eq!(
            start_urls_from(Some("http://a.i2p/".into()), args),
            ["http://a.i2p/", "http://b.i2p/"]
        );
        assert!(start_urls_from(None, std::iter::empty()).is_empty());
    }

    #[test]
    fn exit_after_parses_seconds() {
        assert_eq!(exit_after_from(Some(" 3 ".into())), Some(3));
        assert_eq!(exit_after_from(Some("soon".into())), None);
        assert_eq!(exit_after_from(None), None);
    }

    #[test]
    fn js_off_only_for_off() {
        assert!(js_forced_off_from(Some("OFF".into())));
        assert!(!js_forced_off_from(Some("on".into())));
        assert!(!js_forced_off_from(None));
    }

    #[test]
    fn router_helper_needs_both_parts() {
        let file = std::env::temp_dir().join(format!("eepview-token-{}", std::process::id()));
        std::fs::write(&file, " secret\n").unwrap();
        let path = Some(file.display().to_string());
        let (addr, token) =
            router_helper_from(Some("127.0.0.1:7657".into()), path.clone()).unwrap();
        assert_eq!(
            (addr.to_string().as_str(), token.as_str()),
            ("127.0.0.1:7657", "secret")
        );
        assert!(router_helper_from(None, path.clone()).is_none());
        assert!(router_helper_from(Some("example.com:1".into()), path.clone()).is_none());
        assert!(router_helper_from(Some("127.0.0.1:7657".into()), None).is_none());
        assert!(
            router_helper_from(
                Some("127.0.0.1:7657".into()),
                Some("/nonexistent/eepview".into())
            )
            .is_none()
        );
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn process_inputs_read_the_environment() {
        assert_eq!(proxy_text(), proxy_text_from(var("EEPVIEW_PROXY")));
        assert_eq!(proxy().is_ok(), proxy_from(var("EEPVIEW_PROXY")).is_ok());
        assert_eq!(exit_after(), exit_after_from(var("EEPVIEW_EXIT_AFTER")));
        assert_eq!(js_forced_off(), js_forced_off_from(var("EEPVIEW_JS")));
        assert_eq!(
            router_helper().is_some(),
            router_helper_from(
                var("EEPVIEW_ROUTER_STATUS"),
                var("EEPVIEW_ROUTER_STATUS_TOKEN"),
            )
            .is_some()
        );
        assert!(start_urls().iter().all(|u| !u.starts_with('-')));
    }
}
