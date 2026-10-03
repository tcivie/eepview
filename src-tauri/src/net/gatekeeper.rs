// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The gatekeeper (ADR 0001, layer L1): eepview's own HTTP proxy on `127.0.0.1:<random>`.
//!
//! Every content webview uses it as its proxy. It forwards a request to the verified router
//! proxy only when the host passes [`super::host::is_i2p_host`], and answers everything else
//! itself with no upstream connection. It forwards one request per connection, adds the page
//! policy (L3) to every response, and never pipes raw bytes from the engine to the router
//! except inside a `CONNECT <name>.i2p:443` TLS tunnel.

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use super::http::{self, Head, Plan, Refusal};
use super::loopback::LoopbackAddr;
use super::verify::VerifiedUpstream;
use crate::diag::{self, Code, ErrorKind, Field};

/// Most connections handled at once.
const MAX_CONNECTIONS: usize = 256;
/// Wait for a request head from the engine.
const CLIENT_TIMEOUT: Duration = Duration::from_mins(1);
/// Wait for the router: I2P tunnels can take minutes on a cold start.
const UPSTREAM_TIMEOUT: Duration = Duration::from_mins(5);
/// Whether `CONNECT *.i2p:443` is relayed (see [`Gatekeeper::start`]).
const TLS_TUNNELS: bool = cfg!(windows);
const ESTABLISHED: &[u8] = b"HTTP/1.1 200 Connection established\r\n\r\n";

/// A running gatekeeper. Only [`Gatekeeper::start`] builds one; dropping it closes it.
#[derive(Debug)]
pub struct Gatekeeper {
    addr: LoopbackAddr,
    open: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
struct Shared {
    upstream: LoopbackAddr,
    open: Arc<AtomicBool>,
    active: Arc<AtomicUsize>,
    limit: usize,
    allow_tls: bool,
}

impl Gatekeeper {
    /// Starts the gatekeeper in front of a verified router proxy.
    ///
    /// TLS tunnels (`CONNECT *.i2p:443`) open only where the engine has its own request filter
    /// (L3b), that is on Windows. Relayed TLS responses cannot carry the page policy (L3a), so
    /// on macOS (`WKWebView` skips the proxy for loopback, spike S1) and on Linux (no engine
    /// filter) they stay closed: fail closed.
    ///
    /// # Errors
    ///
    /// Fails when no loopback port can be bound.
    pub fn start(upstream: &VerifiedUpstream) -> io::Result<Self> {
        Self::start_with(upstream, TLS_TUNNELS)
    }

    fn start_with(upstream: &VerifiedUpstream, allow_tls: bool) -> io::Result<Self> {
        Self::start_limited(upstream, allow_tls, MAX_CONNECTIONS)
    }

    fn start_limited(
        upstream: &VerifiedUpstream,
        allow_tls: bool,
        limit: usize,
    ) -> io::Result<Self> {
        let (listener, addr) = LoopbackAddr::listen_any()?;
        let open = Arc::new(AtomicBool::new(true));
        let shared = Shared {
            upstream: upstream.addr(),
            open: Arc::clone(&open),
            active: Arc::new(AtomicUsize::new(0)),
            limit,
            allow_tls,
        };
        thread::Builder::new()
            .name("gatekeeper".into())
            .spawn(move || accept_loop(&listener, &shared))?;
        Ok(Self { addr, open })
    }

    /// `http://127.0.0.1:<port>`, the proxy URL for content webviews.
    #[must_use]
    pub fn url(&self) -> String {
        self.addr.http_url()
    }

    /// Stops accepting and refuses every request still in flight.
    pub fn close(&self) {
        if self.open.swap(false, Ordering::SeqCst) {
            // Wake the accept loop so it sees the flag and drops the listener.
            let _ = self.addr.connect(Duration::from_millis(200));
        }
    }
}

impl Drop for Gatekeeper {
    fn drop(&mut self) {
        self.close();
    }
}

/// Waits after failed `accept` calls: 50 ms, doubling up to 1 s, back to the start after a
/// success. A persistent error (such as no free file descriptors) then costs no CPU.
#[derive(Debug)]
struct Backoff {
    next: Duration,
}

impl Backoff {
    const FIRST: Duration = Duration::from_millis(50);
    const MAX: Duration = Duration::from_secs(1);

