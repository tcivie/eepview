// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! HTTP/1.x message heads: parse, decide, rewrite. Pure functions for the gatekeeper.

use tauri::Url;

use super::host::is_i2p_host;
use super::rules;

/// Largest request or response head the gatekeeper reads.
pub const MAX_HEAD: usize = 64 * 1024;

/// Request headers that never travel to the router: hop-by-hop, or set by the gatekeeper.
const DROP_REQUEST: [&str; 9] = [
    "connection",
    "proxy-connection",
    "keep-alive",
    "proxy-authorization",
    "te",
    "trailer",
    "upgrade",
    "expect",
    "host",
];

/// Response headers the gatekeeper replaces or removes. `alt-svc` could start QUIC (UDP).
const DROP_RESPONSE: [&str; 4] = ["connection", "proxy-connection", "keep-alive", "alt-svc"];

/// A request or response head: the start line and the header fields, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// `GET http://a.i2p/ HTTP/1.1` or `HTTP/1.1 200 OK`.
    pub start: String,
    /// Header fields as sent.
    pub headers: Vec<(String, String)>,
}

impl Head {
    /// Parses a head that ends with an empty line. `None` when it is not valid UTF-8 HTTP, or
    /// when a line holds a bare CR or LF (RFC 9112, 2.2 and 5.5): such a byte lets one side read a
    /// second request line or header that the other side does not see.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(bytes).ok()?;
        let mut lines = text.split("\r\n");
        let start = lines.next().filter(|l| !l.is_empty())?.to_owned();
        let mut headers = Vec::new();
        for line in lines.take_while(|l| !l.is_empty()) {
            let (name, value) = line.split_once(':')?;
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
        // Check the raw lines: `trim` would hide a CR or LF at the end of a value.
        let mut raw = text.split("\r\n").take_while(|l| !l.is_empty());
        let clean = raw.all(|l| !l.contains(['\r', '\n']));
        clean.then_some(Self { start, headers })
    }

    /// The first value of a header, by case-insensitive name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Removes every header named in `names` (lower case).
    pub fn remove(&mut self, names: &[&str]) {
        self.headers
            .retain(|(n, _)| !names.contains(&n.to_ascii_lowercase().as_str()));
    }

    /// The head as bytes, with the closing empty line.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = format!("{}\r\n", self.start);
        for (name, value) in &self.headers {
            out.push_str(name);
            out.push_str(": ");
            out.push_str(value);
            out.push_str("\r\n");
        }
        out.push_str("\r\n");
        out.into_bytes()
    }

    /// The three words of a request line.
    #[must_use]
    pub fn request_line(&self) -> Option<(&str, &str, &str)> {
        let mut parts = self.start.split(' ');
        let line = (parts.next()?, parts.next()?, parts.next()?);
        parts.next().is_none().then_some(line)
    }

    /// The `Content-Length` of a body. `Err` for a chunked or a broken length.
    ///
    /// # Errors
    ///
    /// Fails for `Transfer-Encoding` bodies and lengths that are not numbers.
    pub fn body_length(&self) -> Result<u64, Refusal> {
        if self.header("transfer-encoding").is_some() {
            return Err(Refusal::LengthRequired);
        }
        let mut lengths = self
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"));
        let first = lengths.next();
        if lengths.next().is_some() {
            // Two lengths, equal or not, are a smuggling risk: refuse.
            return Err(Refusal::BadRequest);
        }
        first.map_or(Ok(0), |(_, v)| v.parse().map_err(|_| Refusal::BadRequest))
    }
}

/// The end of the head in `buf`: the index just after `\r\n\r\n`.
#[must_use]
pub fn head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

/// Why the gatekeeper answers by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The destination is not an I2P site (403).
    NotI2p,
    /// The request is malformed (400).
    BadRequest,
    /// A chunked request body (411).
    LengthRequired,
    /// The router proxy is not verified, or could not be reached (502).
    Upstream,
    /// Too many open connections (503).
    Busy,
}

