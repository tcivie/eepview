// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Property tests of the host predicate, the address bar, the engine rules and the gatekeeper
//! request planner. Each property names the requirement it checks (`Req:`).
//!
//! Sources of the requirements: `docs/wiki/adr-0001-no-leak-architecture.md` (ADR),
//! `docs/wiki/ipc-contract.md` (IPC) and the doc comments of the public functions.

use eepview_lib::nav::{Target, classify, guard, is_allowed_web};
use eepview_lib::net::host::is_i2p_host;
use eepview_lib::net::http::{Head, Plan, plan, rewrite_response};
use eepview_lib::net::rules::{csp_header, engine_allows};
use proptest::prelude::*;
use tauri::Url;

/// Printable text, tricky ASCII, IDN, userinfo, ports, IPv6 literals, trailing dots.
fn tricky() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        "\\PC{0,120}",
        "[a-zA-Z0-9.:@\\[\\]/\\\\?#%_ -]{0,80}",
        "[a-z0-9-]{1,10}(\\.[a-z0-9-]{1,10}){0,3}\\.i2p\\.?(:[0-9]{1,6})?(/[a-z0-9]{0,8})?",
        "[a-z\\x{e0}-\\x{ff}\\x{400}-\\x{4ff}\\x{3002}\\x{ff0e}.-]{0,30}\\.[iI\\x{456}]2[pP]",
        "(https?|ftp|file|data|javascript)://([a-z0-9]+@)?[a-zA-Z0-9.\\[\\]:-]{0,30}(/[a-z]{0,5})?",
    ]
}

/// URLs that sit next to the rule edges: IDN labels, `b32` names of every length, port 0.
fn host_like() -> impl Strategy<Value = String> {
    prop_oneof![
        "https?://(xn--)?[a-z0-9-]{0,8}(\\.[a-z0-9-]{0,8}){0,2}\\.i2p(:[0-9]{1,2})?/[a-z]{0,3}",
        "https?://([a-z0-9]{0,6}\\.)?[a-z2-7]{0,60}\\.b32\\.i2p(:[0-9]{1,2})?/",
        "https?://(b32|a\\.b32|b32\\.a)\\.i2p(:[0-9]{1,2})?/",
    ]
}

/// A host that is valid by the rule of the ADR: lower-case labels, `.i2p`.
fn plain_i2p_host() -> impl Strategy<Value = String> {
    "[a-ac-z0-9][a-z0-9-]{0,10}(\\.[a-ac-z0-9][a-z0-9]{0,10}){0,3}\\.i2p"
}

#[cfg(test)]
mod adr {
    use eepview_lib::net::rules::content_rule_list;
    use regex::Regex;
    use serde_json::Value;

    /// One rule of the `WKContentRuleList`: its regex and what it does.
    struct Rule {
        filter: Regex,
        blocks: bool,
    }

    fn rule(json: &Value) -> Rule {
        let pattern = json["trigger"]["url-filter"].as_str().unwrap();
        let sensitive = json["trigger"]["url-filter-is-case-sensitive"] == true;
        let case = if sensitive { "" } else { "(?i)" };
        Rule {
            filter: Regex::new(&format!("{case}{pattern}")).unwrap(),
            blocks: json["action"]["type"] == "block",
        }
    }

    /// The L3b rule list (ADR 0001: block all, then allow `.i2p`, `about:`, `data:`, `blob:`) as
    /// an oracle that does not call the guard or the host predicate. A rule that blocks marks
    /// the URL blocked; `ignore-previous-rules` clears that mark, as `WebKit` does.
    pub fn allows(url: &str) -> bool {
        let list = content_rule_list();
        let step = |blocked: bool, rule: Rule| {
            [blocked, rule.blocks][usize::from(rule.filter.is_match(url))]
        };
        !list.as_array().unwrap().iter().map(rule).fold(false, step)
    }
}

fn head_of(text: &str) -> Option<Head> {
    Head::parse(text.as_bytes())
}

