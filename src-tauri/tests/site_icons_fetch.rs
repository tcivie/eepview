// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Site icons, fetch side (R1 to R5, R11 to R16 of `docs/wiki/site-icons.md`).
//!
//! A scripted fake router sits behind a real gatekeeper. The tests count what the router
//! sees and check what `net::icons::fetch` returns. Expected values come from the
//! requirement text, not from the constants of the code.

#[path = "site_icons_support/router.rs"]
mod router;

use std::error::Error;
use std::time::{Duration, Instant};

use eepview_lib::net::icons::{FetchError, Limits, fetch, fetch_with, request};
use router::{Reply, Router};

type Res<T> = Result<T, Box<dyn Error>>;

const SITE: &str = "site.i2p";

/// An HTTP answer: status text, extra header lines, body.
fn http(status: &str, headers: &[&str], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\n");
    for header in headers {
        out.push_str(header);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

/// A 200 answer with a matching `Content-Length`.
fn ok(body: &[u8]) -> Vec<u8> {
    let length = format!("Content-Length: {}", body.len());
    http("200 OK", &[&length], body)
}

/// A router that sends `bytes` for every request, then closes (or stalls).
fn router_sending(bytes: Vec<u8>, stall: bool) -> Res<Router> {
    Router::start(move |_| Reply {
        bytes: bytes.clone(),
        stall,
    })
}

/// A fetch with a short timeout, so a stalled router ends the test fast.
fn quick() -> Limits {
    Limits {
        timeout: Duration::from_secs(5),
        max_body: 65_536,
    }
}

/// The names of the headers of one request head, lower case.
fn header_names(head: &str) -> Vec<String> {
    head.lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, _)| name.trim().to_ascii_lowercase())
        .collect()
}

#[test]
fn r1_the_only_request_is_get_favicon_ico_on_the_host() -> Res<()> {
    // R1: one GET /favicon.ico, even when the answer names other icons in HTML.
    let html = br#"<html><link rel="icon" href="/other.png"><link rel="shortcut icon" href="http://x.i2p/y.ico"></html>"#;
    let router = router_sending(ok(html), false)?;
    let gate = router.gatekeeper()?;
    let body = fetch(&gate, SITE)?;
    assert_eq!(body, html.to_vec());
    assert_eq!(router.connections(), 1, "R1: exactly one request");
    let heads = router.heads();
    let first = heads.first().ok_or("no request seen")?;
    assert_eq!(
        first.lines().next(),
        Some("GET http://site.i2p/favicon.ico HTTP/1.1")
    );
    Ok(())
}

#[test]
fn r2_a_closed_gatekeeper_means_no_request_reaches_the_router() -> Res<()> {
    // R2: the fetch goes through the gatekeeper only. With the gatekeeper closed, the
    // router must see nothing: the fetch has no other way to it.
    let router = router_sending(ok(b"x"), false)?;
    let gate = router.gatekeeper()?;
    gate.close();
    let before = router.connections();
    assert!(fetch_with(&gate, SITE, quick()).is_err(), "R2: no answer");
    assert_eq!(
        router.connections(),
        before,
        "R2: nothing reached the router"
    );
    assert!(router.heads().is_empty());
    Ok(())
}

#[test]
fn r3_the_request_head_is_exactly_four_lines() {
    // R3: the exact bytes, in this order.
    let want = b"GET http://site.i2p/favicon.ico HTTP/1.1\r\nHost: site.i2p\r\nAccept: image/*\r\nConnection: close\r\n\r\n";
    assert_eq!(request(SITE), Some(want.to_vec()));
    let other = b"GET http://alpha.i2p/favicon.ico HTTP/1.1\r\nHost: alpha.i2p\r\nAccept: image/*\r\nConnection: close\r\n\r\n";
    assert_eq!(request("alpha.i2p"), Some(other.to_vec()));
}

