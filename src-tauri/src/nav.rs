// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Address-bar input rules and the I2P-only navigation guard.
//!
//! Pure functions: no Tauri runtime, no I/O. `tauri::Url` is the `url` crate.

use tauri::Url;

use crate::net::host::is_i2p_host;

/// The scheme of the bundled pages.
pub const INTERNAL_SCHEME: &str = "eepview";

/// Every internal page, as `eepview://<name>`.
pub const INTERNAL_PAGES: [&str; 9] = [
    "home",
    "bookmarks",
    "history",
    "stats",
    "settings",
    "setup",
    "blocked",
    "router-down",
    "report",
];

/// Schemes that never take `//`. Input that starts with one of them is never a host name.
const OPAQUE_SCHEMES: [&str; 8] = [
    "javascript",
    "data",
    "about",
    "blob",
    "mailto",
    "file",
    "view-source",
    "vbscript",
];

/// What the address bar input means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An I2P site, ready to load.
    Web(Url),
    /// An internal page, normalised to `eepview://<page>[?query]`.
    Internal(String),
    /// Free text: search history and bookmarks.
    Search(String),
    /// Not allowed.
    Refused(Refusal),
}

/// Why an input was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A valid address that is not on I2P.
    NotI2p,
    /// Not an address at all.
    Invalid,
}

impl Refusal {
    /// The `NavResult.reason` string of the contract.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotI2p => "not-i2p",
            Self::Invalid => "invalid",
        }
    }
}

/// Classifies address-bar input.
#[must_use]
pub fn classify(input: &str) -> Target {
    let text = input.trim();
    if text.is_empty() {
        return Target::Refused(Refusal::Invalid);
    }
    if scheme_of(text).is_some_and(|s| s == INTERNAL_SCHEME) {
        return internal_target(text);
    }
    let has_scheme = scheme_of(text).is_some();
    if !has_scheme && is_search(text) {
        return Target::Search(text.to_owned());
    }
    let candidate = if has_scheme {
        text.to_owned()
    } else {
        format!("http://{text}")
    };
    match Url::parse(&candidate) {
        Ok(url) => web_target(url),
        Err(_) => Target::Refused(Refusal::Invalid),
    }
}

/// The lower-case scheme of `text`, when it has one.
///
/// `localhost:8080` and `foo.i2p:80` have no scheme: only `scheme://` or a known opaque scheme
/// (`javascript:`, `data:` …) counts.
fn scheme_of(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(':')?;
    let mut chars = head.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !(first_ok && rest_ok) {
        return None;
    }
    let scheme = head.to_ascii_lowercase();
    let opaque = OPAQUE_SCHEMES.contains(&scheme.as_str());
    (rest.starts_with("//") || opaque || scheme == INTERNAL_SCHEME).then_some(scheme)
}

/// One word with no dot and no colon (owner decision): `stats` searches, `a b`, `a.b` and
/// `localhost:8080` do not.
fn is_search(text: &str) -> bool {
    !text.chars().any(char::is_whitespace) && !text.contains(['.', ':'])
}

fn web_target(mut url: Url) -> Target {
    trim_trailing_dot(&mut url);
    if is_allowed_web(&url) {
        Target::Web(url)
    } else {
        Target::Refused(Refusal::NotI2p)
    }
}

/// `foo.i2p.` and `foo.i2p` name the same host; the address bar keeps the short form.
fn trim_trailing_dot(url: &mut Url) {
    let Some(host) = url.host_str().map(str::to_owned) else {
        return;
    };
    if let Some(short) = host.strip_suffix('.') {
        // A failed set_host leaves the URL as it was; the guard then refuses it.
        let _ = url.set_host(Some(short));
    }
}

fn internal_target(text: &str) -> Target {
    match Url::parse(text) {
        Ok(url) => {
            internal_from_url(&url).map_or(Target::Refused(Refusal::Invalid), Target::Internal)
        }
        Err(_) => Target::Refused(Refusal::Invalid),
    }
}

/// The normalised `eepview://page[?query]` string of an internal URL, when the page exists.
#[must_use]
pub fn internal_from_url(url: &Url) -> Option<String> {
    if url.scheme() != INTERNAL_SCHEME {
        return None;
    }
    let page = url.host_str()?;
    if !INTERNAL_PAGES.contains(&page) || !matches!(url.path(), "" | "/") {
        return None;
    }
    Some(match url.query() {
        Some(q) if !q.is_empty() => format!("{INTERNAL_SCHEME}://{page}?{q}"),
        _ => format!("{INTERNAL_SCHEME}://{page}"),
    })
}

/// True when a `tab-*` webview may load `url` as a page or a frame.
///
/// Only `http(s)://<name>.i2p` passes, plus the two empty documents an engine makes by itself.
#[must_use]
pub fn guard(url: &Url) -> bool {
    is_allowed_web(url) || matches!(url.as_str(), "about:blank" | "about:srcdoc")
}