proptest! {
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    // Req: ADR L1 "is_i2p_host: lowercase, no trailing dot, no userinfo, IDN refused";
    // the predicate never panics, and accepts only lower-case ASCII *.i2p names.
    #[test]
    fn host_predicate_accepts_only_plain_i2p_names(host in tricky()) {
        if is_i2p_host(&host) {
            prop_assert!(host.strip_suffix(".i2p").is_some());
            prop_assert!(host.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-'));
            prop_assert!(!host.starts_with('.') && !host.contains(".."));
            prop_assert!(!host.contains("xn--"));
        }
    }

    // Req: ADR "One host predicate is the single source of truth for L1, L3 and L4": a host
    // that passes it passes the L4 guard, the engine rule (L3b, the same rule list the engine
    // gets), and the gatekeeper planner (L1). PO decision: an explicit port 1-65535 on a .i2p
    // URL is allowed by every layer.
    #[test]
    fn the_three_layers_agree_on_accepted_hosts(
        host in plain_i2p_host(),
        port in prop::option::of(1u32..=65535),
    ) {
        prop_assume!(is_i2p_host(&host));
        let authority = port.map_or(host.clone(), |p| format!("{host}:{p}"));
        let url = format!("http://{authority}/x?y=1");
        let parsed = Url::parse(&url).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(guard(&parsed), "L4 refuses {url}");
        prop_assert!(adr::allows(parsed.as_str()), "the ADR L3 rule refuses {url}");
        let head = head_of(&format!("GET {url} HTTP/1.1\r\n\r\n"));
        let accepted = matches!(head.map(|h| plan(&h, false)), Some(Plan::Http { .. }));
        prop_assert!(accepted, "L1 refuses {url}");
        prop_assert!(matches!(classify(&url), Target::Web(_)), "the address bar refuses {url}");
    }

    // Req: ADR "One host predicate is the single source of truth for L1, L3 and L4": for any
    // http(s) URL, the L3b rule list allows exactly what the L4 guard allows. Found by the fuzz
    // target `url_rules` (docs/wiki/fuzzing.md).
    #[test]
    fn the_l3b_rule_list_allows_exactly_what_the_guard_allows(
        input in prop_oneof![tricky(), host_like()],
    ) {
        let Ok(url) = Url::parse(&input) else { return Ok(()); };
        if matches!(url.scheme(), "http" | "https") {
            prop_assert_eq!(adr::allows(url.as_str()), guard(&url), "{}", url);
        }
    }

    // Req: PO decision "the address bar lowercases the host and strips one trailing dot": any
    // letter case and one trailing dot give the lower-case URL; two trailing dots are refused.
    #[test]
    fn address_bar_lowercases_and_strips_one_trailing_dot(
        host in plain_i2p_host(),
        flips in prop::collection::vec(any::<bool>(), 64),
        dots in 0..3usize,
        scheme in prop_oneof![Just(""), Just("http://"), Just("https://")],
    ) {
        prop_assume!(is_i2p_host(&host));
        let typed: String = host
            .chars()
            .zip(flips.iter().cycle())
            .map(|(c, up)| if *up { c.to_ascii_uppercase() } else { c })
            .collect();
        let input = format!("{scheme}{typed}{}", ".".repeat(dots));
        let target = classify(&input);
        if dots < 2 {
            let proto = if scheme.is_empty() { "http://" } else { scheme };
            let want = Url::parse(&format!("{proto}{host}/")).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(target, Target::Web(want), "{}", input);
        } else {
            prop_assert!(!matches!(target, Target::Web(_)), "{input} loads");
        }
    }

    // Req: PO decision "explicit port 1-65535 is allowed": the address bar loads it; port 0 is
    // refused.
    #[test]
    fn address_bar_ports(host in plain_i2p_host(), port in 1u32..=65535) {
        prop_assume!(is_i2p_host(&host));
        let load = |input: String| matches!(classify(&input), Target::Web(_));
        prop_assert!(load(format!("{host}:{port}")), "{host}:{port}");
        prop_assert!(load(format!("http://{host}:{port}/p")), "{host}:{port}");
        prop_assert!(!load(format!("http://{host}:0/")), "port 0");
        prop_assert!(!load(format!("http://{host}:65536/")), "port 65536");
    }

    // Req: ADR L4 "only the same .i2p rule": a URL the guard allows is allowed by the ADR L3
    // rule too, the engine filter allows nothing beyond the rule, and the guard only ever allows http(s) .i2p URLs without user info.
    #[test]
    fn guard_is_never_wider_than_the_engine_rule(input in tricky()) {
        let Ok(url) = Url::parse(&input) else { return Ok(()); };
        if guard(&url) {
            prop_assert!(adr::allows(url.as_str()), "{url}");
        }
        // Req: ADR L3b "block all, then allow .i2p and about:, data:, blob:": the engine filter
        // is never wider than that rule (checked against the ADR patterns, not against `guard`).
        if engine_allows(url.as_str()) {
            prop_assert!(adr::allows(url.as_str()), "the engine filter allows {url}");
        }
        if is_allowed_web(&url) {
            prop_assert!(matches!(url.scheme(), "http" | "https"));
            prop_assert!(url.username().is_empty() && url.password().is_none());
            prop_assert!(url.host_str().is_some_and(is_i2p_host));
        }
    }

    // Req: IPC `navigate`: anything that is not an I2P site is refused. The address bar never
    // panics, and `Target::Web` is always a URL the L4 guard accepts.
    #[test]
    fn address_bar_returns_web_only_for_guarded_i2p_urls(input in tricky()) {
        if let Target::Web(url) = classify(&input) {
            prop_assert!(guard(&url), "{input:?} -> {url}");
            prop_assert!(url.host_str().is_some_and(is_i2p_host));
        }
    }

    // Req: ADR L4 "file:, data:, javascript:, custom schemes" never load; IPC "anything else is
    // refused".
    #[test]
    fn dangerous_schemes_are_never_loaded(
        scheme in "(?i-u:file|data|javascript)",
        sep in prop_oneof![Just(":"), Just("://")],
        rest in "\\PC{0,40}",
    ) {
        let input = format!("{scheme}{sep}{rest}");
        let target = classify(&input);
        prop_assert!(matches!(target, Target::Refused(_)), "{input:?} -> {target:?}");
    }

    // Req: ADR L4 "custom schemes" never load.
    #[test]
    fn custom_schemes_with_authority_are_never_loaded(
        scheme in "(?i-u:ftp|ws|wss|chrome|view-source|tauri|ipfs|ssh|gopher|eepview2)",
        rest in "\\PC{0,40}",
    ) {
        let input = format!("{scheme}://{rest}");
        let target = classify(&input);
        prop_assert!(matches!(target, Target::Refused(_)), "{input:?} -> {target:?}");
    }

    // Req: IPC `navigate`: `foo.i2p` becomes `http://foo.i2p/`.
    #[test]
    fn bare_i2p_host_becomes_an_http_url(host in plain_i2p_host()) {
        prop_assume!(is_i2p_host(&host));
        let want = format!("http://{host}/");
        prop_assert_eq!(classify(&host), Target::Web(Url::parse(&want).map_err(|e| TestCaseError::fail(e.to_string()))?));
    }

    // Req: IPC `navigate` / owner decision: one word with no dot, no colon and no scheme
    // searches history and bookmarks.
    #[test]
    fn one_word_searches(word in "[a-z0-9]{1,12}") {
        prop_assert!(matches!(classify(&word), Target::Search(_)), "{word:?}");
    }

    // Req: owner decision "one word ... searches; anything else that is not on *.i2p is refused":
    // text with spaces inside never searches and never loads.
    #[test]
    fn several_words_are_refused(text in "[a-z]{1,8}( [a-z]{1,8}){1,3}") {
        prop_assert!(matches!(classify(&text), Target::Refused(_)), "{text:?}");
    }

    // Req: IPC `navigate`: an `http(s)` URL on a `.i2p` host loads (with a path, query, fragment).
    #[test]
    fn http_and_https_i2p_urls_load(
        host in plain_i2p_host(),
        secure in any::<bool>(),
        path in "(/[a-z0-9._~-]{0,8}){0,3}(\\?[a-z0-9=&]{0,8})?",
    ) {
        prop_assume!(is_i2p_host(&host));
        let url = format!("{}://{host}{path}", if secure { "https" } else { "http" });
        prop_assert!(matches!(classify(&url), Target::Web(_)), "{url}");
    }

    // Req: ADR L1 "forwards only when the host passes is_i2p_host; everything else 403, no
    // upstream", "CONNECT only to *.i2p:80 or *.i2p:443". The planner never panics and every
    // host it forwards, terminates or relays is an I2P host.
    #[test]
    fn planner_never_names_a_non_i2p_host(
        method in prop_oneof![Just("GET".to_owned()), Just("POST".to_owned()), Just("CONNECT".to_owned()), "[A-Za-z]{1,8}"],
        target in tricky(),
        version in prop_oneof![Just("HTTP/1.1".to_owned()), Just("HTTP/1.0".to_owned()), Just("HTTP/2".to_owned()), "\\PC{0,8}"],
        allow_tls in any::<bool>(),
    ) {
        let text = format!("{method} {target} {version}\r\nHost: x\r\n\r\n");
        let Some(head) = head_of(&text) else { return Ok(()); };
        match plan(&head, allow_tls) {
            Plan::Http { host, .. } => prop_assert!(is_i2p_host(&host)),
            Plan::Terminate { host } => prop_assert!(is_i2p_host(&host)),
            Plan::Relay { host } => {
                prop_assert!(allow_tls, "Relay although TLS tunnels are closed");
                prop_assert!(is_i2p_host(&host));
            }
            Plan::Refuse(_) => {}
        }
    }

    // Req: ADR L1 "CONNECT goes only to *.i2p:80 (terminated and checked) or *.i2p:443 (relayed
    // only where TLS tunnels are open); everything else gets 403" (PO ruling: 80 and 443,
    // every other port refused).
    #[test]
    fn connect_only_to_i2p_ports_80_and_443(host in tricky(), port in 0u32..70000, allow_tls in any::<bool>()) {
        let text = format!("CONNECT {host}:{port} HTTP/1.1\r\n\r\n");
        let Some(head) = head_of(&text) else { return Ok(()); };
        match plan(&head, allow_tls) {
            Plan::Terminate { host: h } => prop_assert!(port == 80 && is_i2p_host(&h)),
            Plan::Relay { host: h } => prop_assert!(allow_tls && port == 443 && is_i2p_host(&h)),
            Plan::Http { .. } => prop_assert!(false, "CONNECT planned as plain HTTP"),
            Plan::Refuse(_) => {}
        }
    }

    // Req: the same rule, from the accepting side: an I2P host on port 80 is always accepted,
    // on port 443 when TLS tunnels are open, and any other port is refused.
    #[test]
    fn connect_to_i2p_ports_is_decided_by_port(host in plain_i2p_host(), port in 0u32..70000) {
        prop_assume!(is_i2p_host(&host));
        let head = head_of(&format!("CONNECT {host}:{port} HTTP/1.1\r\n\r\n"))
            .ok_or_else(|| TestCaseError::fail("unparsable"))?;
        let open = plan(&head, true);
        let closed = plan(&head, false);
        let right = match port {
            80 => matches!((&open, &closed), (Plan::Terminate { .. }, Plan::Terminate { .. })),
            443 => matches!((&open, &closed), (Plan::Relay { .. }, Plan::Refuse(_))),
            _ => matches!((&open, &closed), (Plan::Refuse(_), Plan::Refuse(_))),
        };
        prop_assert!(right, "port {port}: {open:?} / {closed:?}");
    }

    // Req: the planner and the parser never panic on arbitrary bytes (a malformed request head
    // is refused, not a crash).
    #[test]
    fn head_parser_survives_any_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        if let Some(head) = Head::parse(&bytes) {
            let _ = plan(&head, true);
            let _ = head.body_length();
            let _ = rewrite_response(head);
        }
    }

    // Req: ADR L3a "the gatekeeper adds a Content-Security-Policy to every response": whatever
    // headers the router sends, the response head the engine gets carries the policy.
    #[test]
    fn every_response_head_carries_the_policy(
        status in 100u16..600,
        headers in proptest::collection::vec(("[A-Za-z-]{1,20}", "[ -~]{0,40}"), 0..8),
    ) {
        let lines = headers.iter().fold(String::new(), |acc, (n, v)| acc + n + ": " + v + "\r\n");
        let text = format!("HTTP/1.1 {status} X\r\n{lines}\r\n");
        let Some(head) = head_of(&text) else { return Ok(()); };
        let out = rewrite_response(head);
        let parsed = Head::parse(&out).ok_or_else(|| TestCaseError::fail("unparsable output"))?;
        let policy: Vec<&str> = parsed
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("content-security-policy"))
            .map(|(_, v)| v.as_str())
            .collect();
        prop_assert!(policy.contains(&csp_header()), "no gatekeeper policy in {parsed:?}");
    }
}

