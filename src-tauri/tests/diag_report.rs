// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R6.1 to R6.8 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! the report text and the prefilled issue URL (`eepview_lib::diag::report`).
//! Public API only. The properties check the privacy rule and the URL length cap.

use eepview_lib::diag::report::{
    self, ISSUE_PREFIX, MAX_URL, PREVIEW_EVENTS, Report, ReportKind, TRIM_NOTE, file_name,
    is_issue_url, issue_url, percent_encode, text,
};
use eepview_lib::diag::sysinfo::{RouterKind, SystemInfo, UptimeBucket};
use eepview_lib::diag::{REMOVED, RouterState};
use proptest::prelude::*;

const PREFIX: &str = "https://github.com/tcivie/eepview/issues/new";

fn info() -> SystemInfo {
    SystemInfo {
        version: "0.1.0".to_owned(),
        commit: "abc1234".to_owned(),
        os: "macOS 14.5".to_owned(),
        arch: "aarch64",
        engine: "WebKit 621.1.15".to_owned(),
        router_kind: RouterKind::I2pd,
        router_version: Some("2.50.0".to_owned()),
        router_state: RouterState::Ok,
        managed: true,
        js_default: true,
        tabs: 2,
        uptime: UptimeBucket::from_secs(30),
    }
}

fn report_of(description: &str, log: Vec<String>, include_log: bool) -> Report {
    Report {
        kind: ReportKind::General,
        description: description.to_owned(),
        info: info(),
        log,
        include_log,
    }
}

fn log_lines(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("INFO gatekeeper-refused refuse=not-i2p count={i:04}"))
        .collect()
}

/// The decoded value of a query parameter of the issue URL.
fn param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    let pair = query
        .split('&')
        .find_map(|p| p.strip_prefix(&format!("{key}=")))?;
    decode(pair)
}