impl Refusal {
    /// The reason kind for the diagnostics log. The host is never logged.
    #[must_use]
    pub fn reason(self) -> crate::diag::RefuseReason {
        use crate::diag::RefuseReason as R;
        match self {
            Self::NotI2p => R::NotI2p,
            Self::BadRequest => R::BadRequest,
            Self::LengthRequired => R::LengthRequired,
            Self::Upstream => R::Upstream,
            Self::Busy => R::Busy,
        }
    }

    /// The full response the gatekeeper sends, with no upstream connection.
    #[must_use]
    pub fn response(self) -> Vec<u8> {
        let (code, text) = match self {
            Self::NotI2p => (403, "Forbidden: eepview opens .i2p sites only"),
            Self::BadRequest => (400, "Bad Request"),
            Self::LengthRequired => (411, "Length Required"),
            Self::Upstream => (502, "Bad Gateway: the I2P router is not verified"),
            Self::Busy => (503, "Service Unavailable: too many connections"),
        };
        format!(
            "HTTP/1.1 {code} {text}\r\nContent-Type: text/plain; charset=utf-8\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
        .into_bytes()
    }
}

/// What the gatekeeper does with a request from the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Forward one plain HTTP request to `http://<host><path>`.
    Http {
        /// Method.
        method: String,
        /// I2P host.
        host: String,
        /// Path and query.
        path: String,
    },
    /// `CONNECT <host>:<port>` for any port 1–65535 but 443: answer 200, then read one plain
    /// HTTP request from the tunnel.
    Terminate {
        /// I2P host.
        host: String,
        /// The `CONNECT` port.
        port: u16,
    },
    /// `CONNECT <host>:443`: relay the TLS bytes through the router proxy.
    Relay {
        /// I2P host.
        host: String,
    },
    /// Answer by itself.
    Refuse(Refusal),
}

/// Decides a request that arrives at the proxy port.
#[must_use]
pub fn plan(head: &Head, allow_tls: bool) -> Plan {
    let Some((method, target, version)) = head.request_line() else {
        return Plan::Refuse(Refusal::BadRequest);
    };
    if !version.starts_with("HTTP/1.") {
        return Plan::Refuse(Refusal::BadRequest);
    }
    if method == "CONNECT" {
        return plan_connect(target, allow_tls);
    }
    plan_absolute(method, target)
}

fn plan_connect(target: &str, allow_tls: bool) -> Plan {
    let Some((host, port)) = target.rsplit_once(':') else {
        return Plan::Refuse(Refusal::BadRequest);
    };
    if !is_i2p_host(host) {
        return Plan::Refuse(Refusal::NotI2p);
    }
    let host = host.to_owned();
    match connect_port(port) {
        Some(443) if allow_tls => Plan::Relay { host },
        Some(443) | None => Plan::Refuse(Refusal::NotI2p),
        Some(port) => Plan::Terminate { host, port },
    }
}

/// The port of a `CONNECT` target: digits only, 1–65535.
fn connect_port(text: &str) -> Option<u16> {
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok().filter(|&port| port != 0)
}

fn plan_absolute(method: &str, target: &str) -> Plan {
    if !target.starts_with("http://") {
        return Plan::Refuse(Refusal::NotI2p);
    }
    let Ok(url) = Url::parse(target) else {
        return Plan::Refuse(Refusal::BadRequest);
    };
    match i2p_host(&url) {
        Some(host) => Plan::Http {
            method: method.to_owned(),
            host,
            path: path_of(&url),
        },
        None => Plan::Refuse(Refusal::NotI2p),
    }
}

/// `host` or `host:port` of an `http://` URL with no user info, when it is an I2P host. An
/// explicit port must be 1–65535.
fn i2p_host(url: &Url) -> Option<String> {
    let clean = url.username().is_empty() && url.password().is_none();
    let host = url.host_str()?;
    if !clean || !is_i2p_host(host) {
        return None;
    }
    match url.port() {
        None => Some(host.to_owned()),
        Some(0) => None,
        Some(port) => Some(format!("{host}:{port}")),
    }
}

