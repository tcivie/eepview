//! VERIFY: prove that a local proxy is an I2P router proxy with no outproxy (fail closed).
//!
//! Two requests, no network traffic when the proxy is what it claims:
//! 1. `GET http://proxy.i2p/` must answer 200 with "I2P HTTP proxy OK" (Java I2P and i2pd).
//! 2. `GET http://example.com/` must answer 5xx: Java I2P says 503 "No Outproxy Configured",
//!    i2pd says 500 "Outproxy failure". Any other answer means a clearnet exit.

use std::io::{Read, Write};
use std::time::Duration;

use super::http::status_code;
use super::loopback::LoopbackAddr;

/// The marker text of the I2P proxy self-test page.
const PROXY_OK: &str = "I2P HTTP proxy OK";
/// Most bytes read from one probe answer.
const MAX_ANSWER: u64 = 256 * 1024;
const PROXY_TIMEOUT: Duration = Duration::from_secs(5);
const CLEARNET_TIMEOUT: Duration = Duration::from_secs(20);

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
    /// An I2P proxy with no outproxy.
    Ok(VerifiedUpstream),
    /// Nothing answers.
    Down(String),
    /// Something answers, but not an I2P proxy.
    NotI2p(String),
    /// An I2P proxy that reaches the clearnet, or a clearnet probe with no clear answer.
    Outproxy(String),
}

/// One probe answer: status code and body text.
pub type Answer = Result<(u16, String), String>;

/// Runs both probes against `addr`.
#[must_use]
pub fn verify(addr: LoopbackAddr) -> Verdict {
    let first = probe(addr, "proxy.i2p", PROXY_TIMEOUT);
    judge(addr, first, || probe(addr, "example.com", CLEARNET_TIMEOUT))
}

/// Decides from the probe answers. The clearnet probe runs only when the first one passes.
pub fn judge(addr: LoopbackAddr, proxy: Answer, clearnet: impl FnOnce() -> Answer) -> Verdict {
    let (code, body) = match proxy {
        Ok(answer) => answer,
        Err(e) => return Verdict::Down(e),
    };
    if code != 200 || !body.contains(PROXY_OK) {
        return Verdict::NotI2p(format!(
            "http://proxy.i2p/ answered {code} without \"{PROXY_OK}\""
        ));
    }
    match clearnet() {
        Ok((code, _)) if (500..600).contains(&code) => Verdict::Ok(VerifiedUpstream { addr }),
        Ok((code, _)) => Verdict::Outproxy(format!(
            "http://example.com/ answered {code}: the proxy has a clearnet outproxy"
        )),
        Err(e) => Verdict::Outproxy(format!("the clearnet probe gave no clear answer: {e}")),
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
    use std::thread;

    fn addr() -> LoopbackAddr {
        LoopbackAddr::parse("127.0.0.1:4444").unwrap()
    }

    fn ok_body() -> (u16, String) {
        (200, "<p>I2P HTTP proxy OK</p>".into())
    }

    #[test]
    fn judge_accepts_an_i2p_proxy_without_outproxy() {
        let ok = judge(addr(), Ok(ok_body()), || {
            Ok((503, "No Outproxy Configured".into()))
        });
        assert_eq!(ok, Verdict::Ok(VerifiedUpstream { addr: addr() }));
        let i2pd = judge(addr(), Ok(ok_body()), || {
            Ok((500, "Outproxy failure".into()))
        });
        assert!(matches!(i2pd, Verdict::Ok(_)));
    }

    #[test]
    fn judge_refuses_a_clearnet_exit() {
        let exit = judge(addr(), Ok(ok_body()), || Ok((200, "Example Domain".into())));
        assert!(matches!(exit, Verdict::Outproxy(_)));
        let redirect = judge(addr(), Ok(ok_body()), || Ok((301, String::new())));
        assert!(matches!(redirect, Verdict::Outproxy(_)));
        let hang = judge(addr(), Ok(ok_body()), || Err("timed out".into()));
        assert!(matches!(hang, Verdict::Outproxy(_)));
    }

    #[test]
    fn judge_refuses_a_proxy_that_is_not_i2p() {
        let down = judge(addr(), Err("refused".into()), || unreachable!());
        assert_eq!(down, Verdict::Down("refused".into()));
        let other = judge(addr(), Ok((200, "hello".into())), || unreachable!());
        assert!(matches!(other, Verdict::NotI2p(_)));
        let status = judge(addr(), Ok((404, PROXY_OK.into())), || unreachable!());
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

    fn serve_one(mut stream: TcpStream, answer: fn(&str) -> &'static str) {
        let mut buf = [0u8; 2048];
        let n = stream.read(&mut buf).unwrap_or(0);
        let text = String::from_utf8_lossy(&buf[..n]);
        let target = text.split_whitespace().nth(1).unwrap_or("").to_owned();
        let _ = stream.write_all(answer(&target).as_bytes());
    }

    fn serve_all(listener: &std::net::TcpListener, answer: fn(&str) -> &'static str) {
        for stream in listener.incoming().flatten() {
            serve_one(stream, answer);
        }
    }

    /// A fake proxy on 127.0.0.1:0 that answers each request with `answer(target)`.
    fn fake(answer: fn(&str) -> &'static str) -> LoopbackAddr {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        thread::spawn(move || serve_all(&listener, answer));
        addr
    }

    fn i2p_answer(target: &str) -> &'static str {
        if target.contains("proxy.i2p") {
            return "HTTP/1.1 200 OK\r\n\r\nI2P HTTP proxy OK";
        }
        "HTTP/1.1 503 No Outproxy Configured\r\n\r\n"
    }

    fn exit_answer(target: &str) -> &'static str {
        if target.contains("proxy.i2p") {
            return "HTTP/1.1 200 OK\r\n\r\nI2P HTTP proxy OK";
        }
        "HTTP/1.1 200 OK\r\n\r\nExample Domain"
    }

    #[test]
    fn verify_against_fake_proxies() {
        let i2p = fake(i2p_answer);
        assert!(matches!(verify(i2p), Verdict::Ok(v) if v.addr() == i2p));
        assert!(matches!(verify(fake(exit_answer)), Verdict::Outproxy(_)));
        assert!(matches!(verify(fake(|_| "garbage")), Verdict::Down(_)));
    }

    #[test]
    fn verify_closed_port_is_down() {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        drop(listener);
        assert!(matches!(verify(addr), Verdict::Down(_)));
    }
}
