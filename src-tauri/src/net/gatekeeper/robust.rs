// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Robustness tests of the gatekeeper with real loopback sockets and a fake router proxy:
//! random and malformed request heads, CONNECT lines, huge heads, partial reads, slow and
//! half-closed clients. Each test names the requirement it checks (`Req:`).
//!
//! Sources: ADR 0001 layer L1 (`docs/wiki/adr-0001-no-leak-architecture.md`) and the
//! doc comments of the public functions.

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::time::Instant;

use tauri::Url;

use super::*;
use crate::net::host::is_i2p_host;
use crate::net::testing::FakeRouter;

const READ_LIMIT: Duration = Duration::from_secs(5);

struct Fixture {
    router: FakeRouter,
    gate: Gatekeeper,
}

impl Fixture {
    fn new() -> Self {
        let router = FakeRouter::start();
        let gate = Gatekeeper::start(&router.verified()).unwrap();
        Self { router, gate }
    }

    fn connect(&self) -> TcpStream {
        let addr = LoopbackAddr::parse(&self.gate.url()).unwrap();
        addr.connect(Duration::from_secs(2)).unwrap()
    }
}

/// What came back and whether the gatekeeper stayed silent until the read timeout.
struct Answer {
    text: String,
    hung: bool,
}

impl Answer {
    fn status(&self) -> Option<u16> {
        self.text.strip_prefix("HTTP/1.1 ")?.get(..3)?.parse().ok()
    }

    fn refused(&self) -> bool {
        matches!(self.status(), Some(400..=499))
    }
}

/// Sends `chunks` (a pause between them), half-closes, and reads to the end.
fn exchange(fx: &Fixture, chunks: &[&[u8]], limit: Duration) -> Answer {
    let mut stream = fx.connect();
    stream.set_read_timeout(Some(limit)).unwrap();
    for chunk in chunks {
        let _ = stream.write_all(chunk);
        let _ = stream.flush();
        thread::sleep(Duration::from_millis(3));
    }
    let _ = stream.shutdown(Shutdown::Write);
    read_answer(&mut stream)
}

fn read_answer(stream: &mut TcpStream) -> Answer {
    let mut out = Vec::new();
    let result = stream.read_to_end(&mut out);
    let hung = matches!(&result, Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut));
    Answer {
        text: String::from_utf8_lossy(&out).into_owned(),
        hung,
    }
}

fn send(fx: &Fixture, raw: &[u8]) -> Answer {
    exchange(fx, &[raw], READ_LIMIT)
}

/// The host a router request line names: `GET http://h/p HTTP/1.1` or `CONNECT h:443 HTTP/1.1`.
fn line_host(line: &str) -> Option<String> {
    let target = line.split(' ').nth(1)?;
    let authority = match target.strip_prefix("http://") {
        Some(rest) => rest.split('/').next()?,
        None => target.rsplit_once(':')?.0,
    };
    Some(
        authority
            .rsplit_once(':')
            .map_or(authority, |(h, _)| h)
            .to_owned(),
    )
}

/// Req: ADR L1 "forwards a request only when its host passes `is_i2p_host`": every request line
/// the router saw names an I2P host.
fn router_saw_only_i2p(fx: &Fixture, from: usize) -> Result<(), TestCaseError> {
    for line in fx.router.lines().iter().skip(from) {
        let ok = line_host(line).is_some_and(|h| is_i2p_host(&h));
        prop_assert!(ok, "the router got a request for a non-I2P host: {line:?}");
    }
    Ok(())
}

/// Req: ADR L1 "the gatekeeper answers a valid I2P request": the liveness probe.
fn assert_serves(fx: &Fixture) {
    let answer = send(
        fx,
        b"GET http://site.i2p/ HTTP/1.1\r\nHost: site.i2p\r\n\r\n",
    );
    assert_eq!(answer.status(), Some(200), "{}", answer.text);
    // Req: ADR L3a "the gatekeeper adds a Content-Security-Policy to every response".
    assert!(
        answer.text.contains("Content-Security-Policy:"),
        "{}",
        answer.text
    );
}

