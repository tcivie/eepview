// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests of the gatekeeper timing and close rules, with real loopback sockets.
//! They use the public API only.
//!
//! Source: ADR 0001 (`docs/wiki/adr-0001-no-leak-architecture.md`), the sections "Gatekeeper
//! answers" and "Gatekeeper timing and close":
//!
//! - Dead router: the connect to the router proxy times out after 500 ms, and the client gets the
//!   502 within 1 s on every OS.
//! - Early answers (the busy 503, and a 403, 400 or 411 sent while request bytes are still
//!   unread): the gatekeeper shuts down its write side, reads and discards the unread input until
//!   the client closes, 1 s passes, or 64 KiB have been read, then closes the socket. The client
//!   gets the full answer, never a reset, and one connection costs at most 1 s and 64 KiB.
//!
//! The busy 503 cannot be triggered from here: the ADR names no connection limit and the public
//! API has no knob for it, so only the 403, 400 and 411 cases are covered.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use eepview_lib::net::gatekeeper::Gatekeeper;
use eepview_lib::net::loopback::LoopbackAddr;
use eepview_lib::net::verify::{Verdict, VerifiedUpstream, verify};

/// A generous client timeout: the early-answer tests check delivery, not speed.
const READ_LIMIT: Duration = Duration::from_secs(5);
/// ADR: "the client gets the 502 within 1 s".
const DEAD_ROUTER_BOUND: Duration = Duration::from_secs(1);
/// Slack for a loaded test machine.
const MARGIN: Duration = Duration::from_millis(250);
/// ADR: the drain ends after 1 s at the latest.
const DRAIN_TIME: Duration = Duration::from_secs(1);
/// Slack for the drain tests (the OS needs a moment to report the close to the writer).
const DRAIN_MARGIN: Duration = Duration::from_millis(750);
/// A client that floods the gatekeeper reaches the 64 KiB limit long before the 1 s limit.
const FLOOD_BOUND: Duration = Duration::from_millis(600);
/// A reset depends on timing, so each early answer is checked many times.
const REPEATS: usize = 20;
/// An unread request body of 32 KiB.
const BODY_SIZE: usize = 32 * 1024;

#[cfg(test)]
mod fake {
    use super::*;

    /// A fake router proxy that answers VERIFY, and that can go away: after `stop` nothing listens on
    /// its port, so a connect there is refused (the same as a connect to `127.0.0.1:1`).
    pub struct StubRouter {
        addr: LoopbackAddr,
        stop: Arc<AtomicBool>,
        worker: Option<JoinHandle<()>>,
    }

    impl StubRouter {
        pub fn start() -> Self {
            let (listener, addr) = LoopbackAddr::listen_any().unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&stop);
            let worker = thread::spawn(move || serve(&listener, &flag));
            Self {
                addr,
                stop,
                worker: Some(worker),
            }
        }

        pub fn verified(&self) -> VerifiedUpstream {
            match verify(self.addr) {
                Verdict::Ok(up) => up,
                other => panic!("the stub router failed VERIFY: {other:?}"),
            }
        }

        /// Closes the listener and waits until it is gone.
        pub fn stop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let worker = self
                .worker
                .take()
                .expect("the stub router was stopped twice");
            worker.join().unwrap();
        }
    }

    fn serve(listener: &TcpListener, stop: &AtomicBool) {
        while !stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => drop(thread::spawn(move || answer_verify(stream))),
                Err(_) => thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    fn answer_verify(mut stream: TcpStream) {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut seen = Vec::new();
        let mut chunk = [0u8; 1024];
        while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut chunk) {
                Ok(n) if n > 0 => seen.extend_from_slice(&chunk[..n]),
                _ => return,
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nI2P HTTP proxy OK");
    }

    pub struct Fixture {
        router: StubRouter,
        gate: Gatekeeper,
    }

    impl Fixture {
        pub fn new() -> Self {
            let router = StubRouter::start();
            let gate = Gatekeeper::start(&router.verified()).unwrap();
            Self { router, gate }
        }

        /// The router is verified, the gatekeeper runs, and then the router goes away.
        pub fn with_dead_router() -> Self {
            let mut fx = Self::new();
            fx.router.stop();
            fx
        }

        pub fn connect(&self, limit: Duration) -> TcpStream {
            let addr = LoopbackAddr::parse(&self.gate.url()).unwrap();
            let stream = addr.connect(Duration::from_secs(2)).unwrap();
            stream.set_read_timeout(Some(limit)).unwrap();
            stream.set_write_timeout(Some(limit)).unwrap();
            stream
        }
    }
}