fn path_of(url: &Url) -> String {
    match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_owned(),
    }
}

/// Decides the one request read inside a `CONNECT <host>:<port>` tunnel. The request goes to
/// `host` and `port` (the host alone for port 80), whatever it names. A `Host` header that
/// names another host or another port is refused.
#[must_use]
pub fn plan_inner(head: &Head, host: &str, port: u16) -> Plan {
    let Some((method, target, version)) = head.request_line() else {
        return Plan::Refuse(Refusal::BadRequest);
    };
    if !version.starts_with("HTTP/1.") || !target.starts_with('/') || method == "CONNECT" {
        return Plan::Refuse(Refusal::BadRequest);
    }
    let with_port = format!("{host}:{port}");
    let pinned = if port == 80 { host } else { &with_port };
    let named = head.header("host").unwrap_or(pinned);
    if named != pinned && named != with_port {
        return Plan::Refuse(Refusal::NotI2p);
    }
    Plan::Http {
        method: method.to_owned(),
        host: pinned.to_owned(),
        path: target.to_owned(),
    }
}

/// The request head the router proxy gets: absolute form, `Connection: close`.
#[must_use]
pub fn upstream_request(method: &str, host: &str, path: &str, from: &Head) -> Vec<u8> {
    let mut head = from.clone();
    head.start = format!("{method} http://{host}{path} HTTP/1.1");
    head.remove(&DROP_REQUEST);
    head.headers.push(("Host".into(), host.to_owned()));
    head.headers.push(("Connection".into(), "close".into()));
    head.to_bytes()
}

/// The `CONNECT` the router proxy gets for a TLS site.
#[must_use]
pub fn upstream_connect(host: &str) -> Vec<u8> {
    format!("CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n").into_bytes()
}

/// The response head the engine gets: the page policy (L3) added, `Connection: close`.
#[must_use]
pub fn rewrite_response(mut head: Head) -> Vec<u8> {
    head.remove(&DROP_RESPONSE);
    head.headers
        .push(("Content-Security-Policy".into(), rules::csp_header().into()));
    head.headers
        .push(("X-DNS-Prefetch-Control".into(), "off".into()));
    head.headers.push(("Connection".into(), "close".into()));
    head.to_bytes()
}

/// True when a response start line says 1xx: an interim head, and a final head follows it.
#[must_use]
pub fn is_interim(start: &str) -> bool {
    status_code(start).is_some_and(|c| (100..200).contains(&c))
}

/// True when a response start line says 2xx.
#[must_use]
pub fn is_success(start: &str) -> bool {
    status_code(start).is_some_and(|c| (200..300).contains(&c))
}