#[derive(Debug, Clone)]
enum End {
    Crlf,
    BareLf,
    Open,
}

#[derive(Debug, Clone)]
struct Req {
    method: String,
    target: String,
    version: String,
    headers: Vec<String>,
    end: End,
}

impl Req {
    fn raw(&self) -> Vec<u8> {
        let mut text = format!("{} {} {}\r\n", self.method, self.target, self.version);
        for header in &self.headers {
            text.push_str(header);
            text.push_str("\r\n");
        }
        text.push_str(match self.end {
            End::Crlf => "\r\n",
            End::BareLf => "\n",
            End::Open => "",
        });
        text.into_bytes()
    }

    /// Req: ADR L1 "everything else gets 403 with no upstream connection": true when the target
    /// names a host the engine would read as non-I2P (read with the URL rules the engine uses),
    /// or a CONNECT to anything but `*.i2p:443`.
    fn must_refuse(&self) -> bool {
        if self.method == "CONNECT" {
            return !connect_allowed(&self.target);
        }
        if !self.target.starts_with("http://") {
            return false;
        }
        match Url::parse(&self.target) {
            Ok(url) => {
                let clean = url.username().is_empty() && url.password().is_none();
                !(clean && url.host_str().is_some_and(is_i2p_host))
            }
            Err(_) => true,
        }
    }
}

/// Req: PO decision "CONNECT only to :443" (an I2P host).
fn connect_allowed(authority: &str) -> bool {
    authority
        .rsplit_once(':')
        .is_some_and(|(host, port)| is_i2p_host(host) && port == "443")
}

fn authority() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z0-9]{1,8}\\.i2p",
        "[a-z0-9]{1,8}\\.i2p:[0-9]{1,5}",
        "[A-Za-z0-9]{1,8}\\.[iI]2[pP]\\.?",
        "[a-z0-9]{1,8}@[a-z0-9]{1,8}\\.i2p",
        "[a-z0-9]{1,8}\\.i2p@[a-z0-9]{1,8}\\.(com|org|net)",
        "[a-z0-9]{1,8}\\.i2p\\\\@[a-z0-9]{1,8}\\.com",
        "[a-z0-9]{1,8}\\.com#@[a-z0-9]{1,8}\\.i2p",
        "[a-z0-9]{1,8}\\.i2p\\.[a-z]{2,3}",
        "[a-z0-9]{1,8}\\.(com|org|onion|local|lan)(:[0-9]{1,5})?",
        "([0-9]{1,3}\\.){3}[0-9]{1,3}(:[0-9]{1,5})?",
        "\\[[0-9a-f:]{2,20}\\](:[0-9]{1,5})?",
        "localhost(:[0-9]{1,5})?",
        "[\\x{e0}-\\x{ff}a-z]{1,8}\\.i2p",
        "xn--[a-z0-9]{1,8}\\.i2p",
        "\\PC{0,30}",
    ]
}

fn target() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => (authority(), "(/[a-z0-9?=&.%]{0,8}){0,3}").prop_map(|(a, p)| format!("http://{a}{p}")),
        1 => (authority(), "(/[a-z0-9]{0,8}){0,2}").prop_map(|(a, p)| format!("https://{a}{p}")),
        2 => authority(),
        1 => "(/[a-z0-9]{0,8}){1,3}",
        1 => Just("*".to_owned()),
        1 => "[ -~]{0,40}",
    ]
}

fn headers() -> impl Strategy<Value = Vec<String>> {
    let line = prop_oneof![
        "(Host|Accept|User-Agent|X-[A-Za-z]{1,8}): [ -~]{0,50}",
        "[ -~]{1,30}",
        Just(format!("X-Big: {}", "a".repeat(5_000))),
        "Host: [a-z0-9.:]{1,20}",
    ];
    prop::collection::vec(line, 0..6)
}

