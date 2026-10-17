// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Robustness tests of the gatekeeper with real loopback sockets and a fake router proxy:
//! random and malformed request heads, CONNECT lines, huge heads, partial reads, slow and
//! half-closed clients, slow concurrent images. Each test names the requirement it checks
//! (`Req:`). They use the public API only and run in their own process, so their many
//! sockets cannot disturb the unit tests of the gatekeeper.
//!
//! Sources: ADR 0001 layer L1 (`docs/wiki/adr-0001-no-leak-architecture.md`) and the
//! doc comments of the public functions.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eepview_lib::net::gatekeeper::Gatekeeper;
use eepview_lib::net::host::is_i2p_host;
use eepview_lib::net::http::{self, Head};
use eepview_lib::net::loopback::LoopbackAddr;
use eepview_lib::net::verify::{Verdict, VerifiedUpstream, verify};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use tauri::Url;

const READ_LIMIT: Duration = Duration::from_secs(5);

#[cfg(test)]
mod fake {
    use super::*;

    /// A fake I2P router proxy: counts connections, records request lines, answers VERIFY, echoes
    /// a `tls.i2p:443` tunnel, and sends `hello body=<body>` for any other request. A path of
    /// the form `/img<n>.png` gets its first byte after `image_delay * (n + 2)` and a 50 KB
    /// `image/png` body.
    pub struct FakeRouter {
        pub addr: LoopbackAddr,
        pub connections: Arc<AtomicUsize>,
        pub lines: Arc<Mutex<Vec<String>>>,
    }

    impl FakeRouter {
        pub fn start(image_delay: Duration) -> Self {
            let (listener, addr) = LoopbackAddr::listen_any().unwrap();
            let connections = Arc::new(AtomicUsize::new(0));
            let lines = Arc::new(Mutex::new(Vec::new()));
            let (count, log) = (Arc::clone(&connections), Arc::clone(&lines));
            thread::spawn(move || serve(&listener, &count, &log, image_delay));
            Self {
                addr,
                connections,
                lines,
            }
        }

        pub fn connections(&self) -> usize {
            self.connections.load(Ordering::SeqCst)
        }

        pub fn lines(&self) -> Vec<String> {
            self.lines.lock().unwrap().clone()
        }

        pub fn verified(&self) -> VerifiedUpstream {
            match verify(self.addr) {
                Verdict::Ok(up) => up,
                other => panic!("the fake router failed VERIFY: {other:?}"),
            }
        }
    }

    fn serve(
        listener: &TcpListener,
        count: &Arc<AtomicUsize>,
        log: &Arc<Mutex<Vec<String>>>,
        image_delay: Duration,
    ) {
        for stream in listener.incoming().flatten() {
            count.fetch_add(1, Ordering::SeqCst);
            let log = Arc::clone(log);
            thread::spawn(move || answer(stream, &log, image_delay));
        }
    }

    fn answer(mut stream: TcpStream, log: &Mutex<Vec<String>>, image_delay: Duration) {
        let Some((head, mut rest)) = read_head(&mut stream) else {
            return;
        };
        let length = head.body_length().unwrap_or(0);
        let missing = length.saturating_sub(rest.len() as u64);
        let _ = (&mut stream).take(missing).read_to_end(&mut rest);
        log.lock().unwrap().push(head.start.clone());
        let target = head.start.split(' ').nth(1).unwrap_or("").to_owned();
        match (target.as_str(), image_number(&target)) {
            ("http://proxy.i2p/", _) => {
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nI2P HTTP proxy OK");
            }
            ("tls.i2p:443", _) => echo_tunnel(stream),
            (_, Some(n)) => send_image(stream, n, image_delay),
            _ => {
                let body = format!("hello body={}", String::from_utf8_lossy(&rest));
                let reply = format!("HTTP/1.1 200 OK\r\nConnection: keep-alive\r\n\r\n{body}");
                let _ = stream.write_all(reply.as_bytes());
            }
        }
    }