// Req: ADR L3a "every fetch directive allows only http(s)://*.i2p:* (plus data: and blob: for
// images, fonts and media)": no source in the policy names another origin, `*`, a bare scheme
// other than data: and blob:, or 'self' (the loopback gatekeeper).
#[test]
fn policy_lists_only_i2p_sources() {
    let allowed = ["http://*.i2p:*", "https://*.i2p:*", "data:", "blob:"];
    let mut directives = 0;
    for directive in csp_header()
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        directives += 1;
        for source in directive.split_whitespace().skip(1) {
            let keyword = source.starts_with('\'');
            assert!(
                (keyword && source != "'self'") || allowed.contains(&source),
                "{directive}: {source}"
            );
        }
    }
    assert!(directives > 0);
    assert!(
        csp_header().contains("default-src"),
        "no fallback for the fetch directives"
    );
}

// Req: ADR L1 "is_i2p_host: lowercase, no trailing dot, no userinfo, IDN refused", from a table
// of hostile hosts.
#[test]
fn known_tricks_are_refused_by_the_predicate() {
    for host in [
        "foo.i2p.",
        "FOO.I2P",
        "xn--bcher-kva.i2p",
        "b\u{fc}cher.i2p",
        "foo.\u{456}2p",
        "a@foo.i2p",
        "foo.i2p:80",
        "foo.i2p.evil.com",
        ".i2p",
        "i2p",
        "",
    ] {
        assert!(!is_i2p_host(host), "{host:?}");
    }
}

