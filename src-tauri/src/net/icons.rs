// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Site icons: one `GET http://<host>/favicon.ico` through the gatekeeper
//! (`docs/wiki/site-icons.md`).
//!
//! The request goes only to the [`Gatekeeper`] on loopback, which checks the host like every
//! page request. The head is fixed: no cookie, no referrer, no user agent. The answer must be
//! a plain 200 with at most [`MAX_BODY`] bytes, within [`TIMEOUT`] in total. This module
//! returns the raw body; `crate::icons::sanitize` decides whether it is an image.

use std::fmt;
use std::io::{self, ErrorKind, Read, Write};
use std::time::{Duration, Instant};

use super::gatekeeper::Gatekeeper;
use super::host::is_i2p_host;
use super::http::{self, Head};
use super::loopback::ReadTimeout;

/// The only path eepview asks for.
pub const PATH: &str = "/favicon.ico";
/// Most body bytes accepted.
pub const MAX_BODY: usize = 64 * 1024;
/// The whole fetch must end within this time.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// The limits of one fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Total time for connect, request and answer.
    pub timeout: Duration,
    /// Most body bytes.
    pub max_body: usize,
}

impl Limits {
    /// [`TIMEOUT`] and [`MAX_BODY`].
    pub const DEFAULT: Self = Self {
        timeout: TIMEOUT,
        max_body: MAX_BODY,
    };
}

/// Why a fetch gave no body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The host is not an I2P host: no connection was opened.
    NotI2p,
    /// Connect, read or write failed.
    Io(String),
    /// The status was not 200.
    Status(u16),
    /// The head or the body is over the limit.
    TooLarge,
    /// The answer has a `Transfer-Encoding`.
    Chunked,
    /// Not HTTP, or a body shorter than its `Content-Length`.
    Malformed,
    /// The fetch took longer than the timeout.
    Timeout,
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotI2p => f.write_str("not an I2P host"),
            Self::Io(e) => write!(f, "connection failed: {e}"),
            Self::Status(code) => write!(f, "the site answered {code}"),
            Self::TooLarge => f.write_str("the answer is too large"),
            Self::Chunked => f.write_str("the answer has a transfer encoding"),
            Self::Malformed => f.write_str("the answer is not valid HTTP"),
            Self::Timeout => f.write_str("the site did not answer in time"),
        }
    }
}

impl std::error::Error for FetchError {}

/// The exact request bytes for `host`, or `None` when it is not an I2P host.
#[must_use]
pub fn request(host: &str) -> Option<Vec<u8>> {
    is_i2p_host(host).then(|| {
        format!(
            "GET http://{host}{PATH} HTTP/1.1\r\nHost: {host}\r\nAccept: image/*\r\n\
             Connection: close\r\n\r\n"
        )
        .into_bytes()
    })
}

/// One fetch through the gatekeeper with [`Limits::DEFAULT`]. Returns the raw body.
///
/// # Errors
///
/// See [`FetchError`].
pub fn fetch(gate: &Gatekeeper, host: &str) -> Result<Vec<u8>, FetchError> {
    fetch_with(gate, host, Limits::DEFAULT)
}

/// One fetch through the gatekeeper with other limits.
///
/// # Errors
///
/// See [`FetchError`].
pub fn fetch_with(gate: &Gatekeeper, host: &str, limits: Limits) -> Result<Vec<u8>, FetchError> {
    let bytes = request(host).ok_or(FetchError::NotI2p)?;
    let deadline = Instant::now() + limits.timeout;
    let mut stream = gate
        .addr()
        .connect(limits.timeout)
        .map_err(|e| failed(&e))?;
    stream.write_all(&bytes).map_err(|e| failed(&e))?;
    let mut answer = Timed { stream, deadline };
    let (head, rest) = read_head(&mut answer)?;
    let length = check_head(&head, limits.max_body)?;
    read_body(&mut answer, rest, length, limits.max_body)
}

/// The body length the head announces, once the head is acceptable.
fn check_head(head: &Head, max_body: usize) -> Result<Option<usize>, FetchError> {
    match http::status_code(&head.start) {
        Some(200) => {}
        Some(code) => return Err(FetchError::Status(code)),
        None => return Err(FetchError::Malformed),
    }
    if head.header("transfer-encoding").is_some() {
        return Err(FetchError::Chunked);
    }
    let Some(text) = head.header("content-length") else {
        return Ok(None);
    };
    let length: usize = text.parse().map_err(|_| FetchError::Malformed)?;
    if length > max_body {
        return Err(FetchError::TooLarge);
    }
    Ok(Some(length))
}

/// Reads the body after `rest`: exactly `length` bytes, or up to the end when unknown.
fn read_body(
    answer: &mut impl Read,
    mut body: Vec<u8>,
    length: Option<usize>,
    max_body: usize,
) -> Result<Vec<u8>, FetchError> {
    let want = length.unwrap_or(max_body.saturating_add(1));
    let missing = want.saturating_sub(body.len()) as u64;
    answer
        .take(missing)
        .read_to_end(&mut body)
        .map_err(|e| failed(&e))?;
    match length {
        Some(n) if body.len() < n => Err(FetchError::Malformed),
        Some(n) => {
            body.truncate(n);
            Ok(body)
        }
        None if body.len() > max_body => Err(FetchError::TooLarge),
        None => Ok(body),
    }
}

/// Reads the response head and the bytes after it.
fn read_head(answer: &mut impl Read) -> Result<(Head, Vec<u8>), FetchError> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(end) = http::head_end(&buf) {
            let rest = buf.split_off(end);
            return Head::parse(&buf)
                .map(|h| (h, rest))
                .ok_or(FetchError::Malformed);
        }
        if buf.len() > http::MAX_HEAD {
            return Err(FetchError::TooLarge);
        }
        let n = answer.read(&mut chunk).map_err(|e| failed(&e))?;
        if n == 0 {
            return Err(FetchError::Malformed);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn failed(e: &io::Error) -> FetchError {
    match e.kind() {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => FetchError::Timeout,
        _ => FetchError::Io(e.to_string()),
    }
}

/// A stream whose reads all end by one deadline.
struct Timed<S> {
    stream: S,
    deadline: Instant,
}

impl<S: Read + ReadTimeout> Read for Timed<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(ErrorKind::TimedOut.into());
        }
        self.stream.read_timeout(Some(left))?;
        self.stream.read(buf)
    }
}