fn request() -> impl Strategy<Value = Req> {
    let method = prop_oneof![
        3 => Just("GET".to_owned()),
        2 => Just("CONNECT".to_owned()),
        1 => Just("POST".to_owned()),
        1 => "[A-Za-z]{1,8}",
        1 => "[ -~]{0,8}",
    ];
    let version = prop_oneof![
        4 => Just("HTTP/1.1".to_owned()),
        1 => Just("HTTP/1.0".to_owned()),
        1 => Just("HTTP/2".to_owned()),
        1 => "[ -~]{0,8}",
    ];
    let end = prop_oneof![
        6 => Just(End::Crlf),
        1 => Just(End::BareLf),
        1 => Just(End::Open),
    ];
    (method, target(), version, headers(), end).prop_map(
        |(method, target, version, headers, end)| Req {
            method,
            target,
            version,
            headers,
            end,
        },
    )
}

fn check_request(fx: &Fixture, req: &Req) -> Result<(), TestCaseError> {
    let before_lines = fx.router.lines().len();
    let before = fx.router.connections();
    let answer = send(fx, &req.raw());
    if req.must_refuse() {
        prop_assert!(answer.refused(), "not refused: {req:?} -> {}", answer.text);
        prop_assert_eq!(
            fx.router.connections(),
            before,
            "upstream connection for {:?}",
            req
        );
    }
    // Lines are logged after the exchange; the answer is complete, so the router is done.
    thread::sleep(Duration::from_millis(1));
    router_saw_only_i2p(fx, before_lines)
}

fn runner(cases: u32) -> TestRunner {
    TestRunner::new(Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    })
}

// Req: ADR L1 "forwards a request only when its host passes is_i2p_host, only to the verified
// router; everything else 403 with no upstream connection". Random request heads: absolute and
// origin form, CONNECT, odd methods and versions, bare LF, open heads, userinfo, ports, IPv6,
// IDN, uppercase, trailing dots, long headers.
#[test]
fn random_request_heads_never_reach_the_router_unless_i2p() {
    let fx = Fixture::new();
    runner(300)
        .run(&request(), |req| check_request(&fx, &req))
        .unwrap();
    assert_serves(&fx);
}

// Req: ADR L1 "everything else gets 403" / task "answers 400/403 for garbage": random bytes
// that end in a blank line, or end in EOF, are refused with a 4xx and open no upstream
// connection.
#[test]
fn garbage_bytes_get_a_4xx_and_no_upstream_connection() {
    let fx = Fixture::new();
    let strategy = (prop::collection::vec(any::<u8>(), 0..400), any::<bool>());
    runner(200)
        .run(&strategy, |(mut bytes, terminate)| {
            if terminate {
                bytes.extend_from_slice(b"\r\n\r\n");
            }
            let before = fx.router.connections();
            let answer = send(&fx, &bytes);
            prop_assert!(answer.refused(), "{bytes:?} -> {}", answer.text);
            prop_assert_eq!(fx.router.connections(), before);
            Ok(())
        })
        .unwrap();
    assert_serves(&fx);
}

// Req: TCP is a byte stream: a request cut into pieces, with pauses, gets the same answer as
// the same request in one write.
#[test]
fn partial_reads_give_the_same_answer() {
    let fx = Fixture::new();
    let requests: [&[u8]; 5] = [
        b"GET http://site.i2p/a HTTP/1.1\r\nHost: site.i2p\r\n\r\n",
        b"GET http://example.com/ HTTP/1.1\r\nHost: example.com\r\n\r\n",
        b"GET http://site.i2p:8080/x HTTP/1.1\r\nHost: site.i2p:8080\r\n\r\n",
        b"CONNECT 127.0.0.1:80 HTTP/1.1\r\n\r\n",
        b"POST http://site.i2p/ HTTP/1.1\r\nContent-Length: 4\r\n\r\nabcd",
    ];
    let strategy = (0..requests.len(), prop::collection::vec(1..40usize, 1..4));
    runner(60)
        .run(&strategy, |(which, cuts)| {
            let raw = requests[which];
            let whole = send(&fx, raw);
            let mut pieces: Vec<&[u8]> = Vec::new();
            let mut rest = raw;
            for cut in cuts {
                let at = cut.min(rest.len());
                let (head, tail) = rest.split_at(at);
                pieces.push(head);
                rest = tail;
            }
            pieces.push(rest);
            let split = exchange(&fx, &pieces, READ_LIMIT);
            prop_assert_eq!(split.status(), whole.status(), "{:?}", pieces);
            Ok(())
        })
        .unwrap();
}