// Req: IPC `navigate`: `.b32.i2p` hosts load. An I2P base32 address is 52 characters of
// a-z2-7 (a 256-bit hash).
#[test]
fn b32_addresses_load() {
    let host = format!(
        "{}.b32.i2p",
        "ukeu3k5oycgaauneqgtnvselmt4yemvoilkln7jpvamvfx7dnkdq"
    );
    assert!(is_i2p_host(&host));
    assert!(matches!(
        classify(&format!("http://{host}/")),
        Target::Web(_)
    ));
    assert!(matches!(classify(&host), Target::Web(_)));
}

// Req: robustness. Very long input is refused or searched, never a crash or a hang.
#[test]
fn very_long_input_is_handled() {
    for len in [10_000, 200_000, 1_000_000] {
        let dots = ".a".repeat(len / 2);
        let long = [
            "a".repeat(len),
            format!("http://a{dots}.i2p/"),
            format!("a{dots}.i2p"),
            "é".repeat(len),
        ];
        for input in long {
            let _ = classify(&input);
            let _ = is_i2p_host(&input);
        }
    }
}

// Req: ADR L1 "forwards a request only when its host passes is_i2p_host", for absolute URIs
// that engines read in a tricky way. The expected reading is the WHATWG one that WebKit and
// WebView2 follow: `None` means the engine reads a non-I2P host (or user info), so the planner
// must refuse; `Some(host)` means a forwarded request may name only that host.
#[test]
fn tricky_absolute_uris_name_the_engine_host() {
    let table: [(&str, Option<&str>); 17] = [
        ("http://foo.i2p@evil.com/", None),
        ("http://evil.com#@foo.i2p/", None),
        ("http://evil.com?@foo.i2p/", None),
        ("http://evil.com/@foo.i2p/", None),
        ("http://evil.com\\@foo.i2p/", None),
        ("http://evil.com\\.i2p/", None),
        ("http://evil.com:80@foo.i2p/", None),
        ("http://user:pw@foo.i2p/", None),
        ("http://foo.i2p%2eevil.com/", None),
        ("http://foo.i2p./", None),
        ("http://foo.i2p..evil.com/", None),
        ("http://[::1]/", None),
        ("http://2130706433/", None),
        ("http://0x7f.1/", None),
        ("http://foo.i2p\\@evil.com/", Some("foo.i2p")),
        ("http://foo.i2p#.evil.com/", Some("foo.i2p")),
        ("http://FOO.I2P/", Some("foo.i2p")),
    ];
    for (target, want) in table {
        let head = head_of(&format!("GET {target} HTTP/1.1\r\n\r\n")).unwrap();
        match (plan(&head, true), want) {
            (Plan::Http { host, .. }, Some(want)) => assert_eq!(host, want, "{target}"),
            (Plan::Refuse(_), _) => {}
            (other, _) => panic!("{target}: {other:?}"),
        }
        if want.is_none() {
            assert!(
                !matches!(plan(&head, true), Plan::Http { .. }),
                "{target} was forwarded"
            );
        }
    }
}