fn decode(encoded: &str) -> Option<String> {
    let bytes = encoded.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

// R6.1: the kind from the page parameter.
#[test]
fn r6_1_kind_from_param() {
    let table = [
        ("crash", "Crash report"),
        ("blocked", "Blocked page report"),
        ("router-down", "Router down report"),
        ("load-failed", "Page load report"),
        ("general", "Problem report"),
        ("", "Problem report"),
        ("anything-else", "Problem report"),
        ("<script>", "Problem report"),
    ];
    for (param, title) in table {
        assert_eq!(ReportKind::from_param(param).title(), title, "`{param}`");
    }
}

// R6.1: the title of every kind.
#[test]
fn r6_1_titles_of_every_kind() {
    assert_eq!(ReportKind::General.title(), "Problem report");
    assert_eq!(ReportKind::Crash.title(), "Crash report");
    assert_eq!(ReportKind::Blocked.title(), "Blocked page report");
    assert_eq!(ReportKind::RouterDown.title(), "Router down report");
    assert_eq!(ReportKind::LoadFailed.title(), "Page load report");
}

// R6.3: the text has the three parts, in order.
#[test]
fn r6_3_text_has_description_system_and_log() {
    let log = log_lines(2);
    let body = text(&report_of(
        "It broke when I clicked reload",
        log.clone(),
        true,
    ));
    let system = info().lines().join("\n");
    assert!(
        body.starts_with("What happened:\nIt broke when I clicked reload\n\nSystem:\n"),
        "{body}"
    );
    assert!(body.contains(&system), "{body}");
    let header = body
        .find("Diagnostics log (last 2 events):\n")
        .expect("log header");
    assert!(body.find("System:").expect("system") < header);
    let first = body.find(&log[0]).expect("first log line");
    let second = body.find(&log[1]).expect("second log line");
    assert!(
        header < first && first < second,
        "log lines in order after the header"
    );
}

// R6.3: a blank description reads `(not given)`.
#[test]
fn r6_3_blank_description_is_not_given() {
    for blank in ["", "   ", "\n\t "] {
        let body = text(&report_of(blank, vec![], true));
        assert!(
            body.starts_with("What happened:\n(not given)\n"),
            "`{blank}`: {body}"
        );
    }
}

// R6.3: without the log, the log part is left out.
#[test]
fn r6_3_log_part_is_left_out_when_not_included() {
    let log = log_lines(3);
    let body = text(&report_of("broke", log.clone(), false));
    assert!(!body.contains("Diagnostics log"), "{body}");
    assert!(!body.contains(&log[0]), "{body}");
    assert!(body.contains("System:\n"), "{body}");
}

// R6.3: the preview limit.
#[test]
fn r6_3_preview_events_is_200() {
    assert_eq!(PREVIEW_EVENTS, 200);
}

// R6.3: the text is the file, so it keeps every log line even when the URL trims them.
#[test]
fn r6_3_text_keeps_all_log_lines() {
    let log = log_lines(200);
    let body = text(&report_of("broke", log.clone(), true));
    assert!(
        body.contains("Diagnostics log (last 200 events):"),
        "{body}"
    );
    for line in &log {
        assert!(body.contains(line.as_str()), "missing {line}");
    }
}

// R6.3 + R4: the text is scrubbed, in the description and in the log.
#[test]
fn r6_3_text_is_scrubbed() {
    let log = vec!["INFO x peer 192.168.1.20 http://leak.example/x".to_owned()];
    let body = text(&report_of(
        "I opened https://secret.example.com/a and bob@example.org",
        log,
        true,
    ));
    for secret in ["secret.example.com", "bob@", "192.168.1.20", "leak.example"] {
        assert!(!body.contains(secret), "`{secret}` in {body}");
    }
    assert!(body.contains(REMOVED));
}

// R6.4: the URL starts with the prefix, and the parameters follow in this order.
#[test]
fn r6_4_url_has_the_parameters_in_order() {
    assert_eq!(ISSUE_PREFIX, PREFIX);
    let (url, trimmed) = issue_url(&report_of("plain words", log_lines(2), true));
    assert!(!trimmed);
    assert!(
        url.starts_with(&format!("{PREFIX}?template=bug.yml&labels=bug&title=")),
        "{url}"
    );
    let mut last = 0;
    for key in ["&version=", "&os=", "&what-happened=", "&diagnostics="] {
        let at = url
            .find(key)
            .unwrap_or_else(|| panic!("{key} missing in {url}"));
        assert!(at > last, "{key} is out of order in {url}");
        last = at;
    }
}

// R6.4: `version` is `<version> (<commit>)`, `os` is `<os> (<arch>)`.
#[test]
fn r6_4_version_and_os_values() {
    let (url, _) = issue_url(&report_of("plain words", vec![], false));
    assert_eq!(param(&url, "version").as_deref(), Some("0.1.0 (abc1234)"));
    assert_eq!(param(&url, "os").as_deref(), Some("macOS 14.5 (aarch64)"));
}

// R6.4: `what-happened` is the description.
#[test]
fn r6_4_what_happened_is_the_description() {
    let (url, _) = issue_url(&report_of("plain words", vec![], false));
    assert!(url.contains("&what-happened=plain%20words&"), "{url}");
    assert_eq!(param(&url, "what-happened").as_deref(), Some("plain words"));
}

// R6.4: `diagnostics` holds the system lines and the log lines.
#[test]
fn r6_4_diagnostics_holds_system_and_log() {
    let log = log_lines(2);
    let (url, _) = issue_url(&report_of("plain words", log.clone(), true));
    let diagnostics = param(&url, "diagnostics").expect("diagnostics");
    assert!(diagnostics.starts_with("System:\n"), "{diagnostics}");
    for line in info().lines().iter().chain(log.iter()) {
        assert!(diagnostics.contains(line.as_str()), "missing {line}");
    }
}

// R6.4: without the log, `diagnostics` has the system part only.
#[test]
fn r6_4_diagnostics_without_log_has_no_log_lines() {
    let log = log_lines(2);
    let (url, trimmed) = issue_url(&report_of("plain words", log.clone(), false));
    let diagnostics = param(&url, "diagnostics").expect("diagnostics");
    assert!(!trimmed);
    assert!(!diagnostics.contains(&log[0]), "{diagnostics}");
}

// R11.1 + R6.3: what the issue form shows is in the preview text. The preview is the text.
#[test]
fn r6_4_everything_in_the_url_is_in_the_preview_text() {
    let log = log_lines(5);
    let rep = report_of("it broke after I clicked reload", log, true);
    let preview = text(&rep);
    let (url, trimmed) = issue_url(&rep);
    assert!(!trimmed);
    let sent = [param(&url, "what-happened"), param(&url, "diagnostics")];
    for value in sent.into_iter().flatten() {
        for line in value.lines().filter(|l| !l.trim().is_empty()) {
            assert!(
                preview.lines().any(|p| p == line),
                "`{line}` is sent but not previewed"
            );
        }
    }
}

// R6.4: every value is scrubbed before it is encoded.
#[test]
fn r6_4_values_are_scrubbed() {
    let (url, _) = issue_url(&report_of(
        "I opened https://secret.example.com/a",
        vec![],
        false,
    ));
    assert!(!url.contains("secret"), "{url}");
    assert!(
        !url.contains(&percent_encode("secret.example.com")),
        "{url}"
    );
}

// R6.5: the characters that stay, and the bytes that are encoded.
#[test]
fn r6_5_percent_encode() {
    assert_eq!(percent_encode("AZaz09-_.~"), "AZaz09-_.~");
    assert_eq!(percent_encode("a b"), "a%20b");
    assert_eq!(percent_encode("a/b?c=d&e#f%g"), "a%2Fb%3Fc%3Dd%26e%23f%25g");
    assert_eq!(percent_encode("line\nbreak"), "line%0Abreak");
    assert_eq!(percent_encode("\u{e9}"), "%C3%A9");
    assert_eq!(percent_encode("\u{1f600}"), "%F0%9F%98%80");
    assert_eq!(percent_encode(""), "");
    assert_eq!(percent_encode("+"), "%2B");
}

// R6.6: the URL is at most MAX_URL characters, the oldest log lines go first, and the
// trim note is the first line of the log part.
#[test]
fn r6_6_long_logs_are_trimmed_oldest_first() {
    assert_eq!(MAX_URL, 8000);
    let log = log_lines(200);
    let (url, trimmed) = issue_url(&report_of("broke", log.clone(), true));
    assert!(trimmed, "200 lines do not fit");
    assert!(url.len() <= 8000, "{} characters", url.len());
    let diagnostics = param(&url, "diagnostics").expect("diagnostics");
    let kept: Vec<bool> = log
        .iter()
        .map(|l| diagnostics.contains(l.as_str()))
        .collect();
    assert!(!kept[0], "the oldest line goes first");
    assert!(kept[199], "the newest line stays");
    let first_kept = kept.iter().position(|k| *k).expect("some line stays");
    assert!(
        kept[first_kept..].iter().all(|k| *k),
        "the kept lines are the newest ones"
    );
    let note = diagnostics.find(TRIM_NOTE).expect("the trim note");
    assert!(note < diagnostics.find(&log[first_kept]).expect("first kept line"));
    assert!(diagnostics.find("Diagnostics log").expect("log header") < note);
}

// R6.6: the trim note text.
#[test]
fn r6_6_trim_note_text() {
    assert_eq!(
        TRIM_NOTE,
        "[log trimmed, the full log is in the attached file]"
    );
}

// R6.6: a log that fits is not trimmed and has no note.
#[test]
fn r6_6_short_log_has_no_trim_note() {
    let (url, trimmed) = issue_url(&report_of("broke", log_lines(3), true));
    assert!(!trimmed);
    assert!(
        !param(&url, "diagnostics")
            .expect("diagnostics")
            .contains("log trimmed")
    );
}

// R6.6: when the URL is still too long without log lines, the description is cut.
#[test]
fn r6_6_long_description_is_cut() {
    let description = "word ".repeat(5000);
    for include_log in [true, false] {
        let (url, _) = issue_url(&report_of(&description, log_lines(10), include_log));
        assert!(
            url.len() <= 8000,
            "{} characters (log {include_log})",
            url.len()
        );
        assert!(url.starts_with(PREFIX));
    }
}

// R6.7: only the prefix alone, or the prefix followed by `?`, is an issue URL.
#[test]
fn r6_7_is_issue_url_accepts_the_prefix_forms() {
    assert!(is_issue_url(PREFIX));
    assert!(is_issue_url(&format!("{PREFIX}?template=bug.yml")));
    assert!(is_issue_url(&format!("{PREFIX}?")));
}

// R6.7: everything else is refused.
#[test]
fn r6_7_is_issue_url_refuses_everything_else() {
    let refused = [
        "",
        "https://github.com/tcivie/eepview/issues",
        "https://github.com/tcivie/eepview/issues/new/choose",
        "https://github.com/tcivie/eepview/issues/newer",
        "https://github.com/tcivie/eepview/issues/new#frag",
        "http://github.com/tcivie/eepview/issues/new",
        "https://github.com.evil.example/tcivie/eepview/issues/new",
        "https://evil.example/?u=https://github.com/tcivie/eepview/issues/new",
        "https://github.com/tcivie/other/issues/new",
        " https://github.com/tcivie/eepview/issues/new",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "forum.i2p",
    ];
    for url in refused {
        assert!(!is_issue_url(url), "`{url}` must be refused");
    }
}

// R6.8: the file name has no date, and a number from 2 on.
#[test]
fn r6_8_file_name() {
    assert_eq!(file_name(0), "eepview-report.txt");
    assert_eq!(file_name(1), "eepview-report.txt");
    assert_eq!(file_name(2), "eepview-report (2).txt");
    assert_eq!(file_name(3), "eepview-report (3).txt");
    assert_eq!(file_name(99), "eepview-report (99).txt");
}

fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// A private value of any of the kinds of R4.4.
fn private_value() -> impl Strategy<Value = String> {
    prop_oneof![
        "https?://zq[a-z]{4,10}\\.[a-z]{2,6}(/[a-z0-9]{1,8}){0,2}",
        "zq[a-z]{3,10}\\.i2p(/[a-z]{1,8})?",
        "[a-z2-7]{52,60}",
        (0u8..=255, 0u8..=255, 0u8..=255, 0u8..=255)
            .prop_map(|(a, b, c, d)| format!("{a}.{b}.{c}.{d}")),
        proptest::collection::vec("[0-9a-f]{1,4}", 8).prop_map(|g| g.join(":")),
        "zq[a-z]{3,8}@[a-z]{3,8}\\.[a-z]{2,4}",
        "/Users/zq[a-z]{3,10}",
        "/home/zq[a-z]{3,10}",
    ]
}

fn words() -> impl Strategy<Value = String> {
    "[A-Z ,;]{0,40}"
}

proptest! {
    #![proptest_config(config())]

    // R4.4 + R6: an injected private value is in neither the text nor the issue URL.
    #[test]
    fn r6_injected_private_values_never_reach_text_or_url(
        before in words(),
        value in private_value(),
        after in words(),
        in_log in any::<bool>(),
    ) {
        let injected = format!("{before} {value} {after}");
        let rep = if in_log {
            report_of("broke", vec![format!("INFO x {injected}")], true)
        } else {
            report_of(&injected, log_lines(3), true)
        };
        let body = text(&rep);
        prop_assert!(!body.contains(&value), "text: {}", body);
        let (url, _) = issue_url(&rep);
        prop_assert!(!url.contains(&percent_encode(&value)), "url: {}", url);
        prop_assert!(!decode(&url).unwrap_or_default().contains(&value), "url: {}", url);
    }

    // R6.5: the encoding has only safe characters, and decoding gives back the text.
    #[test]
    fn r6_5_percent_encode_round_trips(input in "\\PC{0,200}") {
        let encoded = percent_encode(&input);
        prop_assert!(encoded.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.~%".contains(&b)));
        prop_assert_eq!(decode(&encoded), Some(input));
    }

    // R6.8: the name holds no digit but the copy number, and no separator.
    #[test]
    fn r6_8_file_name_holds_no_date(n in any::<u32>()) {
        let name = file_name(n);
        let digits: String = name.chars().filter(char::is_ascii_digit).collect();
        if n < 2 {
            prop_assert_eq!(name, "eepview-report.txt");
        } else {
            prop_assert_eq!(digits, n.to_string());
            prop_assert_eq!(name, format!("eepview-report ({n}).txt"));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 16, ..config() })]

    // R6.4 + R6.6: the issue URL always starts with the prefix, is an issue URL, and stays
    // under the cap, whatever the description and the log. Fewer cases: each one builds a
    // report with up to 200 log lines.
    #[test]
    fn r6_issue_url_keeps_prefix_and_cap(
        description in "\\PC{0,3000}",
        log in proptest::collection::vec("[ -~]{0,100}", 0..=200),
        include_log in any::<bool>(),
    ) {
        let (url, _) = issue_url(&report_of(&description, log, include_log));
        prop_assert!(url.starts_with(PREFIX));
        prop_assert!(is_issue_url(&url));
        prop_assert!(url.len() <= 8000, "{} characters", url.len());
        prop_assert!(url.is_ascii());
    }
}

// R6.6: a description that is too long is cut on the raw text, at a char boundary, with `…` at the end.
#[test]
fn r6_6_cut_description_ends_with_an_ellipsis_and_is_a_prefix() {
    let description = "word ".repeat(5000);
    for include_log in [true, false] {
        let (url, _) = issue_url(&report_of(&description, log_lines(10), include_log));
        let what = param(&url, "what-happened").expect("what-happened");
        assert!(
            what.ends_with('…'),
            "log {include_log}: ends with {:?}",
            what.chars().last()
        );
        let kept = what.trim_end_matches('…');
        assert!(!kept.is_empty() && kept.len() < description.len());
        assert!(description.starts_with(kept), "the kept text is a prefix");
        assert!(url.len() <= 8000);
    }
}

// R6.6: the cut never splits a character, so the value is valid text.
#[test]
fn r6_6_cut_is_at_a_char_boundary() {
    for unit in ["é", "日", "😀"] {
        let description = unit.repeat(6000);
        let (url, _) = issue_url(&report_of(&description, vec![], false));
        let what =
            param(&url, "what-happened").unwrap_or_else(|| panic!("`{unit}`: not valid UTF-8"));
        assert!(what.ends_with('…'), "{unit}");
        assert!(
            what.trim_end_matches('…')
                .chars()
                .all(|c| unit.starts_with(c)),
            "{unit}"
        );
        assert!(url.len() <= 8000, "{unit}");
    }
}

// R6.6: the description is cut only when the URL is still too long without log lines.
#[test]
fn r6_6_a_short_description_is_kept_whole_while_the_log_is_trimmed() {
    let (url, trimmed) = issue_url(&report_of("short description", log_lines(200), true));
    assert!(trimmed);
    assert_eq!(
        param(&url, "what-happened").as_deref(),
        Some("short description")
    );
}

// R6.6: a description that fits has no ellipsis.
#[test]
fn r6_6_a_description_that_fits_is_not_cut() {
    let description = "word ".repeat(100);
    let (url, _) = issue_url(&report_of(&description, vec![], false));
    assert_eq!(
        param(&url, "what-happened").as_deref(),
        Some(description.as_str())
    );
}

// R15.6: `report::text` keeps `file=<name>` for a name of `SOURCE_FILES` and scrubs any other name.
#[test]
fn r15_6_report_text_keeps_known_source_names_only() {
    let log = vec![
        "ERROR panic file=gatekeeper.rs line=10 thread=main".to_owned(),
        "ERROR panic file=zq-private-name.rs line=11 thread=main".to_owned(),
    ];
    let body = text(&report_of("it crashed", log, true));
    assert!(
        body.contains("file=gatekeeper.rs line=10 thread=main"),
        "{body}"
    );
    assert!(!body.contains("zq-private-name"), "{body}");
}

// R15.6: every value of the issue URL does the same.
#[test]
fn r15_6_issue_url_keeps_known_source_names_only() {
    let log = vec![
        "ERROR panic file=gatekeeper.rs line=10 thread=main".to_owned(),
        "ERROR panic file=zq-private-name.rs line=11 thread=main".to_owned(),
    ];
    let (url, _) = issue_url(&report_of("it crashed", log, true));
    let diagnostics = param(&url, "diagnostics").expect("diagnostics");
    assert!(
        diagnostics.contains("file=gatekeeper.rs line=10"),
        "{diagnostics}"
    );
    assert!(!url.contains("zq-private-name"), "{url}");
    assert!(!diagnostics.contains("zq-private-name"), "{diagnostics}");
}

// R15.6: a source name outside a `file=` token is free text, so it is removed.
#[test]
fn r15_6_a_bare_source_name_in_the_description_is_removed() {
    let body = text(&report_of(
        "the crash was in gatekeeper.rs somewhere",
        vec![],
        false,
    ));
    assert!(!body.contains("gatekeeper.rs"), "{body}");
}

// R6: the module path of the free functions.
#[test]
fn r6_functions_are_reachable_by_module_path() {
    assert_eq!(report::percent_encode("a b"), "a%20b");
}
