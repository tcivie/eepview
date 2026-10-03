// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Test helpers with sockets, so tests outside `net/` never open one themselves (ADR 0001
//! rule 2): a fake I2P router proxy and a fake router console on `127.0.0.1:0`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use super::console::{ConsoleKind, VerifiedConsole as Verified, probe};
use super::http::{self, Head};
use super::loopback::LoopbackAddr;
use super::verify::{Verdict, VerifiedUpstream, verify};

/// A fake I2P router proxy that counts connections and records request lines.
pub struct FakeRouter {
    /// Where it listens.
    pub addr: LoopbackAddr,
    connections: Arc<AtomicUsize>,
    lines: Arc<Mutex<Vec<String>>>,
}

impl FakeRouter {
    /// Starts the fake router on a free loopback port.
    pub fn start() -> Self {
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

    /// The connections accepted so far.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// The request lines received so far.
    pub fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    /// The router after a passing VERIFY.
    pub fn verified(&self) -> VerifiedUpstream {
        match verify(self.addr) {
            Verdict::Ok(up) => up,
            other => panic!("fake router failed VERIFY: {other:?}"),
        }
    }
}

/// A loopback address where nothing listens: port 1 (tcpmux) is never used today, and no
/// test can bind it, so a parallel test cannot take it over the way it can a freed port.
pub fn dead_addr() -> LoopbackAddr {
    LoopbackAddr::parse("127.0.0.1:1").unwrap()
}

fn serve(listener: &TcpListener, count: &Arc<AtomicUsize>, log: &Arc<Mutex<Vec<String>>>) {
    for stream in listener.incoming().flatten() {
        count.fetch_add(1, Ordering::SeqCst);
        let log = Arc::clone(log);
        thread::spawn(move || answer(stream, &log));
    }
}

fn answer(mut stream: TcpStream, log: &Mutex<Vec<String>>) {
    let Some((head, mut rest)) = read_head(&mut stream) else {
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

/// Reads one request head and the bytes after it.
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

/// Reads the rest of the body: the gatekeeper writes the head and the body separately.
fn read_body(stream: &mut TcpStream, head: &Head, rest: &mut Vec<u8>) {
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

/// A fake router console of one type on a free loopback port. It answers every `GET` with
/// a page that carries that console's marker, and records the request lines.
pub struct FakeConsole {
    kind: ConsoleKind,
    addr: LoopbackAddr,
    lines: Arc<Mutex<Vec<String>>>,
}

impl FakeConsole {
    /// Starts the fake console.
    pub fn start(kind: ConsoleKind) -> Self {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        let lines = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&lines);
        thread::spawn(move || serve_console(&listener, kind, &log));
        Self { kind, addr, lines }
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// The console after a passing probe.
    pub fn verified(&self) -> Verified {
        probe(self.kind, self.port()).expect("fake console failed the probe")
    }

    /// The request lines received so far.
    pub fn requests(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }
}

fn serve_console(listener: &TcpListener, kind: ConsoleKind, log: &Mutex<Vec<String>>) {
    for stream in listener.incoming().flatten() {
        console_answer(stream, kind, log);
    }
}

fn console_answer(mut stream: TcpStream, kind: ConsoleKind, log: &Mutex<Vec<String>>) {
    let Some((head, _)) = read_head(&mut stream) else {
        return;
    };
    log.lock().unwrap().push(head.start.clone());
    let body = match kind {
        ConsoleKind::Java => {
            "<html><head><link href=\"/themes/console/light/console.css?2.13.0\"></head></html>"
        }
        ConsoleKind::I2pd => {
            "<html><body><b>Version:</b> 2.13.0<br><a href=\"/?page=i2p_tunnels\">I2P tunnels</a></body></html>"
        }
    };
    let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n{body}");
    let _ = stream.write_all(reply.as_bytes());
}