/// Whether the guard (L4), the L3b rule list, the planner (L1) and the address bar accept a
/// URL. `None` when they disagree.
fn layers_accept(url: &str) -> Option<bool> {
    let parsed = Url::parse(url).ok();
    let l4 = parsed.as_ref().is_some_and(guard);
    let l3 = parsed.as_ref().is_some_and(|u| adr::allows(u.as_str()));
    let head = head_of(&format!("GET {url} HTTP/1.1\r\n\r\n"));
    let l1 = matches!(head.map(|h| plan(&h, false)), Some(Plan::Http { .. }));
    let bar = matches!(classify(url), Target::Web(_));
    (l4 == l3 && l3 == l1 && l1 == bar).then_some(l4)
}

// Req: ADR "One host predicate is the single source of truth ... IDN is rejected": the guard
// (L4), the L3b rule list, the gatekeeper planner (L1) and the address bar refuse the same
// forms, and accept a 52-character `b32` name. The inputs are the ones the fuzz target
// `url_rules` found (docs/wiki/fuzzing.md), and their neighbours.
#[test]
fn the_three_layers_agree_on_the_rule_edges() {
    let b32 = "a".repeat(52);
    let accepted = [
        format!("http://{b32}.b32.i2p/"),
        format!("http://{}.b32.i2p:8080/x", "7".repeat(60)),
        "http://ab32.i2p/".to_owned(),
        "http://b32.foo.i2p/".to_owned(),
        "http://foo.b32x.i2p/".to_owned(),
    ];
    // An invalid IDN label does not even parse, which refuses it too.
    let refused = [
        "http://xn--tt88fi-xph5e.i2p/".to_owned(),
        "http://xn--a.i2p/".to_owned(),
        "http://sub.xn--a.i2p/".to_owned(),
        "http://a.xn--a.sub.i2p:8080/".to_owned(),
        "http://b32.i2p/".to_owned(),
        "http://short.b32.i2p/".to_owned(),
        format!("http://{}.b32.i2p/", "a".repeat(51)),
        format!("http://x.{b32}.b32.i2p/"),
        format!("http://{}1.b32.i2p/", "a".repeat(51)),
        "http://foo.i2p:0/".to_owned(),
        format!("http://{b32}.b32.i2p:0/"),
    ];
    for url in &accepted {
        assert_eq!(layers_accept(url), Some(true), "{url}");
    }
    for url in &refused {
        assert_eq!(layers_accept(url), Some(false), "{url}");
    }
}