#[test]
fn r3_the_router_sees_no_cookie_referer_or_other_identifying_header() -> Res<()> {
    // R3: no Cookie, Referer, User-Agent, Origin or Accept-Encoding header.
    let router = router_sending(ok(b"x"), false)?;
    let gate = router.gatekeeper()?;
    fetch(&gate, SITE)?;
    let heads = router.heads();
    let head = heads.first().ok_or("no request seen")?;
    let names = header_names(head);
    for banned in [
        "cookie",
        "referer",
        "user-agent",
        "origin",
        "accept-encoding",
    ] {
        assert!(!names.iter().any(|n| n == banned), "R3: {banned} in {head}");
    }
    assert!(
        names.iter().any(|n| n == "host"),
        "R3: Host is sent: {head}"
    );
    assert!(head.contains("Host: site.i2p"), "{head}");
    assert!(head.contains("Accept: image/*"), "{head}");
    Ok(())
}

#[test]
fn r4_a_host_that_is_not_i2p_gets_no_request_and_no_connection() -> Res<()> {
    // R4: no connection is opened, not even to the gatekeeper's router.
    let router = router_sending(ok(b"x"), false)?;
    let gate = router.gatekeeper()?;
    let hosts = [
        "example.com",
        "localhost",
        "127.0.0.1",
        "10.0.0.1",
        "site.i2p:80",
        "user@site.i2p",
        "site.i2p/path",
        "xn--e1afmkfd.i2p",
        "a..i2p",
        "",
    ];
    for host in hosts {
        assert_eq!(request(host), None, "R4: request bytes for {host:?}");
        assert_eq!(fetch(&gate, host), Err(FetchError::NotI2p), "{host:?}");
    }
    assert_eq!(router.connections(), 0, "R4: no connection opened");
    Ok(())
}

#[test]
fn r4_an_i2p_host_gets_request_bytes() {
    // R4: the other side of the rule.
    assert!(request("site.i2p").is_some());
    assert!(request("stats.i2p").is_some());
}

#[test]
fn r5_a_redirect_is_not_followed() -> Res<()> {
    // R5: any 3xx is refused with its status; the router sees one request only.
    for code in [301_u16, 302, 303, 307, 308] {
        let location = "Location: http://other.i2p/favicon.ico";
        let answer = http(
            &format!("{code} Moved"),
            &[location, "Content-Length: 0"],
            b"",
        );
        let router = router_sending(answer, false)?;
        let gate = router.gatekeeper()?;
        assert_eq!(fetch(&gate, SITE), Err(FetchError::Status(code)));
        assert_eq!(router.connections(), 1, "R5: {code} was followed");
        assert!(router.heads().iter().all(|h| !h.contains("other.i2p")));
    }
    Ok(())
}

#[test]
fn r5_only_status_200_is_accepted() -> Res<()> {
    // R5: 204, 206, 404, 500 are refused with their status.
    for code in [204_u16, 206, 404, 500] {
        let router = router_sending(
            http(&format!("{code} X"), &["Content-Length: 1"], b"x"),
            false,
        )?;
        let gate = router.gatekeeper()?;
        assert_eq!(fetch(&gate, SITE), Err(FetchError::Status(code)));
    }
    let router = router_sending(ok(b"icon"), false)?;
    assert_eq!(fetch(&router.gatekeeper()?, SITE), Ok(b"icon".to_vec()));
    Ok(())
}

#[test]
fn r12_the_default_timeout_is_30_seconds_and_the_body_cap_64_kib() {
    // R12, R13: the default limits, from the requirement text.
    assert_eq!(Limits::DEFAULT.timeout, Duration::from_secs(30));
    assert_eq!(Limits::DEFAULT.max_body, 65_536);
}

#[test]
fn r12_a_silent_router_ends_the_fetch_with_timeout() -> Res<()> {
    // R12: nothing arrives; the fetch ends at the timeout.
    let router = router_sending(Vec::new(), true)?;
    let gate = router.gatekeeper()?;
    let limits = Limits {
        timeout: Duration::from_millis(600),
        max_body: 65_536,
    };
    let started = Instant::now();
    assert_eq!(fetch_with(&gate, SITE, limits), Err(FetchError::Timeout));
    assert!(started.elapsed() < Duration::from_secs(10), "R12: too slow");
    Ok(())
}