/// The status code of a response start line.
#[must_use]
pub fn status_code(start: &str) -> Option<u16> {
    let mut parts = start.split(' ');
    let version = parts.next()?;
    if !version.starts_with("HTTP/") {
        return None;
    }
    parts.next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(text: &str) -> Head {
        Head::parse(text.as_bytes()).unwrap()
    }

    fn plan_of(start: &str) -> Plan {
        plan(&head(&format!("{start}\r\n\r\n")), true)
    }

    #[test]
    fn parse_and_query_headers() {
        let h = head("GET / HTTP/1.1\r\nHost: a.i2p\r\nX-A: 1\r\n\r\n");
        assert_eq!(h.header("host"), Some("a.i2p"));
        assert_eq!(h.header("x-b"), None);
        assert_eq!(h.request_line(), Some(("GET", "/", "HTTP/1.1")));
        assert_eq!(
            String::from_utf8(h.to_bytes()).unwrap(),
            "GET / HTTP/1.1\r\nHost: a.i2p\r\nX-A: 1\r\n\r\n"
        );
        assert!(Head::parse(b"\r\n\r\n").is_none());
        assert!(Head::parse(b"GET / HTTP/1.1\r\nbroken\r\n\r\n").is_none());
        assert!(Head::parse(&[0xff, 0xfe]).is_none());
        assert_eq!(head("GET / HTTP/1.1 x\r\n\r\n").request_line(), None);
    }

    #[test]
    fn head_end_finds_blank_line() {
        assert_eq!(head_end(b"GET / HTTP/1.1\r\n\r\nbody"), Some(18));
        assert_eq!(head_end(b"GET / HTTP/1.1\r\n"), None);
    }

    #[test]
    fn body_length() {
        assert_eq!(head("POST / HTTP/1.1\r\n\r\n").body_length(), Ok(0));
        assert_eq!(
            head("POST / HTTP/1.1\r\nContent-Length: 12\r\n\r\n").body_length(),
            Ok(12)
        );
        assert_eq!(
            head("POST / HTTP/1.1\r\nContent-Length: x\r\n\r\n").body_length(),
            Err(Refusal::BadRequest)
        );
        assert_eq!(
            head("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n").body_length(),
            Err(Refusal::LengthRequired)
        );
    }

    #[test]
    fn absolute_requests() {
        assert_eq!(
            plan_of("GET http://stats.i2p/a?b=1 HTTP/1.1"),
            Plan::Http {
                method: "GET".into(),
                host: "stats.i2p".into(),
                path: "/a?b=1".into()
            }
        );
        assert!(matches!(
            plan_of("GET http://STATS.I2P/ HTTP/1.1"),
            Plan::Http { .. }
        ));
        for refused in [
            "GET http://example.com/ HTTP/1.1",
            "GET http://127.0.0.1:7657/ HTTP/1.1",
            "GET http://localhost/ HTTP/1.1",
            "GET http://192.168.1.1/ HTTP/1.1",
            "GET http://[::1]/ HTTP/1.1",
            "GET http://a@evil.com/#.i2p HTTP/1.1",
            "GET http://u@stats.i2p/ HTTP/1.1",
            "GET http://stats.i2p./ HTTP/1.1",
            "GET http://bücher.i2p/ HTTP/1.1",
            "GET http://stats.i2p:0/ HTTP/1.1",
            "GET https://stats.i2p/ HTTP/1.1",
            "GET /relative HTTP/1.1",
            "GET ftp://stats.i2p/ HTTP/1.1",
        ] {
            assert_eq!(plan_of(refused), Plan::Refuse(Refusal::NotI2p), "{refused}");
        }
        assert_eq!(
            plan_of("GET http://[x/ HTTP/1.1"),
            Plan::Refuse(Refusal::BadRequest)
        );
        assert_eq!(
            plan_of("GET http://a.i2p/ SPDY/3"),
            Plan::Refuse(Refusal::BadRequest)
        );
        assert_eq!(plan_of("GET"), Plan::Refuse(Refusal::BadRequest));
    }

    #[test]
    fn connect_to_any_port_but_443_is_terminated() {
        for port in [80, 1, 22, 8080, 65535] {
            assert_eq!(
                plan_of(&format!("CONNECT stats.i2p:{port} HTTP/1.1")),
                Plan::Terminate {
                    host: "stats.i2p".into(),
                    port
                }
            );
        }
        assert_eq!(
            plan_of("CONNECT stats.i2p:080 HTTP/1.1"),
            Plan::Terminate {
                host: "stats.i2p".into(),
                port: 80
            }
        );
    }

    #[test]
    fn connect_requests() {
        assert_eq!(
            plan_of("CONNECT stats.i2p:443 HTTP/1.1"),
            Plan::Relay {
                host: "stats.i2p".into()
            }
        );
        let tls_off = plan(&head("CONNECT stats.i2p:443 HTTP/1.1\r\n\r\n"), false);
        assert_eq!(tls_off, Plan::Refuse(Refusal::NotI2p));
        for refused in [
            "CONNECT example.com:443 HTTP/1.1",
            "CONNECT 127.0.0.1:80 HTTP/1.1",
            "CONNECT example.com:8080 HTTP/1.1",
            "CONNECT stats.i2p:0 HTTP/1.1",
            "CONNECT stats.i2p:65536 HTTP/1.1",
            "CONNECT stats.i2p:+80 HTTP/1.1",
            "CONNECT stats.i2p: HTTP/1.1",
            "CONNECT stats.i2p:8o HTTP/1.1",
            "CONNECT Stats.i2p:80 HTTP/1.1",
            "CONNECT stats.i2p.:80 HTTP/1.1",
        ] {
            assert_eq!(plan_of(refused), Plan::Refuse(Refusal::NotI2p), "{refused}");
        }
        assert_eq!(
            plan_of("CONNECT stats.i2p HTTP/1.1"),
            Plan::Refuse(Refusal::BadRequest)
        );
    }

    #[test]
    fn inner_requests_stay_on_the_tunnel_host() {
        let ok = head("GET /x HTTP/1.1\r\nHost: a.i2p\r\n\r\n");
        assert!(matches!(plan_inner(&ok, "a.i2p", 80), Plan::Http { .. }));
        let port = head("GET /x HTTP/1.1\r\nHost: a.i2p:80\r\n\r\n");
        assert!(matches!(plan_inner(&port, "a.i2p", 80), Plan::Http { .. }));
        let other = head("GET /x HTTP/1.1\r\nHost: evil.com\r\n\r\n");
        assert_eq!(
            plan_inner(&other, "a.i2p", 80),
            Plan::Refuse(Refusal::NotI2p)
        );
        let absolute = head("GET http://evil.com/ HTTP/1.1\r\n\r\n");
        assert_eq!(
            plan_inner(&absolute, "a.i2p", 80),
            Plan::Refuse(Refusal::BadRequest)
        );
        let nested = head("CONNECT /x HTTP/1.1\r\n\r\n");
        assert_eq!(
            plan_inner(&nested, "a.i2p", 80),
            Plan::Refuse(Refusal::BadRequest)
        );
        assert_eq!(
            plan_inner(&head("x\r\n\r\n"), "a.i2p", 80),
            Plan::Refuse(Refusal::BadRequest)
        );
    }

    // Req: a `CONNECT a.i2p:8080` tunnel forwards to `a.i2p:8080` only. The `Host` header must
    // name that host and port; another port or another host is refused.
    #[test]
    fn inner_requests_on_another_port_stay_on_the_tunnel_port() {
        let ok = head("GET /x HTTP/1.1\r\nHost: a.i2p:8080\r\n\r\n");
        let pinned = Plan::Http {
            method: "GET".into(),
            host: "a.i2p:8080".into(),
            path: "/x".into(),
        };
        assert_eq!(plan_inner(&ok, "a.i2p", 8080), pinned);
        let none = head("GET /x HTTP/1.1\r\n\r\n");
        assert_eq!(plan_inner(&none, "a.i2p", 8080), pinned);
        let out = String::from_utf8(upstream_request("GET", "a.i2p:8080", "/x", &ok)).unwrap();
        assert!(
            out.starts_with("GET http://a.i2p:8080/x HTTP/1.1\r\n"),
            "{out}"
        );
        assert!(out.contains("\r\nHost: a.i2p:8080\r\n"), "{out}");
        for named in [
            "a.i2p",
            "a.i2p:80",
            "a.i2p:8081",
            "b.i2p:8080",
            "evil.com:8080",
        ] {
            let wrong = head(&format!("GET /x HTTP/1.1\r\nHost: {named}\r\n\r\n"));
            assert_eq!(
                plan_inner(&wrong, "a.i2p", 8080),
                Plan::Refuse(Refusal::NotI2p),
                "{named}"
            );
        }
    }

    #[test]
    fn interim_heads_are_1xx() {
        assert!(is_interim("HTTP/1.1 100 Continue"));
        assert!(is_interim("HTTP/1.1 101 Switching Protocols"));
        assert!(is_interim("HTTP/1.1 103 Early Hints"));
        assert!(!is_interim("HTTP/1.1 200 OK"));
        assert!(!is_interim("HTTP/1.1 99 X"));
        assert!(!is_interim("HTTP/1.1 x"));
    }

    #[test]
    fn upstream_request_is_absolute_and_closes() {
        let from = head(
            "GET /p HTTP/1.1\r\nHost: evil.com\r\nConnection: keep-alive\r\n\
             Proxy-Authorization: x\r\nAccept: */*\r\n\r\n",
        );
        let out = String::from_utf8(upstream_request("GET", "a.i2p", "/p", &from)).unwrap();
        assert_eq!(
            out,
            "GET http://a.i2p/p HTTP/1.1\r\nAccept: */*\r\nHost: a.i2p\r\nConnection: close\r\n\r\n"
        );
        let connect = String::from_utf8(upstream_connect("a.i2p")).unwrap();
        assert!(connect.starts_with("CONNECT a.i2p:443 HTTP/1.1\r\n"));
    }

    #[test]
    fn response_gets_policy_and_close() {
        let h = head("HTTP/1.1 200 OK\r\nConnection: keep-alive\r\nAlt-Svc: h3=\":443\"\r\n\r\n");
        let out = String::from_utf8(rewrite_response(h)).unwrap();
        assert!(out.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(out.contains("Content-Security-Policy: default-src"));
        assert!(out.contains("Connection: close\r\n"));
        assert!(!out.contains("keep-alive") && !out.contains("Alt-Svc"));
    }

    #[test]
    fn refusals_have_a_reason_kind() {
        assert_eq!(Refusal::Busy.reason().as_str(), "busy");
        assert_eq!(Refusal::NotI2p.reason().as_str(), "not-i2p");
        assert_eq!(Refusal::Upstream.reason().as_str(), "upstream");
        assert_eq!(Refusal::BadRequest.reason().as_str(), "bad-request");
        assert_eq!(Refusal::LengthRequired.reason().as_str(), "length-required");
    }

    #[test]
    fn refusals_and_status() {
        for (r, code) in [
            (Refusal::NotI2p, 403),
            (Refusal::BadRequest, 400),
            (Refusal::LengthRequired, 411),
            (Refusal::Upstream, 502),
            (Refusal::Busy, 503),
        ] {
            let text = String::from_utf8(r.response()).unwrap();
            assert_eq!(status_code(&text), Some(code));
        }
        assert!(is_success("HTTP/1.1 200 Connection established"));
        assert!(!is_success("HTTP/1.1 503 No Outproxy"));
        assert_eq!(status_code("SPDY 200"), None);
        assert_eq!(status_code("HTTP/1.1"), None);
    }

    // Req: RFC 9112 2.2 and 5.5: a bare CR anywhere in the request line or the headers, and a
    // bare LF or CR inside a header value, make the head invalid. Found by the fuzz target
    // `gatekeeper_request` (docs/wiki/fuzzing.md).
    #[test]
    fn a_bare_cr_or_lf_makes_the_head_invalid() {
        // The exact input of the fuzz crash.
        assert!(Head::parse(b"\r / HTTP/1.").is_none());
        for raw in [
            "\r / HTTP/1.1\r\n\r\n",
            "GET\r http://stats.i2p/ HTTP/1.1\r\n\r\n",
            "GET http://stats.i2p/\n HTTP/1.1\r\n\r\n",
            "GET http://stats.i2p/ HTTP/1.1\r\nX: a\nHost: evil.example\r\n\r\n",
            "GET http://stats.i2p/ HTTP/1.1\r\nX: a\rHost: evil.example\r\n\r\n",
            "GET http://stats.i2p/ HTTP/1.1\r\nX: a\r\r\n\r\n",
            "GET http://stats.i2p/ HTTP/1.1\r\nX\r: a\r\n\r\n",
        ] {
            assert!(Head::parse(raw.as_bytes()).is_none(), "{raw:?}");
        }
        assert!(Head::parse(b"GET / HTTP/1.1\r\nX: a b\r\n\r\n").is_some());
    }
}