// Req: task "huge headers": a head far over any sane size is refused or dropped, never
// forwarded, and the gatekeeper keeps serving. Includes one 8 MB line and 50 000 header lines.
#[test]
fn huge_heads_are_refused_and_the_gatekeeper_lives() {
    let fx = Fixture::new();
    let before = fx.router.connections();
    let mut long_line = b"GET http://site.i2p/ HTTP/1.1\r\nX-A: ".to_vec();
    long_line.extend(std::iter::repeat_n(b'a', 8 * 1024 * 1024));
    long_line.extend_from_slice(b"\r\n\r\n");
    let mut many = b"GET http://site.i2p/ HTTP/1.1\r\n".to_vec();
    for n in 0..50_000 {
        many.extend_from_slice(format!("X-{n}: v\r\n").as_bytes());
    }
    many.extend_from_slice(b"\r\n");
    for raw in [long_line, many] {
        let answer = send(&fx, &raw);
        assert!(!answer.hung, "no answer, no close");
        assert_ne!(
            answer.status(),
            Some(200),
            "a head over any limit was forwarded"
        );
    }
    assert_eq!(
        fx.router.connections(),
        before,
        "an oversized head reached the router"
    );
    assert_serves(&fx);
}

// Req: task "a slow or half-closed client does not block other connections": idle clients, a
// client that trickles its head, and clients that vanish mid-request do not delay a good one.
#[test]
fn slow_and_half_closed_clients_do_not_block_others() {
    let fx = Fixture::new();
    let mut idle: Vec<TcpStream> = (0..20).map(|_| fx.connect()).collect();
    for stream in &mut idle {
        let _ = stream.write_all(b"GET http://site.i2p/ HT");
    }
    let mut trickle = fx.connect();
    let _ = trickle.write_all(b"G");
    let mut gone = fx.connect();
    let _ = gone.write_all(b"POST http://site.i2p/ HTTP/1.1\r\nContent-Length: 100\r\n\r\nab");
    let _ = gone.shutdown(Shutdown::Both);
    drop(gone);
    let started = Instant::now();
    assert_serves(&fx);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    let _ = trickle.write_all(b"E");
    let empty = exchange(&fx, &[], Duration::from_secs(2));
    assert!(
        !empty.hung && empty.refused(),
        "an empty half-closed client: {}",
        empty.text
    );
    assert_serves(&fx);
    drop(idle);
}

// Req: task "half-closed client": a client that announces a body, sends part of it and closes
// its sending side gets an answer or a close, not silence. Silence would hold a gatekeeper
// slot and a router connection until a timeout of minutes.
#[test]
fn a_truncated_body_ends_the_exchange() {
    let fx = Fixture::new();
    let raw = b"POST http://site.i2p/ HTTP/1.1\r\nContent-Length: 100\r\n\r\nab";
    let answer = exchange(&fx, &[raw], Duration::from_secs(4));
    assert!(
        !answer.hung,
        "the gatekeeper waits for the rest of the body of a closed client"
    );
}