    fn read_head(stream: &mut TcpStream) -> Option<(Head, Vec<u8>)> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        while http::head_end(&buf).is_none() {
            let n = stream.read(&mut chunk).ok().filter(|n| *n > 0)?;
            buf.extend_from_slice(&chunk[..n]);
        }
        let end = http::head_end(&buf)?;
        let rest = buf.split_off(end);
        Head::parse(&buf).map(|h| (h, rest))
    }

    fn echo_tunnel(mut stream: TcpStream) {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n");
        let mut buf = [0u8; 64];
        if let Ok(n) = stream.read(&mut buf) {
            let _ = stream.write_all(&buf[..n]);
        }
    }

    fn image_number(target: &str) -> Option<usize> {
        let (_, rest) = target.rsplit_once("/img")?;
        rest.strip_suffix(".png")?.parse().ok()
    }

    /// The image of request `n`: 50 KB that differ per request, so a mix-up shows.
    pub fn image_bytes(n: usize) -> Vec<u8> {
        let seed = u8::try_from(n % 251).unwrap_or(0);
        (0..50_000usize)
            .map(|i| u8::try_from(i % 251).unwrap_or(0).wrapping_add(seed))
            .collect()
    }

    fn send_image(mut stream: TcpStream, n: usize, delay: Duration) {
        thread::sleep(delay * u32::try_from(n + 2).unwrap_or(2));
        let body = image_bytes(n);
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&body);
    }

    pub struct Fixture {
        pub router: FakeRouter,
        pub gate: Gatekeeper,
    }

    impl Fixture {
        pub fn new() -> Self {
            let router = FakeRouter::start(Duration::ZERO);
            let gate = Gatekeeper::start(&router.verified()).unwrap();
            Self { router, gate }
        }

        pub fn connect_with_timeouts(&self, limit: Duration) -> TcpStream {
            let stream = self.connect();
            stream.set_read_timeout(Some(limit)).unwrap();
            stream.set_write_timeout(Some(limit)).unwrap();
            stream
        }

        pub fn connect(&self) -> TcpStream {
            let addr = LoopbackAddr::parse(&self.gate.url()).unwrap();
            addr.connect(Duration::from_secs(2)).unwrap()
        }
    }
}