use fake::Fixture;

/// Everything the client read, and the error that ended the read (None for a normal end).
struct Reply {
    bytes: Vec<u8>,
    error: Option<io::ErrorKind>,
}

impl Reply {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    fn status(&self) -> Option<u16> {
        self.text()
            .strip_prefix("HTTP/1.1 ")?
            .get(..3)?
            .parse()
            .ok()
    }

    /// The head is whole, and the body is as long as the `Content-Length` says, when there is one.
    fn complete(&self) -> bool {
        let Some(end) = self.bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
            return false;
        };
        let head = String::from_utf8_lossy(&self.bytes[..end]).into_owned();
        let length = head.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        });
        length.is_none_or(|n| self.bytes.len() - (end + 4) == n)
    }
}

fn read_reply(stream: &mut TcpStream) -> Reply {
    let mut bytes = Vec::new();
    let error = stream.read_to_end(&mut bytes).err().map(|e| e.kind());
    Reply { bytes, error }
}

/// The client got the whole answer, with the status, and the read did not fail.
fn assert_delivered(reply: &Reply, status: u16, label: &str) {
    assert_eq!(
        reply.error,
        None,
        "{label}: the read failed, so the answer was lost or never ended; got {:?}",
        reply.text()
    );
    assert_eq!(reply.status(), Some(status), "{label}: {:?}", reply.text());
    assert!(
        reply.complete(),
        "{label}: a cut answer: {:?}",
        reply.text()
    );
}

// Req: ADR "Dead router": with no router behind the gatekeeper, the client gets the 502 within
// 1 s, measured from the request write to the end of the answer. Repeated, since the first
// connect and the later ones can differ.
#[test]
fn dead_router_gets_a_502_within_one_second() {
    let fx = Fixture::with_dead_router();
    for round in 0..5 {
        let mut stream = fx.connect(READ_LIMIT);
        let started = Instant::now();
        stream
            .write_all(b"GET http://site.i2p/ HTTP/1.1\r\nHost: site.i2p\r\n\r\n")
            .unwrap();
        let _ = stream.shutdown(Shutdown::Write);
        let reply = read_reply(&mut stream);
        let elapsed = started.elapsed();
        assert_delivered(&reply, 502, &format!("round {round}"));
        assert!(
            elapsed < DEAD_ROUTER_BOUND + MARGIN,
            "round {round}: the 502 took {elapsed:?}, the bound is {DEAD_ROUTER_BOUND:?}"
        );
    }
}

fn filler(len: usize) -> Vec<u8> {
    vec![b'x'; len]
}

fn with_body(head: &str, body: &[u8]) -> Vec<u8> {
    let mut raw = head.as_bytes().to_vec();
    raw.extend_from_slice(body);
    raw
}

/// Sends `raw` in one write and reads the answer to its end with a generous timeout. The client
/// closes its sending side in every second round, and keeps it open in the others.
fn check_early_answer(label: &str, status: u16, raw: &[u8]) {
    let fx = Fixture::new();
    for round in 0..REPEATS {
        let mut stream = fx.connect(READ_LIMIT);
        // A write that fails because the gatekeeper already closed is not the subject here.
        let _ = stream.write_all(raw);
        if round % 2 == 1 {
            let _ = stream.shutdown(Shutdown::Write);
        }
        let reply = read_reply(&mut stream);
        assert_delivered(&reply, status, &format!("{label}, round {round}"));
    }
}

// Req: ADR "Early answers reach the client", "A non-.i2p host gets 403": a 403 sent while the
// 32 KiB request body is unread reaches the client whole, with no reset.
#[test]
fn early_403_for_a_clearnet_host_with_an_unread_body_reaches_the_client() {
    let raw = with_body(
        &format!(
            "POST http://example.com/ HTTP/1.1\r\nHost: example.com\r\nContent-Length: {BODY_SIZE}\r\n\r\n"
        ),
        &filler(BODY_SIZE),
    );
    check_early_answer("403 clearnet host", 403, &raw);
}

// Req: ADR "CONNECT stays limited to :80 and :443": the 403 for a refused CONNECT port, with 32 KiB
// of tunnel bytes behind the head unread, reaches the client whole.
#[test]
fn early_403_for_a_refused_connect_port_with_unread_bytes_reaches_the_client() {
    let raw = with_body("CONNECT site.i2p:22 HTTP/1.1\r\n\r\n", &filler(BODY_SIZE));
    check_early_answer("403 CONNECT port 22", 403, &raw);
}