    fn new() -> Self {
        Self { next: Self::FIRST }
    }

    /// The wait after one more failure.
    fn fail(&mut self) -> Duration {
        let wait = self.next;
        self.next = (self.next * 2).min(Self::MAX);
        wait
    }

    fn reset(&mut self) {
        self.next = Self::FIRST;
    }
}

fn accept_loop(listener: &TcpListener, shared: &Shared) {
    let mut backoff = Backoff::new();
    for stream in listener.incoming() {
        if !shared.open.load(Ordering::SeqCst) {
            return;
        }
        let Ok(stream) = stream else {
            thread::sleep(backoff.fail());
            continue;
        };
        backoff.reset();
        if shared.active.fetch_add(1, Ordering::SeqCst) >= shared.limit {
            shared.active.fetch_sub(1, Ordering::SeqCst);
            let mut stream = stream;
            refused(Refusal::Busy);
            let _ = stream.write_all(&Refusal::Busy.response());
            continue;
        }
        let active = Arc::clone(&shared.active);
        let shared = shared.clone();
        let spawned = thread::Builder::new().spawn(move || {
            serve(stream, &shared);
            shared.active.fetch_sub(1, Ordering::SeqCst);
        });
        if spawned.is_err() {
            active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Handles one connection; a failed one records `page-load-failed` with its error kind.
fn serve(client: TcpStream, shared: &Shared) {
    if let Err(e) = handle(client, shared) {
        diag::event(Code::PageLoadFailed, &[Field::Error(ErrorKind::from(&e))]);
    }
}

fn handle(mut client: TcpStream, shared: &Shared) -> io::Result<()> {
    client.set_read_timeout(Some(CLIENT_TIMEOUT))?;
    client.set_write_timeout(Some(UPSTREAM_TIMEOUT))?;
    let Some((head, rest)) = read_head(&mut client)? else {
        return refuse(&mut client, Refusal::BadRequest);
    };
    if !shared.open.load(Ordering::SeqCst) {
        return refuse(&mut client, Refusal::Upstream);
    }
    match http::plan(&head, shared.allow_tls) {
        Plan::Http { method, host, path } => {
            let request = Request {
                method: &method,
                host: &host,
                path: &path,
                head: &head,
            };
            forward(&mut client, &rest, &request, shared)
        }
        Plan::Terminate { host } => terminate(&mut client, &rest, &host, shared),
        Plan::Relay { host } => relay(client, &rest, &host, shared),
        Plan::Refuse(reason) => refuse(&mut client, reason),
    }
}

/// Records a refusal by its reason kind only.
fn refused(reason: Refusal) {
    diag::event(Code::GatekeeperRefused, &[Field::Refuse(reason.reason())]);
}

/// Records a router error page (5xx) by its status code only.
fn upstream_status(head: &Head) {
    if let Some(status) = http::status_code(&head.start).filter(|s| *s >= 500) {
        diag::event(Code::PageLoadFailed, &[Field::Status(status)]);
    }
}

fn refuse(client: &mut TcpStream, reason: Refusal) -> io::Result<()> {
    refused(reason);
    client.write_all(&reason.response())?;
    client.flush()
}

/// Reads one message head. `None` on a clean EOF, a broken head or a head over the limit.
fn read_head(stream: &mut impl Read) -> io::Result<Option<(Head, Vec<u8>)>> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(end) = http::head_end(&buf) {
            let rest = buf.split_off(end);
            return Ok(Head::parse(&buf).map(|h| (h, rest)));
        }
        if buf.len() > http::MAX_HEAD {
            return Ok(None);
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// `CONNECT <host>:80`: confirm, then read exactly one plain request from the tunnel.
fn terminate(client: &mut TcpStream, early: &[u8], host: &str, shared: &Shared) -> io::Result<()> {
    client.write_all(ESTABLISHED)?;
    let mut source = early.chain(&mut *client);
    let head = read_head(&mut source)?;
    let (unread, _) = source.into_inner();
    let Some((head, mut rest)) = head else {
        return Ok(());
    };
    rest.extend_from_slice(unread);
    match http::plan_inner(&head, host) {
        Plan::Http { method, host, path } => {
            let request = Request {
                method: &method,
                host: &host,
                path: &path,
                head: &head,
            };
            forward(client, &rest, &request, shared)
        }
        Plan::Refuse(reason) => refuse(client, reason),
        _ => refuse(client, Refusal::BadRequest),
    }
}

/// One plain HTTP request, checked by [`http::plan`] or [`http::plan_inner`].
struct Request<'a> {
    method: &'a str,
    host: &'a str,
    path: &'a str,
    head: &'a Head,
}

fn forward(client: &mut TcpStream, rest: &[u8], req: &Request, shared: &Shared) -> io::Result<()> {
    let length = match req.head.body_length() {
        Ok(n) => n,
        Err(reason) => return refuse(client, reason),
    };
    let Ok(mut upstream) = shared.upstream.connect(UPSTREAM_TIMEOUT) else {
        return refuse(client, Refusal::Upstream);
    };
    upstream.write_all(&http::upstream_request(
        req.method, req.host, req.path, req.head,
    ))?;
    send_body(client, rest, length, &mut upstream)?;
    let Some((head, body)) = read_head(&mut upstream)? else {
        return refuse(client, Refusal::Upstream);
    };
    upstream_status(&head);
    client.write_all(&http::rewrite_response(head))?;
    client.write_all(&body)?;
    io::copy(&mut upstream, client)?;
    client.flush()
}

/// Sends exactly `length` body bytes: first those already read, then the rest from `client`.
fn send_body(
    client: &mut TcpStream,
    rest: &[u8],
    length: u64,
    upstream: &mut TcpStream,
) -> io::Result<()> {
    let have = usize::try_from(length).map_or(rest.len(), |n| n.min(rest.len()));
    upstream.write_all(&rest[..have])?;
    let missing = length.saturating_sub(have as u64);
    let copied = io::copy(&mut client.take(missing), upstream)?;
    if copied < missing {
        // The client ended before the body did: close both sides now, do not wait for the
        // router to time out.
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "request body cut short",
        ));
    }
    Ok(())
}

/// `CONNECT <host>:443`: open the same tunnel through the router and relay the TLS bytes.
fn relay(mut client: TcpStream, rest: &[u8], host: &str, shared: &Shared) -> io::Result<()> {
    let Ok(mut upstream) = shared.upstream.connect(UPSTREAM_TIMEOUT) else {
        return refuse(&mut client, Refusal::Upstream);
    };
    upstream.write_all(&http::upstream_connect(host))?;
    let Some((head, early)) = read_head(&mut upstream)? else {
        return refuse(&mut client, Refusal::Upstream);
    };
    if !http::is_success(&head.start) {
        return refuse(&mut client, Refusal::Upstream);
    }
    client.write_all(ESTABLISHED)?;
    client.write_all(&early)?;
    upstream.write_all(rest)?;
    pipe(client, upstream)
}

fn pipe(client: TcpStream, upstream: TcpStream) -> io::Result<()> {
    let mut client_read = client.try_clone()?;
    let mut upstream_write = upstream.try_clone()?;
    let outbound = thread::Builder::new().spawn(move || {
        let _ = io::copy(&mut client_read, &mut upstream_write);
        let _ = upstream_write.shutdown(Shutdown::Write);
    })?;
    let (mut upstream_read, mut client_write) = (upstream, client);
    let _ = io::copy(&mut upstream_read, &mut client_write);
    let _ = client_write.shutdown(Shutdown::Both);
    let _ = outbound.join();
    Ok(())
}

#[cfg(test)]
mod tests;
