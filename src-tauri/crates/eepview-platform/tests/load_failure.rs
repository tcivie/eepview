// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the pure part of the failed-load bridge: which engine errors count as a
//! failed load (F2), which reason each one gets (F3) and the text of the code (F4). The source is
//! the "Failed loads" section of `docs/wiki/browser-shell.md`. The tests read the requirement and
//! the public interface only, never the implementation.

use eepview_platform::{FailReason, LoadFailure};

const NS: &str = "NSURLErrorDomain";
const WK: &str = "WebKitErrorDomain";

/// A failure of `domain` with `code` and no address.
fn failure(domain: &str, code: i64) -> LoadFailure {
    LoadFailure {
        domain: domain.to_owned(),
        code,
        url: None,
    }
}

/// The reason of a failure of `domain` with `code`.
fn reason(domain: &str, code: i64) -> Option<FailReason> {
    failure(domain, code).reason()
}

// ---------------------------------------------------------------------------------------------
// F2: a cancelled load is not a failure
// ---------------------------------------------------------------------------------------------

#[test]
fn f2_nsurl_cancelled_999_is_not_a_failure() {
    assert_eq!(reason(NS, -999), None);
}

#[test]
fn f2_webkit_frame_load_interrupted_102_is_not_a_failure() {
    assert_eq!(reason(WK, 102), None);
}

#[test]
fn f2_cancel_ignores_the_address() {
    let cancelled = LoadFailure {
        domain: NS.to_owned(),
        code: -999,
        url: Some("http://a.i2p/".to_owned()),
    };
    assert_eq!(cancelled.reason(), None);
}

#[test]
fn f2_999_in_another_domain_is_a_failure() {
    assert_eq!(reason("OtherDomain", -999), Some(FailReason::Engine));
}

#[test]
fn f2_102_in_the_nsurl_domain_is_a_failure() {
    assert_eq!(reason(NS, 102), Some(FailReason::Engine));
}

// ---------------------------------------------------------------------------------------------
// F3: the reason
// ---------------------------------------------------------------------------------------------

#[test]
fn f3_app_transport_security_1022_is_blocked() {
    assert_eq!(reason(NS, -1022), Some(FailReason::Blocked));
}

#[test]
fn f3_content_rule_list_104_is_blocked() {
    assert_eq!(reason(WK, 104), Some(FailReason::Blocked));
}

#[test]
fn f3_every_connection_error_is_unreachable() {
    let codes = [
        (-1001, "timed out"),
        (-1003, "host not found"),
        (-1004, "cannot connect"),
        (-1005, "connection lost"),
        (-1006, "DNS failed"),
        (-1009, "offline"),
    ];
    for (code, name) in codes {
        assert_eq!(
            reason(NS, code),
            Some(FailReason::Unreachable),
            "{name} ({code})"
        );
    }
}

#[test]
fn f3_an_unknown_nsurl_code_is_engine() {
    for code in [-1, -1000, -1002, -1007, -1100, -9999, 0, 1] {
        assert_eq!(reason(NS, code), Some(FailReason::Engine), "code {code}");
    }
}

#[test]
fn f3_an_unknown_webkit_code_is_engine() {
    for code in [0, 100, 101, 103, 105, 204] {
        assert_eq!(reason(WK, code), Some(FailReason::Engine), "code {code}");
    }
}

#[test]
fn f3_the_domain_matters_for_1022() {
    assert_eq!(reason("OtherDomain", -1022), Some(FailReason::Engine));
    assert_eq!(reason(WK, -1022), Some(FailReason::Engine));
    assert_eq!(reason("", -1022), Some(FailReason::Engine));
}

#[test]
fn f3_the_domain_matters_for_104_and_the_connection_codes() {
    assert_eq!(reason(NS, 104), Some(FailReason::Engine));
    assert_eq!(reason("OtherDomain", 104), Some(FailReason::Engine));
    assert_eq!(reason(WK, -1001), Some(FailReason::Engine));
    assert_eq!(reason("OtherDomain", -1009), Some(FailReason::Engine));
}

#[test]
fn f3_the_address_does_not_change_the_reason() {
    let with_url = LoadFailure {
        domain: NS.to_owned(),
        code: -1022,
        url: Some("http://example.com/".to_owned()),
    };
    assert_eq!(with_url.reason(), Some(FailReason::Blocked));
}

#[test]
fn f3_the_reason_words_are_blocked_unreachable_engine() {
    assert_eq!(FailReason::Blocked.as_str(), "blocked");
    assert_eq!(FailReason::Unreachable.as_str(), "unreachable");
    assert_eq!(FailReason::Engine.as_str(), "engine");
}

// ---------------------------------------------------------------------------------------------
// F4: the code
// ---------------------------------------------------------------------------------------------

#[test]
fn f4_the_code_is_the_domain_a_space_and_the_number() {
    assert_eq!(failure(NS, -1022).code_text(), "NSURLErrorDomain -1022");
}

#[test]
fn f4_a_positive_number_has_no_sign() {
    assert_eq!(failure(WK, 104).code_text(), "WebKitErrorDomain 104");
}

#[test]
fn f4_the_code_text_follows_any_domain_and_number() {
    assert_eq!(failure("Custom.Domain", 0).code_text(), "Custom.Domain 0");
    assert_eq!(failure(NS, -1009).code_text(), "NSURLErrorDomain -1009");
}