// Req: ADR "TLS tunnels open only on Windows": a CONNECT to port 443 is refused on macOS and
// Linux, with no upstream connection. Port 80 is terminated and checked.
#[test]
fn connect_443_is_closed_off_windows() {
    let fx = Fixture::new();
    let before = fx.router.connections();
    let answer = send(&fx, b"CONNECT tls.i2p:443 HTTP/1.1\r\n\r\n");
    if cfg!(windows) {
        assert!(answer.text.starts_with("HTTP/1.1 200"), "{}", answer.text);
    } else {
        assert!(answer.refused(), "{}", answer.text);
        assert_eq!(fx.router.connections(), before);
    }
}

// Req: PO decision "CONNECT only to :443": a CONNECT to any other port, port 80 included, is
// refused with a 4xx and no upstream connection.
#[test]
fn connect_to_other_ports_is_refused() {
    let fx = Fixture::new();
    let before = fx.router.connections();
    for port in [0, 1, 22, 80, 81, 8080, 4444, 65535] {
        let raw = format!(
            "CONNECT site.i2p:{port} HTTP/1.1\r\n\r\nGET / HTTP/1.1\r\nHost: site.i2p\r\n\r\n"
        );
        let answer = send(&fx, raw.as_bytes());
        assert!(answer.refused(), "port {port}: {}", answer.text);
    }
    assert_eq!(fx.router.connections(), before);
}

// Req: PO decision ".i2p URLs with an explicit port 1-65535 are allowed": the request reaches
// the router with that port, and the page gets the answer (with the page policy). Port 0 is
// refused.
#[test]
fn explicit_ports_on_i2p_urls_are_forwarded() {
    let fx = Fixture::new();
    for port in [1, 81, 8080, 65535] {
        let raw = format!("GET http://site.i2p:{port}/p HTTP/1.1\r\nHost: site.i2p:{port}\r\n\r\n");
        let answer = send(&fx, raw.as_bytes());
        assert_eq!(answer.status(), Some(200), "port {port}: {}", answer.text);
        assert!(answer.text.contains("Content-Security-Policy:"));
        let lines = fx.router.lines();
        let last = lines.last().map_or("", String::as_str);
        assert!(
            last.contains(&format!("site.i2p:{port}/p")),
            "port {port}: {last}"
        );
    }
    let zero = send(&fx, b"GET http://site.i2p:0/ HTTP/1.1\r\n\r\n");
    assert!(zero.refused(), "{}", zero.text);
}

// Req: PO decision "duplicate Content-Length headers give 400 and close": equal or different
// values, any letter case. Nothing reaches the router, and the connection ends.
#[test]
fn duplicate_content_length_is_a_400() {
    let fx = Fixture::new();
    let before = fx.router.connections();
    for (a, b) in [("5", "5"), ("5", "6"), ("0", "5"), ("5", "5, 5")] {
        let raw = format!(
            "POST http://site.i2p/ HTTP/1.1\r\nContent-Length: {a}\r\ncontent-length: {b}\r\n\r\nhello"
        );
        let answer = send(&fx, raw.as_bytes());
        assert_eq!(answer.status(), Some(400), "{a}/{b}: {}", answer.text);
        assert!(!answer.hung, "{a}/{b}: the connection stays open");
    }
    assert_eq!(fx.router.connections(), before);
    assert_serves(&fx);
}

// Req: ADR "Fail closed": after `close()` the gatekeeper forwards nothing, whatever arrives.
#[test]
fn nothing_is_forwarded_after_close() {
    let fx = Fixture::new();
    fx.gate.close();
    thread::sleep(Duration::from_millis(150));
    let before = fx.router.connections();
    for _ in 0..5 {
        let addr = LoopbackAddr::parse(&fx.gate.url()).unwrap();
        if let Ok(mut stream) = addr.connect(Duration::from_millis(500)) {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = stream.write_all(b"GET http://site.i2p/ HTTP/1.1\r\n\r\n");
            let answer = read_answer(&mut stream);
            assert_ne!(answer.status(), Some(200), "{}", answer.text);
        }
    }
    assert_eq!(fx.router.connections(), before);
}