// Req: ADR "A malformed request 400 (two Content-Length headers count as malformed)": the 400
// with a 32 KiB body unread reaches the client whole.
#[test]
fn early_400_for_two_content_lengths_with_an_unread_body_reaches_the_client() {
    let raw = with_body(
        &format!(
            "POST http://site.i2p/ HTTP/1.1\r\nHost: site.i2p\r\nContent-Length: {BODY_SIZE}\r\ncontent-length: {BODY_SIZE}\r\n\r\n"
        ),
        &filler(BODY_SIZE),
    );
    check_early_answer("400 duplicate Content-Length", 400, &raw);
}

// Req: ADR "A malformed request 400": a request line that is not HTTP, with 32 KiB behind the
// head unread, gets the 400 whole.
#[test]
fn early_400_for_a_malformed_request_line_with_unread_bytes_reaches_the_client() {
    let raw = with_body("this is not http\r\n\r\n", &filler(BODY_SIZE));
    check_early_answer("400 malformed request line", 400, &raw);
}

// Req: ADR "a chunked request body 411": the 411, with 32 KiB of chunk data unread, reaches the
// client whole.
#[test]
fn early_411_for_a_chunked_body_with_unread_chunks_reaches_the_client() {
    let mut raw = with_body(
        "POST http://site.i2p/ HTTP/1.1\r\nHost: site.i2p\r\nTransfer-Encoding: chunked\r\n\r\n8000\r\n",
        &filler(BODY_SIZE),
    );
    raw.extend_from_slice(b"\r\n0\r\n\r\n");
    check_early_answer("411 chunked", 411, &raw);
}

/// A head the gatekeeper refuses (403) and a body it never reads.
fn refused_head() -> &'static str {
    "POST http://example.com/ HTTP/1.1\r\nHost: example.com\r\nContent-Length: 100000000\r\n\r\n"
}

/// Writes `chunk` every `pause` until a write fails because the peer closed the socket, and says
/// how long after `started` that happened. None: no close within `give_up`. A write that only
/// times out means the gatekeeper stopped reading but kept the socket, which is not a close.
fn closed_after(
    stream: &mut TcpStream,
    started: Instant,
    chunk: &[u8],
    pause: Duration,
) -> Option<Duration> {
    let give_up = started + Duration::from_secs(4);
    while Instant::now() < give_up {
        match stream.write_all(chunk) {
            Ok(()) => thread::sleep(pause),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return None;
            }
            Err(_) => return Some(started.elapsed()),
        }
    }
    None
}

/// Sends the refused request, then stays and probes with `chunk` every `pause`.
fn probe_after_refusal(chunk: &[u8], pause: Duration, bound: Duration, label: &str) {
    let fx = Fixture::new();
    for round in 0..3 {
        let mut stream = fx.connect(Duration::from_millis(500));
        let started = Instant::now();
        let _ = stream.write_all(&with_body(refused_head(), &filler(BODY_SIZE)));
        let closed = closed_after(&mut stream, started, chunk, pause);
        assert!(
            closed.is_some_and(|t| t < bound),
            "{label}, round {round}: the gatekeeper did not close within {bound:?} (closed after {closed:?})"
        );
    }
}

// Req: ADR "reads and discards the unread input, until the client closes, 1 s passes, or 64 KiB
// have been read": a client that sends its request and then goes silent, and never closes, does
// not hold the connection past 1 s. The server side closes the socket (a later write fails).
#[test]
fn a_silent_client_that_never_closes_is_dropped_after_one_second() {
    probe_after_refusal(
        b"x",
        Duration::from_millis(50),
        DRAIN_TIME + DRAIN_MARGIN,
        "silent client",
    );
}

// Req: same, for a client that trickles input slowly and never reaches 64 KiB in 1 s: the 1 s
// limit still ends the drain, the trickle does not extend it.
#[test]
fn a_trickling_client_does_not_extend_the_drain_past_one_second() {
    probe_after_refusal(
        &filler(512),
        Duration::from_millis(100),
        DRAIN_TIME + DRAIN_MARGIN,
        "trickling client",
    );
}

// Req: same, for a client that keeps sending past the byte limit: the drain stops at 64 KiB, long
// before 1 s, and the gatekeeper closes the socket.
#[test]
fn a_flooding_client_is_dropped_at_the_byte_limit_long_before_one_second() {
    probe_after_refusal(
        &filler(8 * 1024),
        Duration::ZERO,
        FLOOD_BOUND,
        "flooding client",
    );
}
