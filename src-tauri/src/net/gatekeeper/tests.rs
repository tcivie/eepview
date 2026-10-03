// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Gatekeeper tests with real loopback sockets and a fake router proxy on `127.0.0.1:0`.

use super::*;
use crate::net::verify::{Verdict, verify};
use std::sync::Mutex;

/// A fake I2P router proxy that counts connections and records request lines.
struct FakeRouter {
    addr: LoopbackAddr,
    connections: Arc<AtomicUsize>,
    lines: Arc<Mutex<Vec<String>>>,
}

impl FakeRouter {
    fn start() -> Self {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        let connections = Arc::new(AtomicUsize::new(0));
        let lines = Arc::new(Mutex::new(Vec::new()));
        let (count, log) = (Arc::clone(&connections), Arc::clone(&lines));
        thread::spawn(move || serve(&listener, &count, &log));
        Self {
            addr,
            connections,
            lines,
        }
    }

    fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    fn verified(&self) -> VerifiedUpstream {
        match verify(self.addr) {
            Verdict::Ok(up) => up,
            other => panic!("fake router failed VERIFY: {other:?}"),
        }
    }
}

fn serve(listener: &TcpListener, count: &Arc<AtomicUsize>, log: &Arc<Mutex<Vec<String>>>) {
    for stream in listener.incoming().flatten() {
        count.fetch_add(1, Ordering::SeqCst);
        let log = Arc::clone(log);
        thread::spawn(move || answer(stream, &log));
    }
}

fn answer(mut stream: TcpStream, log: &Mutex<Vec<String>>) {
    let Ok(Some((head, mut rest))) = read_head(&mut stream) else {
        return;
    };
    read_body(&mut stream, &head, &mut rest);
    log.lock().unwrap().push(head.start.clone());
    let reply: Vec<u8> = match head.start.split(' ').nth(1).unwrap_or("") {
        "http://proxy.i2p/" => b"HTTP/1.1 200 OK\r\n\r\nI2P HTTP proxy OK".to_vec(),
        "http://example.com/" => b"HTTP/1.1 503 No Outproxy Configured\r\n\r\n".to_vec(),
        "tls.i2p:443" => return echo_tunnel(stream),
        _ => {
            let body = format!("hello body={}", String::from_utf8_lossy(&rest));
            format!("HTTP/1.1 200 OK\r\nConnection: keep-alive\r\n\r\n{body}").into_bytes()
        }
    };
    let _ = stream.write_all(&reply);
}

/// Reads the rest of the body: the gatekeeper writes the head and the body separately.
fn read_body(stream: &mut TcpStream, head: &http::Head, rest: &mut Vec<u8>) {
    let length = head.body_length().unwrap_or(0);
    let missing = length.saturating_sub(rest.len() as u64);
    let _ = stream.take(missing).read_to_end(rest);
}

fn echo_tunnel(mut stream: TcpStream) {
    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n");
    let mut buf = [0u8; 64];
    if let Ok(n) = stream.read(&mut buf) {
        let _ = stream.write_all(&buf[..n]);
    }
}

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
