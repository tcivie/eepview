// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! A scripted fake I2P router proxy for the site icon tests, and a real gatekeeper in front
//! of it. The crate-private `net::testing::FakeRouter` is out of reach for an integration
//! test, and it cannot send the odd answers these tests need.
//!
//! The router counts the connections it accepts and records every request head it reads.
//! A `Reply` says what it sends back for one head.

use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eepview_lib::net::gatekeeper::Gatekeeper;
use eepview_lib::net::loopback::LoopbackAddr;
use eepview_lib::net::verify::{Verdict, judge};

/// What the router sends for one request head.
#[derive(Default)]
pub struct Reply {
    /// The raw bytes to send.
    pub bytes: Vec<u8>,
    /// Keep the connection open and silent after the bytes, instead of closing it.
    pub stall: bool,
}

/// How long a stalled connection stays open at most.
const STALL_MAX: Duration = Duration::from_secs(20);

type Script = Arc<dyn Fn(&str) -> Reply + Send + Sync>;

/// The fake router.
pub struct Router {
    addr: LoopbackAddr,
    connections: Arc<AtomicUsize>,
    heads: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
}

impl Router {
    /// Starts the router on a free loopback port. `script` gets every request head.
    pub fn start(
        script: impl Fn(&str) -> Reply + Send + Sync + 'static,
    ) -> Result<Self, Box<dyn Error>> {
        let (listener, addr) = LoopbackAddr::listen_any()?;
        let connections = Arc::new(AtomicUsize::new(0));
        let heads = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let script: Script = Arc::new(script);
        let (count, log, halt) = (
            Arc::clone(&connections),
            Arc::clone(&heads),
            Arc::clone(&stop),
        );
        thread::spawn(move || accept_loop(&listener, &count, &log, &halt, &script));
        Ok(Self {
            addr,
            connections,
            heads,
            stop,
        })
    }

    /// The connections accepted so far.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// The request heads read so far, each without the final blank line.
    pub fn heads(&self) -> Vec<String> {
        self.heads
            .lock()
            .map_or_else(|_| Vec::new(), |heads| heads.clone())
    }

    /// A gatekeeper in front of this router, as after a passing VERIFY.
    pub fn gatekeeper(&self) -> Result<Gatekeeper, Box<dyn Error>> {
        let verdict = judge(self.addr, Ok((200, "I2P HTTP proxy OK".to_owned())));
        match verdict {
            Verdict::Ok(upstream) => Ok(Gatekeeper::start(&upstream)?),
            other => Err(format!("VERIFY did not pass: {other:?}").into()),
        }
    }
}

impl Drop for Router {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the stop flag.
        let _ = self.addr.connect(Duration::from_millis(200));
    }
}

/// Accepts connections until the stop flag is set.
fn accept_loop(
    listener: &TcpListener,
    count: &AtomicUsize,
    log: &Arc<Mutex<Vec<String>>>,
    halt: &Arc<AtomicBool>,
    script: &Script,
) {
    for stream in listener.incoming() {
        if halt.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        count.fetch_add(1, Ordering::SeqCst);
        let (log, halt, script) = (Arc::clone(log), Arc::clone(halt), Arc::clone(script));
        thread::spawn(move || handle(stream, &log, &halt, &script));
    }
}

/// Reads one head, records it, answers it.
fn handle(mut stream: TcpStream, log: &Mutex<Vec<String>>, halt: &AtomicBool, script: &Script) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Some(head) = read_head(&mut stream) else {
        return;
    };
    if let Ok(mut heads) = log.lock() {
        heads.push(head.clone());
    }
    let reply = script(&head);
    let _ = stream.write_all(&reply.bytes);
    let _ = stream.flush();
    let started = Instant::now();
    while reply.stall && !halt.load(Ordering::SeqCst) && started.elapsed() < STALL_MAX {
        thread::sleep(Duration::from_millis(20));
    }
}

/// The request head up to the blank line, or `None` when the client sent nothing useful.
fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut seen = Vec::new();
    let mut byte = [0_u8; 1];
    while !seen.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => seen.push(byte[0]),
            _ => return None,
        }
        if seen.len() > 64 * 1024 {
            return None;
        }
    }
    seen.truncate(seen.len() - 4);
    Some(String::from_utf8_lossy(&seen).into_owned())
}
