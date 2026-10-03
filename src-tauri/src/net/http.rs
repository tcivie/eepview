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
    /// Parses a head that ends with an empty line. `None` when it is not valid UTF-8 HTTP.
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
        Some(Self { start, headers })
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
    /// `CONNECT <host>:80`: answer 200, then read one plain HTTP request from the tunnel.
    Terminate {
        /// I2P host.
        host: String,
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
    match port {
        "80" => Plan::Terminate { host },
        "443" if allow_tls => Plan::Relay { host },
        _ => Plan::Refuse(Refusal::NotI2p),
    }
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

/// Decides the one request read inside a `CONNECT <host>:80` tunnel.
#[must_use]
pub fn plan_inner(head: &Head, host: &str) -> Plan {
    let Some((method, target, version)) = head.request_line() else {
        return Plan::Refuse(Refusal::BadRequest);
    };
    if !version.starts_with("HTTP/1.") || !target.starts_with('/') || method == "CONNECT" {
        return Plan::Refuse(Refusal::BadRequest);
    }
    let named = head.header("host").unwrap_or(host);
    if named != host && named != format!("{host}:80") {
        return Plan::Refuse(Refusal::NotI2p);
    }
    Plan::Http {
        method: method.to_owned(),
        host: host.to_owned(),
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
    fn connect_requests() {
        assert_eq!(
            plan_of("CONNECT stats.i2p:80 HTTP/1.1"),
            Plan::Terminate {
                host: "stats.i2p".into()
            }
        );
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
            "CONNECT stats.i2p:22 HTTP/1.1",
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
        assert!(matches!(plan_inner(&ok, "a.i2p"), Plan::Http { .. }));
        let port = head("GET /x HTTP/1.1\r\nHost: a.i2p:80\r\n\r\n");
        assert!(matches!(plan_inner(&port, "a.i2p"), Plan::Http { .. }));
        let other = head("GET /x HTTP/1.1\r\nHost: evil.com\r\n\r\n");
        assert_eq!(plan_inner(&other, "a.i2p"), Plan::Refuse(Refusal::NotI2p));
        let absolute = head("GET http://evil.com/ HTTP/1.1\r\n\r\n");
        assert_eq!(
            plan_inner(&absolute, "a.i2p"),
            Plan::Refuse(Refusal::BadRequest)
        );
        let nested = head("CONNECT /x HTTP/1.1\r\n\r\n");
        assert_eq!(
            plan_inner(&nested, "a.i2p"),
            Plan::Refuse(Refusal::BadRequest)
        );
        assert_eq!(
            plan_inner(&head("x\r\n\r\n"), "a.i2p"),
            Plan::Refuse(Refusal::BadRequest)
        );
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
}
