// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! VERIFY: prove that a local proxy is an I2P router proxy (fail closed).
//!
//! One request: `GET http://proxy.i2p/` must answer 200 with "I2P HTTP proxy OK" (Java I2P
//! and i2pd answer it locally). The watcher repeats it every 5 s.
//!
//! eepview never asks for a non-`.i2p` host, not even to test for an outproxy: on a router
//! with one, that request would reach the clearnet. A router outproxy cannot be used anyway,
//! because the gatekeeper (L1) forwards only `.i2p` hosts.

use std::io::{Read, Write};
use std::time::Duration;

use super::http::status_code;
use super::loopback::LoopbackAddr;

/// The marker text of the I2P proxy self-test page.
const PROXY_OK: &str = "I2P HTTP proxy OK";
/// Most bytes read from one probe answer.
const MAX_ANSWER: u64 = 256 * 1024;
const PROXY_TIMEOUT: Duration = Duration::from_secs(5);
/// The only host VERIFY asks for.
const PROXY_HOST: &str = "proxy.i2p";

/// The router proxy after VERIFY passed. Only [`verify`] builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedUpstream {
    addr: LoopbackAddr,
}

impl VerifiedUpstream {
    /// The proxy address.
    #[must_use]
    pub fn addr(&self) -> LoopbackAddr {
        self.addr
    }
}

/// The outcome of VERIFY.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// An I2P router proxy.
    Ok(VerifiedUpstream),
    /// Nothing answers.
    Down(String),
    /// Something answers, but not an I2P proxy.
    NotI2p(String),
}

/// One probe answer: status code and body text.
pub type Answer = Result<(u16, String), String>;

/// Asks the proxy at `addr` for its self-test page.
#[must_use]
pub fn verify(addr: LoopbackAddr) -> Verdict {
    judge(addr, probe(addr, PROXY_HOST, PROXY_TIMEOUT))
}

/// Decides from the self-test answer.
#[must_use]
pub fn judge(addr: LoopbackAddr, proxy: Answer) -> Verdict {
    match proxy {
        Err(e) => Verdict::Down(e),
        Ok((200, body)) if body.contains(PROXY_OK) => Verdict::Ok(VerifiedUpstream { addr }),
        Ok((code, _)) => Verdict::NotI2p(format!(
            "http://{PROXY_HOST}/ answered {code} without \"{PROXY_OK}\""
        )),
    }
}

/// `GET http://<host>/` through the proxy at `addr`.
fn probe(addr: LoopbackAddr, host: &str, timeout: Duration) -> Answer {
    let mut stream = addr.connect(timeout).map_err(|e| format!("{addr}: {e}"))?;
    let request = format!(
        "GET http://{host}/ HTTP/1.1\r\nHost: {host}\r\nUser-Agent: eepview\r\n\
         Accept: text/html\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("{addr}: {e}"))?;
    let mut raw = Vec::new();
    // A timeout after some bytes still leaves a usable answer, so the error is not fatal.
    let read = (&mut stream).take(MAX_ANSWER).read_to_end(&mut raw);
    parse_answer(&raw).ok_or_else(|| match read {
        Err(e) => format!("{addr}: {e}"),
        Ok(_) => format!("{addr}: not an HTTP answer"),
    })
}

/// Status code and body of a raw HTTP answer.
#[must_use]
pub fn parse_answer(raw: &[u8]) -> Option<(u16, String)> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let code = status_code(head.lines().next()?)?;
    Some((code, body.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;
    use std::sync::{Arc, Mutex};
    use std::thread;

    fn addr() -> LoopbackAddr {
        LoopbackAddr::parse("127.0.0.1:4444").unwrap()
    }

    fn ok_body() -> (u16, String) {
        (200, "<p>I2P HTTP proxy OK</p>".into())
    }

    #[test]
    fn judge_accepts_the_self_test_page() {
        let ok = judge(addr(), Ok(ok_body()));
        assert_eq!(ok, Verdict::Ok(VerifiedUpstream { addr: addr() }));
    }

    #[test]
    fn judge_refuses_a_proxy_that_is_not_i2p() {
        let down = judge(addr(), Err("refused".into()));
        assert_eq!(down, Verdict::Down("refused".into()));
        let other = judge(addr(), Ok((200, "hello".into())));
        assert!(matches!(other, Verdict::NotI2p(_)));
        let status = judge(addr(), Ok((404, PROXY_OK.into())));
        assert!(matches!(status, Verdict::NotI2p(_)));
    }

    #[test]
    fn parse_answer_cases() {
        assert_eq!(
            parse_answer(b"HTTP/1.1 503 No Outproxy\r\nX: y\r\n\r\nbody"),
            Some((503, "body".into()))
        );
        assert_eq!(parse_answer(b"HTTP/1.1 200 OK"), Some((200, String::new())));
        assert_eq!(parse_answer(b"SSH-2.0-OpenSSH"), None);
        assert_eq!(parse_answer(b""), None);
    }

    fn serve_one(mut stream: TcpStream, seen: &Mutex<Vec<String>>) {
        let mut buf = [0u8; 2048];
        let n = stream.read(&mut buf).unwrap_or(0);
        let text = String::from_utf8_lossy(&buf[..n]);
        let target = text.split_whitespace().nth(1).unwrap_or("").to_owned();
        let answer = if target == "http://proxy.i2p/" {
            "HTTP/1.1 200 OK\r\n\r\nI2P HTTP proxy OK"
        } else {
            "HTTP/1.1 404 Not Found\r\n\r\n"
        };
        seen.lock().unwrap().push(target);
        let _ = stream.write_all(answer.as_bytes());
    }

    /// A fake I2P proxy on 127.0.0.1:0 that records every request target.
    fn fake() -> (LoopbackAddr, Arc<Mutex<Vec<String>>>) {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        thread::spawn(move || {
            listener
                .incoming()
                .flatten()
                .for_each(|s| serve_one(s, &log));
        });
        (addr, seen)
    }

    #[test]
    fn verify_asks_only_for_the_self_test_page() {
        let (i2p, seen) = fake();
        assert!(matches!(verify(i2p), Verdict::Ok(v) if v.addr() == i2p));
        assert!(matches!(verify(i2p), Verdict::Ok(_)));
        let targets = seen.lock().unwrap().clone();
        assert_eq!(targets, vec!["http://proxy.i2p/"; 2]);
    }

    fn garbage(mut stream: TcpStream) {
        let _ = stream.write_all(b"garbage");
    }

    #[test]
    fn verify_refuses_garbage() {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        thread::spawn(move || listener.incoming().flatten().for_each(garbage));
        assert!(matches!(verify(addr), Verdict::Down(_)));
    }

    #[test]
    fn verify_closed_port_is_down() {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        drop(listener);
        assert!(matches!(verify(addr), Verdict::Down(_)));
    }
}
