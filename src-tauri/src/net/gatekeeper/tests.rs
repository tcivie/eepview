// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Gatekeeper tests with real loopback sockets and a fake router proxy on `127.0.0.1:0`.

use super::*;
use crate::net::testing::FakeRouter;

fn send(gate: &Gatekeeper, raw: &str) -> String {
    let mut stream = gate.addr.connect(Duration::from_secs(5)).unwrap();
    stream.write_all(raw.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = stream.read_to_end(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

fn setup() -> (FakeRouter, Gatekeeper, usize) {
    let router = FakeRouter::start();
    let gate = Gatekeeper::start_with(&router.verified(), true).unwrap();
    let base = router.connections();
    (router, gate, base)
}

#[test]
fn i2p_request_is_forwarded_with_policy() {
    let (router, gate, base) = setup();
    let out = send(
        &gate,
        "GET http://site.i2p/a?b HTTP/1.1\r\nHost: site.i2p\r\n\r\n",
    );
    assert!(out.starts_with("HTTP/1.1 200 OK\r\n"), "{out}");
    assert!(out.contains("Content-Security-Policy: default-src http://*.i2p:*"));
    assert!(out.contains("Connection: close") && !out.contains("keep-alive"));
    assert!(out.ends_with("hello body="));
    assert_eq!(router.connections(), base + 1);
    assert!(
        router
            .lines()
            .contains(&"GET http://site.i2p/a?b HTTP/1.1".to_owned())
    );
    assert!(gate.url().starts_with("http://127.0.0.1:"));
}

#[test]
fn post_body_is_forwarded() {
    let (_router, gate, _) = setup();
    let out = send(
        &gate,
        "POST http://site.i2p/ HTTP/1.1\r\nContent-Length: 5\r\n\r\nab=cd",
    );
    assert!(out.ends_with("hello body=ab=cd"), "{out}");
}

#[test]
fn connect_80_is_terminated_and_checked() {
    let (router, gate, base) = setup();
    let out = send(
        &gate,
        "CONNECT site.i2p:80 HTTP/1.1\r\n\r\nGET /x HTTP/1.1\r\nHost: site.i2p\r\n\r\n",
    );
    assert!(
        out.starts_with("HTTP/1.1 200 Connection established\r\n\r\nHTTP/1.1 200 OK"),
        "{out}"
    );
    assert!(
        router
            .lines()
            .contains(&"GET http://site.i2p/x HTTP/1.1".to_owned())
    );
    let evil = send(
        &gate,
        "CONNECT site.i2p:80 HTTP/1.1\r\n\r\nGET / HTTP/1.1\r\nHost: example.com\r\n\r\n",
    );
    assert!(evil.contains("HTTP/1.1 403"), "{evil}");
    assert_eq!(router.connections(), base + 1);
}

#[test]
fn connect_443_relays_tls_bytes() {
    let (router, gate, base) = setup();
    let out = send(&gate, "CONNECT tls.i2p:443 HTTP/1.1\r\n\r\nping");
    assert_eq!(out, "HTTP/1.1 200 Connection established\r\n\r\nping");
    assert_eq!(router.connections(), base + 1);
}

#[test]
fn everything_else_is_refused_with_no_upstream_connection() {
    let (router, gate, base) = setup();
    for raw in [
        "GET http://example.com/ HTTP/1.1\r\n\r\n",
        "GET http://203.0.113.7/ HTTP/1.1\r\n\r\n",
        "GET http://127.0.0.1:7657/ HTTP/1.1\r\n\r\n",
        "GET http://localhost:7657/ HTTP/1.1\r\n\r\n",
        "GET http://[::1]:7657/ HTTP/1.1\r\n\r\n",
        "GET http://192.168.1.1/ HTTP/1.1\r\n\r\n",
        "GET http://10.0.0.1/ HTTP/1.1\r\n\r\n",
        "GET http://a@evil.com/#.i2p HTTP/1.1\r\n\r\n",
        "GET http://user@site.i2p/ HTTP/1.1\r\n\r\n",
        "GET http://site.i2p./ HTTP/1.1\r\n\r\n",
        "GET http://bücher.i2p/ HTTP/1.1\r\n\r\n",
        "GET http://xn--bcher-kva.i2p/ HTTP/1.1\r\n\r\n",
        "CONNECT example.com:443 HTTP/1.1\r\n\r\n",
        "CONNECT 127.0.0.1:80 HTTP/1.1\r\n\r\n",
        "CONNECT site.i2p:22 HTTP/1.1\r\n\r\n",
        "GET /relative HTTP/1.1\r\nHost: site.i2p\r\n\r\n",
    ] {
        let out = send(&gate, raw);
        assert!(out.starts_with("HTTP/1.1 403"), "{raw}: {out}");
    }
    assert_eq!(router.connections(), base);
}

#[test]
fn malformed_and_chunked_requests_are_refused() {
    let (router, gate, base) = setup();
    assert!(send(&gate, "garbage\r\n\r\n").starts_with("HTTP/1.1 400"));
    assert!(send(&gate, "\r\n\r\n").starts_with("HTTP/1.1 400"));
    let chunked = "POST http://site.i2p/ HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n";
    assert!(send(&gate, chunked).starts_with("HTTP/1.1 411"));
    let tunnel_junk = send(&gate, "CONNECT site.i2p:80 HTTP/1.1\r\n\r\nbad\r\n\r\n");
    assert!(tunnel_junk.ends_with("Bad Request"), "{tunnel_junk}");
    assert_eq!(router.connections(), base);
}

#[test]
fn tls_tunnels_can_be_closed() {
    let router = FakeRouter::start();
    let gate = Gatekeeper::start_with(&router.verified(), false).unwrap();
    let base = router.connections();
    assert!(send(&gate, "CONNECT tls.i2p:443 HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 403"));
    assert_eq!(router.connections(), base);
}

#[test]
fn closed_gatekeeper_refuses_all() {
    let (router, gate, base) = setup();
    gate.close();
    gate.close();
    thread::sleep(Duration::from_millis(100));
    let refused = gate.addr.connect(Duration::from_secs(1)).map(|mut s| {
        let _ = s.write_all(b"GET http://site.i2p/ HTTP/1.1\r\n\r\n");
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        out
    });
    assert!(refused.is_err() || !refused.unwrap().starts_with(b"HTTP/1.1 200"));
    assert_eq!(router.connections(), base);
}

#[test]
fn dead_router_gives_bad_gateway() {
    let router = FakeRouter::start();
    let up = router.verified();
    let gate = Gatekeeper::start_with(&up, true).unwrap();
    drop(router);
    // The fake keeps listening in its thread, so point a fresh gatekeeper at a closed port.
    let (listener, addr) = LoopbackAddr::listen_any().unwrap();
    drop(listener);
    let dead = Shared {
        upstream: addr,
        open: Arc::new(AtomicBool::new(true)),
        active: Arc::new(AtomicUsize::new(0)),
        limit: MAX_CONNECTIONS,
        allow_tls: true,
    };
    let (client_side, server_side) = socket_pair();
    let worker = thread::spawn(move || handle(server_side, &dead));
    let mut client = client_side;
    client
        .write_all(b"GET http://site.i2p/ HTTP/1.1\r\n\r\n")
        .unwrap();
    let mut out = Vec::new();
    let _ = client.read_to_end(&mut out);
    assert!(out.starts_with(b"HTTP/1.1 502"));
    worker.join().unwrap().unwrap();
    drop(gate);
}

fn socket_pair() -> (TcpStream, TcpStream) {
    let (listener, addr) = LoopbackAddr::listen_any().unwrap();
    let client = addr.connect(Duration::from_secs(2)).unwrap();
    let (server, _) = listener.accept().unwrap();
    (client, server)
}

#[test]
fn busy_limit_refuses() {
    let router = FakeRouter::start();
    let limit = 4;
    let gate = Gatekeeper::start_limited(&router.verified(), true, limit).unwrap();
    // Hold connections open without a request so the handlers wait.
    let held: Vec<TcpStream> = (0..limit)
        .map(|_| gate.addr.connect(Duration::from_secs(2)).unwrap())
        .collect();
    thread::sleep(Duration::from_millis(300));
    let out = send(&gate, "GET http://site.i2p/ HTTP/1.1\r\n\r\n");
    assert!(out.starts_with("HTTP/1.1 503"), "{out}");
    drop(held);
}

#[test]
fn accept_errors_back_off() {
    let mut b = Backoff::new();
    let waits: Vec<u128> = (0..7).map(|_| b.fail().as_millis()).collect();
    assert_eq!(waits, [50, 100, 200, 400, 800, 1000, 1000]);
    b.reset();
    assert_eq!(b.fail().as_millis(), 50);
}

#[test]
fn tls_tunnels_only_where_the_engine_filters() {
    assert_eq!(TLS_TUNNELS, cfg!(windows));
}

#[test]
fn a_cut_body_closes_both_sides_at_once() {
    let (_router, gate, _) = setup();
    let mut stream = gate.addr.connect(Duration::from_secs(5)).unwrap();
    stream
        .write_all(b"POST http://site.i2p/ HTTP/1.1\r\nContent-Length: 100\r\n\r\nab")
        .unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let started = std::time::Instant::now();
    let mut out = Vec::new();
    let read = stream.read_to_end(&mut out);
    assert!(read.is_ok(), "the gatekeeper kept the connection: {read:?}");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(!String::from_utf8_lossy(&out).contains("hello body"));
}

#[test]
fn duplicate_lengths_are_refused() {
    let (router, gate, base) = setup();
    let out = send(
        &gate,
        "POST http://site.i2p/ HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\nab",
    );
    assert!(out.starts_with("HTTP/1.1 400"), "{out}");
    assert_eq!(router.connections(), base);
}

#[test]
fn explicit_ports_are_forwarded() {
    let (router, gate, _) = setup();
    let out = send(&gate, "GET http://site.i2p:8080/x HTTP/1.1\r\n\r\n");
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    assert!(
        router
            .lines()
            .contains(&"GET http://site.i2p:8080/x HTTP/1.1".to_owned())
    );
    let zero = send(&gate, "GET http://site.i2p:0/x HTTP/1.1\r\n\r\n");
    assert!(zero.starts_with("HTTP/1.1 403"), "{zero}");
}

// UX1-3: a failed page load must not be saved in history; the gatekeeper reports the failures
// it saw so the shell can tell.

#[test]
fn upstream_5xx_is_a_failure_that_clears_on_read() {
    let (_router, gate, _) = setup();
    let out = send(
        &gate,
        "GET http://down.i2p/ HTTP/1.1\r\nHost: down.i2p\r\n\r\n",
    );
    assert!(out.starts_with("HTTP/1.1 503"), "{out}");
    assert!(
        gate.take_failure("http://down.i2p/"),
        "5xx answer is a failure"
    );
    assert!(
        !gate.take_failure("http://down.i2p/"),
        "the flag clears when read"
    );
}

#[test]
fn a_2xx_answer_is_not_a_failure() {
    let (_router, gate, _) = setup();
    let out = send(
        &gate,
        "GET http://site.i2p/ok HTTP/1.1\r\nHost: site.i2p\r\n\r\n",
    );
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    assert!(!gate.take_failure("http://site.i2p/ok"));
}

#[test]
fn failure_is_kept_per_exact_url() {
    let (_router, gate, _) = setup();
    send(
        &gate,
        "GET http://down.i2p/ HTTP/1.1\r\nHost: down.i2p\r\n\r\n",
    );
    assert!(
        !gate.take_failure("http://down.i2p/other"),
        "another path is another URL"
    );
    assert!(
        !gate.take_failure("http://site.i2p/"),
        "another host is another URL"
    );
    assert!(
        gate.take_failure("http://down.i2p/"),
        "reading other URLs does not clear this one"
    );
}

#[test]
fn a_later_2xx_for_the_same_url_clears_the_failure() {
    let (_router, gate, _) = setup();
    let first = send(
        &gate,
        "GET http://flaky.i2p/ HTTP/1.1\r\nHost: flaky.i2p\r\n\r\n",
    );
    assert!(first.starts_with("HTTP/1.1 503"), "{first}");
    let second = send(
        &gate,
        "GET http://flaky.i2p/ HTTP/1.1\r\nHost: flaky.i2p\r\n\r\n",
    );
    assert!(second.starts_with("HTTP/1.1 200"), "{second}");
    assert!(!gate.take_failure("http://flaky.i2p/"));
}

#[test]
fn bad_gateway_is_a_failure() {
    let (_router, gate, _) = setup();
    let out = send(
        &gate,
        "GET http://cut.i2p/ HTTP/1.1\r\nHost: cut.i2p\r\n\r\n",
    );
    assert!(out.starts_with("HTTP/1.1 502"), "{out}");
    assert!(gate.take_failure("http://cut.i2p/"));
}

#[test]
fn a_bad_request_for_an_i2p_url_is_a_failure() {
    let (_router, gate, _) = setup();
    let out = send(
        &gate,
        "POST http://site.i2p/bad HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\nab",
    );
    assert!(out.starts_with("HTTP/1.1 400"), "{out}");
    assert!(gate.take_failure("http://site.i2p/bad"));
}

#[test]
fn a_length_required_refusal_for_an_i2p_url_is_a_failure() {
    let (_router, gate, _) = setup();
    let out = send(
        &gate,
        "POST http://site.i2p/chunk HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n",
    );
    assert!(out.starts_with("HTTP/1.1 411"), "{out}");
    assert!(gate.take_failure("http://site.i2p/chunk"));
}