/// True for `http(s)://<name>.i2p[...]` with no user info.
#[must_use]
pub fn is_allowed_web(url: &Url) -> bool {
    let scheme_ok = matches!(url.scheme(), "http" | "https");
    let no_userinfo = url.username().is_empty() && url.password().is_none();
    let host_ok = url.host_str().is_some_and(is_i2p_host);
    let port_ok = url.port() != Some(0);
    scheme_ok && no_userinfo && host_ok && port_ok
}

/// The host of an allowed web URL string, for per-site settings.
#[must_use]
pub fn host_of(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    is_allowed_web(&parsed).then(|| parsed.host_str().map(str::to_owned))?
}

/// True for an `http(s)` URL on an I2P host (text form of [`is_allowed_web`]).
#[must_use]
pub fn is_allowed(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| is_allowed_web(&u))
}

/// True for an `http(s)` address (allowed or not); see [`is_allowed`] for the I2P rule.
#[must_use]
pub fn is_web(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// `eepview://<page>` with one query parameter, percent-encoded.
#[must_use]
pub fn internal_with(page: &str, params: &[(&str, &str)]) -> String {
    let base = format!("{INTERNAL_SCHEME}://{page}");
    if params.is_empty() {
        return base;
    }
    match Url::parse(&base) {
        Ok(mut url) => {
            url.query_pairs_mut().extend_pairs(params);
            url.to_string()
        }
        Err(_) => base,
    }
}

/// The bundled file that renders an internal page: `src/ui/<page>.html[?query]`.
#[must_use]
pub fn internal_file(internal: &str) -> Option<String> {
    let url = Url::parse(internal).ok()?;
    let normal = internal_from_url(&url)?;
    let page = url.host_str()?;
    let query = normal.split_once('?').map(|(_, q)| q);
    Some(match query {
        Some(q) => format!("src/ui/{page}.html?{q}"),
        None => format!("src/ui/{page}.html"),
    })
}

/// The internal page a bundled file URL renders, the reverse of [`internal_file`].
#[must_use]
pub fn internal_from_file(url: &Url) -> Option<String> {
    let file = url.path().strip_prefix("/src/ui/")?.strip_suffix(".html")?;
    if !INTERNAL_PAGES.contains(&file) {
        return None;
    }
    Some(match url.query() {
        Some(q) if !q.is_empty() => format!("{INTERNAL_SCHEME}://{file}?{q}"),
        _ => format!("{INTERNAL_SCHEME}://{file}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const B32: &str = "ukeu3k5oycgaauneqgtnvselmt4yemvoilkln7jpvamvfx7dnkdq.b32.i2p";

    fn web(input: &str) -> String {
        match classify(input) {
            Target::Web(url) => url.to_string(),
            other => panic!("{input}: expected Web, got {other:?}"),
        }
    }

    fn refused(input: &str) -> Refusal {
        match classify(input) {
            Target::Refused(r) => r,
            other => panic!("{input}: expected Refused, got {other:?}"),
        }
    }

    #[test]
    fn bare_i2p_host_gets_http() {
        assert_eq!(web("foo.i2p"), "http://foo.i2p/");
        assert_eq!(web("  stats.i2p/path "), "http://stats.i2p/path");
    }

    #[test]
    fn b32_url_with_path_and_query_loads() {
        let input = format!("http://{B32}/p?q");
        assert_eq!(web(&input), input);
    }

    #[test]
    fn https_i2p_loads() {
        assert_eq!(web("https://foo.i2p"), "https://foo.i2p/");
    }

    #[test]
    fn lookalike_and_loopback_hosts_are_not_i2p() {
        for input in [
            "foo.i2p.evil.com",
            "127.0.0.1",
            "http://127.0.0.1:7657/",
            "localhost:8080",
            "http://localhost/",
            "http://[::1]/",
            "http://i2p/",
            "http://.i2p/",
            "http://foo..i2p/",
            "ftp://foo.i2p/",
        ] {
            assert_eq!(refused(input), Refusal::NotI2p, "{input}");
        }
    }

    #[test]
    fn bare_localhost_is_a_search_not_a_load() {
        assert_eq!(classify("localhost"), Target::Search("localhost".into()));
    }

    #[test]
    fn dangerous_schemes_are_refused() {
        for input in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,<b>x</b>",
            "about:config",
        ] {
            assert_eq!(refused(input), Refusal::NotI2p, "{input}");
        }
    }

    #[test]
    fn userinfo_tricks_are_refused() {
        assert_eq!(refused("http://a@evil.com#.i2p"), Refusal::NotI2p);
        assert_eq!(refused("http://user:pw@foo.i2p/"), Refusal::NotI2p);
    }

    #[test]
    fn uppercase_hosts_are_lowered() {
        assert_eq!(web("HTTP://STATS.I2P/A"), "http://stats.i2p/A");
        assert_eq!(web("Stats.I2P"), "http://stats.i2p/");
    }

    #[test]
    fn idn_hosts_are_refused() {
        assert_eq!(refused("http://bücher.i2p/"), Refusal::NotI2p);
        assert_eq!(refused("http://xn--bcher-kva.i2p/"), Refusal::NotI2p);
        // Cyrillic "і" in the TLD: the host does not end in ".i2p".
        assert_eq!(refused("http://foo.\u{456}2p/"), Refusal::NotI2p);
    }

    #[test]
    fn trailing_dot_is_trimmed_in_the_address_bar() {
        assert_eq!(web("foo.i2p."), "http://foo.i2p/");
        assert_eq!(web("http://foo.i2p./x"), "http://foo.i2p/x");
        assert_eq!(web("Foo.I2P."), "http://foo.i2p/");
        assert_eq!(web("http://foo.i2p:8080/"), "http://foo.i2p:8080/");
        assert_eq!(refused("http://foo.i2p:0/"), Refusal::NotI2p);
    }

    #[test]
    fn internal_pages() {
        assert_eq!(
            classify("eepview://home"),
            Target::Internal("eepview://home".into())
        );
        assert_eq!(
            classify("EEPVIEW://home/"),
            Target::Internal("eepview://home".into())
        );
        assert_eq!(
            classify("eepview://history?q=x"),
            Target::Internal("eepview://history?q=x".into())
        );
        assert_eq!(refused("eepview://nope"), Refusal::Invalid);
        assert_eq!(refused("eepview://home/deeper"), Refusal::Invalid);
        assert_eq!(refused("eepview:home"), Refusal::Invalid);
    }

    #[test]
    fn search_and_invalid() {
        assert_eq!(refused("hello world"), Refusal::Invalid);
        assert_eq!(refused("localhost:8080"), Refusal::NotI2p);
        assert_eq!(classify("forum"), Target::Search("forum".into()));
        assert_eq!(refused("   "), Refusal::Invalid);
        assert_eq!(refused("http://"), Refusal::Invalid);
        assert_eq!(refused("1a:b"), Refusal::Invalid);
        assert_eq!(Refusal::Invalid.as_str(), "invalid");
        assert_eq!(Refusal::NotI2p.as_str(), "not-i2p");
    }

    #[test]
    fn tab_guard_refuses_internal_and_clearnet() {
        let parse = |s: &str| Url::parse(s).unwrap();
        assert!(guard(&parse("http://stats.i2p/")));
        assert!(guard(&parse(&format!("https://{B32}/"))));
        assert!(guard(&parse("about:blank")));
        assert!(!guard(&parse("eepview://home")));
        assert!(!guard(&parse("http://example.com/")));
        assert!(!guard(&parse("http://foo.i2p./")));
        assert!(!guard(&parse("tauri://localhost/src/ui/home.html")));
        assert!(!guard(&parse("http://127.0.0.1/")));
    }

    #[test]
    fn helpers() {
        assert_eq!(host_of("http://stats.i2p/x").as_deref(), Some("stats.i2p"));
        assert_eq!(host_of("http://example.com/"), None);
        assert_eq!(host_of("::"), None);
        assert!(is_web("https://a.i2p/") && !is_web("eepview://home"));
        assert_eq!(
            internal_with("blocked", &[("url", "http://a b/")]),
            "eepview://blocked?url=http%3A%2F%2Fa+b%2F"
        );
        assert_eq!(internal_with("home", &[]), "eepview://home");
    }

    #[test]
    fn internal_file_round_trip() {
        assert_eq!(
            internal_file("eepview://home").as_deref(),
            Some("src/ui/home.html")
        );
        assert_eq!(
            internal_file("eepview://blocked?url=x").as_deref(),
            Some("src/ui/blocked.html?url=x")
        );
        assert_eq!(internal_file("eepview://nope"), None);
        assert_eq!(internal_file("not a url"), None);
        let file = Url::parse("tauri://localhost/src/ui/blocked.html?url=x").unwrap();
        assert_eq!(
            internal_from_file(&file).as_deref(),
            Some("eepview://blocked?url=x")
        );
        let plain = Url::parse("http://localhost:1420/src/ui/home.html").unwrap();
        assert_eq!(
            internal_from_file(&plain).as_deref(),
            Some("eepview://home")
        );
        let other = Url::parse("http://localhost:1420/src/ui/toolbar.html").unwrap();
        assert_eq!(internal_from_file(&other), None);
        assert_eq!(
            internal_from_url(&Url::parse("http://home/").unwrap()),
            None
        );
    }
}