#[test]
fn r12_a_fetch_that_times_out_returns_no_body_even_when_bytes_arrived() -> Res<()> {
    // R12: the head and half of the body arrive, then silence. No partial body is returned.
    let partial = http("200 OK", &["Content-Length: 100"], &[7_u8; 50]);
    let router = router_sending(partial, true)?;
    let gate = router.gatekeeper()?;
    let limits = Limits {
        timeout: Duration::from_millis(600),
        max_body: 65_536,
    };
    assert_eq!(fetch_with(&gate, SITE, limits), Err(FetchError::Timeout));
    Ok(())
}

#[test]
fn r13_a_body_of_exactly_64_kib_is_accepted() -> Res<()> {
    // R13: at most 65 536 bytes.
    let body = vec![9_u8; 65_536];
    let router = router_sending(ok(&body), false)?;
    let got = fetch(&router.gatekeeper()?, SITE)?;
    assert_eq!(got.len(), 65_536);
    Ok(())
}

#[test]
fn r13_a_larger_content_length_is_refused_before_the_body_is_read() -> Res<()> {
    // R13: the head says 65 537 bytes; the body never comes. The answer is TooLarge,
    // not Timeout, so the fetch did not wait for the body.
    let head = http("200 OK", &["Content-Length: 65537"], b"");
    let router = router_sending(head, true)?;
    let gate = router.gatekeeper()?;
    let started = Instant::now();
    assert_eq!(fetch_with(&gate, SITE, quick()), Err(FetchError::TooLarge));
    assert!(started.elapsed() < Duration::from_secs(4), "R13: it waited");
    Ok(())
}

#[test]
fn r13_a_body_that_grows_past_the_limit_is_refused() -> Res<()> {
    // R13: no Content-Length; the router streams 70 000 bytes and closes.
    let answer = http("200 OK", &[], &vec![1_u8; 70_000]);
    let router = router_sending(answer, false)?;
    let gate = router.gatekeeper()?;
    assert_eq!(fetch_with(&gate, SITE, quick()), Err(FetchError::TooLarge));
    Ok(())
}

#[test]
fn r13_a_small_limit_in_limits_is_honoured() -> Res<()> {
    // R13: `Limits::max_body` is the cap.
    let router = router_sending(ok(&[3_u8; 200]), false)?;
    let gate = router.gatekeeper()?;
    let limits = Limits {
        timeout: Duration::from_secs(5),
        max_body: 100,
    };
    assert_eq!(fetch_with(&gate, SITE, limits), Err(FetchError::TooLarge));
    Ok(())
}

#[test]
fn r14_a_transfer_encoding_header_is_refused() -> Res<()> {
    // R14: chunked, in any letter case, and any other transfer coding.
    let chunked = b"5\r\nhello\r\n0\r\n\r\n";
    for header in [
        "Transfer-Encoding: chunked",
        "transfer-encoding: chunked",
        "Transfer-Encoding: gzip",
    ] {
        let router = router_sending(http("200 OK", &[header], chunked), false)?;
        let gate = router.gatekeeper()?;
        assert_eq!(fetch(&gate, SITE), Err(FetchError::Chunked), "{header}");
    }
    Ok(())
}

#[test]
fn r14_a_body_shorter_than_its_content_length_is_refused() -> Res<()> {
    // R14: the head says 100 bytes, 10 arrive, the router closes.
    let answer = http("200 OK", &["Content-Length: 100"], &[5_u8; 10]);
    let router = router_sending(answer, false)?;
    let gate = router.gatekeeper()?;
    assert_eq!(fetch_with(&gate, SITE, quick()), Err(FetchError::Malformed));
    Ok(())
}

#[test]
fn r15_a_response_head_larger_than_64_kib_is_refused() -> Res<()> {
    // R15: a 70 000 byte header line.
    let pad = format!("X-Pad: {}", "a".repeat(70_000));
    let answer = http("200 OK", &[&pad, "Content-Length: 1"], b"x");
    let router = router_sending(answer, false)?;
    let gate = router.gatekeeper()?;
    // The gatekeeper may refuse the head itself (its own 502) before `fetch` sees it.
    let got = fetch_with(&gate, SITE, quick());
    assert!(
        matches!(got, Err(FetchError::TooLarge | FetchError::Status(502))),
        "R15: {got:?}"
    );
    Ok(())
}