use fake::{FakeRouter, Fixture, image_bytes};

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
    // A gatekeeper that stops reading must fail the test, not hang `write_all`.
    let mut stream = fx.connect_with_timeouts(limit);
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
    /// is not a clean absolute `http://` URL on an I2P host (https, authority form, origin form,
    /// `*`, any other scheme), or a CONNECT to anything but `*.i2p` on a port 1-65535 (`:443`
    /// only on Windows).
    fn must_refuse(&self) -> bool {
        if self.method == "CONNECT" {
            return !connect_allowed(&self.target);
        }
        if !self.target.starts_with("http://") {
            return true;
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

/// Req: ADR L1 "CONNECT goes only to *.i2p" on a port of digits, 1-65535 (an I2P host; `:443`
/// also needs TLS tunnels, which the fixture opens only on Windows).
fn connect_allowed(authority: &str) -> bool {
    authority.rsplit_once(':').is_some_and(|(host, port)| {
        let digits = port.bytes().all(|b| b.is_ascii_digit());
        let number = port.parse::<u16>().ok().filter(|_| digits);
        is_i2p_host(host) && number.is_some_and(|n| n != 0 && (n != 443 || cfg!(windows)))
    })
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

// Req: ADR L1 "a CONNECT port must be 1-65535, digits only": port 0, ports over 65535 and
// ports that are not digits are refused with a 4xx and no upstream connection. Port 443
// passes only where TLS tunnels are open (Windows).
#[test]
fn connect_ports_out_of_range_are_refused() {
    let fx = Fixture::new();
    let before = fx.router.connections();
    for port in ["0", "65536", "99999", "+80", "8o"] {
        let raw = format!(
            "CONNECT site.i2p:{port} HTTP/1.1\r\n\r\nGET / HTTP/1.1\r\nHost: site.i2p:{port}\r\n\r\n"
        );
        let answer = send(&fx, raw.as_bytes());
        assert!(answer.refused(), "port {port}: {}", answer.text);
    }
    assert_eq!(
        fx.router.connections(),
        before,
        "a refused port reached the router"
    );
    // :443 passes where TLS tunnels are open (Windows); elsewhere it is refused (ADR).
    let tls = send(&fx, b"CONNECT tls.i2p:443 HTTP/1.1\r\n\r\nping");
    if cfg!(windows) {
        assert!(
            tls.text.starts_with("HTTP/1.1 200 Connection established"),
            "{}",
            tls.text
        );
    } else {
        assert!(tls.refused(), "{}", tls.text);
    }
}

/// The answer head inside a `CONNECT site.i2p:<port>` tunnel for `GET /p` with `Host: <host>`.
fn tunnel_get(fx: &Fixture, port: u16, host: &str) -> String {
    let raw =
        format!("CONNECT site.i2p:{port} HTTP/1.1\r\n\r\nGET /p HTTP/1.1\r\nHost: {host}\r\n\r\n");
    let answer = send(fx, raw.as_bytes());
    assert!(
        answer
            .text
            .starts_with("HTTP/1.1 200 Connection established"),
        "port {port}: {}",
        answer.text
    );
    tunnel_response(answer.text.as_bytes()).0
}

// Req: ADR L1 / browser-shell "eepsite ports 1-65535 work": a CONNECT to *.i2p on any port
// but 443 is terminated and checked like plain HTTP, and its request reaches the router with
// that port (macOS sends plain http as CONNECT host:port). The answer has the page policy.
#[test]
fn connect_ports_other_than_443_are_terminated() {
    let fx = Fixture::new();
    for port in [1, 22, 81, 7657, 8080, 4444, 65535] {
        let head = tunnel_get(&fx, port, &format!("site.i2p:{port}"));
        assert!(head.starts_with("HTTP/1.1 200 OK"), "port {port}: {head}");
        assert!(head.contains("Content-Security-Policy:"), "port {port}");
        let line = format!("GET http://site.i2p:{port}/p HTTP/1.1");
        assert_eq!(fx.router.lines().last(), Some(&line));
    }
    let head = tunnel_get(&fx, 80, "site.i2p");
    assert!(head.starts_with("HTTP/1.1 200 OK"), "port 80: {head}");
    let line = "GET http://site.i2p/p HTTP/1.1".to_owned();
    assert_eq!(fx.router.lines().last(), Some(&line));
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

/// The body of a tunnelled answer: after `200 Connection established`, one HTTP response.
fn tunnel_response(raw: &[u8]) -> (String, Vec<u8>) {
    let established = b"HTTP/1.1 200 Connection established\r\n\r\n";
    let rest = raw.strip_prefix(established.as_slice()).unwrap_or(raw);
    let end = http::head_end(rest).unwrap_or(rest.len());
    let head = String::from_utf8_lossy(&rest[..end]).into_owned();
    (head, rest[end..].to_vec())
}

// Req: "a page's same-site .i2p images load completely, also when the engine fetches many of
// them at once": 8 concurrent `CONNECT a.i2p:80` tunnels, each with a GET for an image, to a
// router that delays the first byte by 2 to 9 s and answers 50 KB `image/png` with
// `Content-Length`. Every answer is 200 and the body is byte-identical to what the router sent.
#[test]
fn eight_concurrent_slow_images_arrive_complete() {
    // 2 s for image 0, 3 s for image 1, ... 9 s for image 7.
    let router = FakeRouter::start(Duration::from_secs(1));
    let gate = Gatekeeper::start(&router.verified()).unwrap();
    let addr = LoopbackAddr::parse(&gate.url()).unwrap();
    let workers: Vec<_> = (0..8usize)
        .map(|n| {
            thread::spawn(move || {
                let mut stream = addr.connect(Duration::from_secs(5)).unwrap();
                stream.set_read_timeout(Some(Duration::from_mins(1))).unwrap();
                let raw = format!(
                    "CONNECT a.i2p:80 HTTP/1.1\r\n\r\nGET /img{n}.png HTTP/1.1\r\nHost: a.i2p\r\nAccept: image/png\r\n\r\n"
                );
                stream.write_all(raw.as_bytes()).unwrap();
                let _ = stream.shutdown(Shutdown::Write);
                let answer = read_answer_bytes(&mut stream);
                (n, answer)
            })
        })
        .collect();
    for worker in workers {
        let (n, raw) = worker.join().unwrap();
        let (head, body) = tunnel_response(&raw);
        assert!(head.starts_with("HTTP/1.1 200"), "image {n}: {head}");
        assert!(head.contains("image/png"), "image {n}: {head}");
        assert_eq!(body.len(), 50_000, "image {n}: short or long body");
        assert!(body == image_bytes(n), "image {n}: the bytes differ");
    }
}

fn read_answer_bytes(stream: &mut TcpStream) -> Vec<u8> {
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
    out
}