#[test]
fn r15_a_response_that_is_not_http_is_refused() -> Res<()> {
    // R15: not HTTP at all.
    for junk in [
        b"SSH-2.0-OpenSSH_9.6\r\n\r\nhello".to_vec(),
        b"garbage without any line end".to_vec(),
        b"\x00\x01\x02\x03\x04\r\n\r\n".to_vec(),
    ] {
        let router = router_sending(junk.clone(), false)?;
        let gate = router.gatekeeper()?;
        let got = fetch_with(&gate, SITE, quick());
        assert!(got.is_err(), "R15: accepted {junk:?}");
    }
    Ok(())
}

#[test]
fn r16_the_content_type_does_not_change_what_fetch_returns() -> Res<()> {
    // R16: the type comes from the magic bytes later; fetch returns the raw body for any
    // Content-Type, and the sanitizer decides.
    let png_magic = [0x89_u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
    for content_type in ["text/html", "image/svg+xml", "application/octet-stream"] {
        let length = format!("Content-Length: {}", png_magic.len());
        let type_line = format!("Content-Type: {content_type}");
        let router = router_sending(http("200 OK", &[&length, &type_line], &png_magic), false)?;
        let gate = router.gatekeeper()?;
        assert_eq!(fetch(&gate, SITE), Ok(png_magic.to_vec()), "{content_type}");
    }
    Ok(())
}

#[test]
fn r11_reached_site_is_false_for_the_failures_that_never_reached_the_site() {
    // R11: no connection, timeout, and the gatekeeper's or the router's own 502 and 503.
    for error in [
        FetchError::NotI2p,
        FetchError::Io("closed".to_owned()),
        FetchError::Timeout,
        FetchError::Status(502),
        FetchError::Status(503),
    ] {
        assert!(
            !error.reached_site(),
            "R11: {error:?} did not reach the site"
        );
    }
}

#[test]
fn r11_reached_site_is_true_for_every_other_failure() {
    // R11: every other failure counts.
    for error in [
        FetchError::Status(301),
        FetchError::Status(403),
        FetchError::Status(404),
        FetchError::Status(500),
        FetchError::Status(504),
        FetchError::TooLarge,
        FetchError::Chunked,
        FetchError::Malformed,
    ] {
        assert!(error.reached_site(), "R11: {error:?} reached the site");
    }
}

#[test]
fn r11_a_closed_gatekeeper_a_503_and_a_stalled_router_did_not_reach_the_site() -> Res<()> {
    // R11: the real failures, through a real gatekeeper.
    let router = router_sending(ok(b"x"), false)?;
    let gate = router.gatekeeper()?;
    gate.close();
    let closed = fetch_with(&gate, SITE, quick())
        .err()
        .ok_or("closed gatekeeper answered")?;
    assert!(!closed.reached_site(), "{closed:?}");
    let busy = router_sending(http("503 Busy", &["Content-Length: 0"], b""), false)?;
    let got = fetch_with(&busy.gatekeeper()?, SITE, quick())
        .err()
        .ok_or("503 accepted")?;
    assert!(!got.reached_site(), "{got:?}");
    let silent = router_sending(Vec::new(), true)?;
    let limits = Limits {
        timeout: Duration::from_millis(500),
        max_body: 65_536,
    };
    let late = fetch_with(&silent.gatekeeper()?, SITE, limits)
        .err()
        .ok_or("no timeout")?;
    assert_eq!(late, FetchError::Timeout);
    assert!(!late.reached_site());
    Ok(())
}

#[test]
fn r11_a_404_and_a_redirect_from_the_site_count_as_reached() -> Res<()> {
    // R11: the site answered, so the attempt counts.
    for code in [404_u16, 301] {
        let answer = http(&format!("{code} X"), &["Content-Length: 0"], b"");
        let router = router_sending(answer, false)?;
        let got = fetch_with(&router.gatekeeper()?, SITE, quick())
            .err()
            .ok_or("accepted")?;
        assert!(got.reached_site(), "{got:?}");
    }
    Ok(())
}
