// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R15.1 to R15.6 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! the scrub rules of the privacy review. Public API only. Examples state each rule; the
//! properties inject random private values and check that none survives.

use eepview_lib::diag::{REMOVED, SOURCE_FILES, scrub, scrub_report, scrub_with};
use proptest::prelude::*;

fn plain(text: &str) -> String {
    scrub_with(text, None, None)
}

fn assert_gone(text: &str, secrets: &[&str]) {
    let out = plain(text);
    for (case, secret) in secrets.iter().enumerate() {
        assert!(
            !out.contains(secret),
            "injected value {case} ({} bytes) survives the scrub",
            secret.len()
        );
    }
    assert!(
        out.contains(REMOVED),
        "no marker in the output ({} bytes)",
        out.len()
    );
}

// R15.1: `%2E` and `%2F` (any case) are decoded before matching, so they hide nothing.
#[test]
fn r15_1_percent_encoded_dots_and_slashes_do_not_hide_a_name() {
    assert_gone("see forum%2Ei2p now", &["forum"]);
    assert_gone("see forum%2EI2P now", &["forum"]);
    assert_gone("peer 10%2E0%2E0%2E1 failed", &["10%2E0", "10.0.0.1"]);
    assert_gone("peer 10%2e0%2e0%2e1 failed", &["10%2e0"]);
    assert_gone(
        "mail bob@mail%2Eexample%2Eorg now",
        &["bob@", "mail%2Eexample"],
    );
    assert_gone("file /Users%2Fzqalice%2FLibrary", &["zqalice"]);
    assert_gone("file /Users%2fzqalice%2fLibrary", &["zqalice"]);
    assert_gone(
        "see example%2Ecom%2Fpath now",
        &["example%2Ecom", "example.com"],
    );
}

// R15.2: a host name with a non-ASCII letter is removed, with its port and path.
#[test]
fn r15_2_internationalised_host_names_are_removed() {
    for host in [
        "bücher.example",
        "münchen.de",
        "例え.jp",
        "пример.рф",
        "café.fr",
        "zqabc.é",
    ] {
        let out = plain(&format!("visit {host}:8080/secret/path now"));
        assert!(!out.contains(host), "`{host}` survives in `{out}`");
        assert!(!out.contains("8080") && !out.contains("secret"), "{out}");
        assert!(out.contains(REMOVED), "{out}");
    }
}

// R15.2: a word with a non-ASCII letter and no dot is not a host name.
#[test]
fn r15_2_words_with_no_dot_stay() {
    let text = "Grüße aus Köln, café au lait";
    assert_eq!(plain(text), text);
}

// R15.3: a run of 32 or more hex digits with a decimal digit and a letter is removed, in any case.
#[test]
fn r15_3_hex_run_of_32_goes() {
    for run in [
        "0123456789abcdef0123456789abcdef",
        "0123456789ABCDEF0123456789ABCDEF",
        "0123456789aBcDeF0123456789AbCdEf",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ] {
        assert_gone(&format!("hash {run} end"), &[run]);
    }
}

// R15.3: a longer run goes whole.
#[test]
fn r15_3_longer_hex_run_goes_whole() {
    let run = "0123456789abcdef".repeat(8);
    let out = plain(&format!("hash {run} end"));
    assert!(!out.contains("89abcdef"), "{out}");
    assert!(out.starts_with("hash ") && out.ends_with(" end"), "{out}");
}

// R15.3: a hex run of one kind only stays: letters only (also 51 times `a`) or digits only.
#[test]
fn r15_3_single_kind_hex_runs_stay() {
    let runs = [
        "a".repeat(32),
        "a".repeat(51),
        "F".repeat(40),
        "abcdefabcdefabcdefabcdefabcdefabcdef".to_owned(),
        "0".repeat(32),
        "1234567890".repeat(5),
    ];
    for run in runs {
        let text = format!("word {run} end");
        assert_eq!(plain(&text), text, "{run}");
    }
}

// R15.3: one digit and 31 letters (or the reverse) is enough.
#[test]
fn r15_3_one_digit_and_one_letter_are_enough() {
    let runs = [
        format!("0{}", "a".repeat(31)),
        format!("{}7", "a".repeat(31)),
        format!("{}9", "F".repeat(31)),
        format!("b{}", "0".repeat(31)),
        format!("{}c", "1".repeat(40)),
    ];
    for run in runs {
        assert_gone(&format!("hash {run} end"), &[run.as_str()]);
    }
}

// R15.3: a run of 31 hex digits stays.
#[test]
fn r15_3_hex_run_of_31_stays() {
    let text = "hash 0123456789abcdef0123456789abcde end";
    assert_eq!(plain(text), text);
}

// R15.4: `Users` or `home` between two separators, each `/` or `\`, and a name with spaces.
#[test]
fn r15_4_home_folders_with_any_separators() {
    for path in [
        "/Users/zqalice/Library",
        "\\Users\\zqalice\\AppData",
        "\\Users/zqalice/AppData",
        "/home\\zqalice\\x",
        "C:\\Users/zqalice\\x",
    ] {
        assert_gone(&format!("file {path}"), &["zqalice"]);
    }
}

// R15.4: the name may hold spaces; it ends at the next separator, a quote or the line end.
#[test]
fn r15_4_home_folder_names_may_hold_spaces() {
    assert_gone(
        "file /Users/zqalice smith/Library/Logs",
        &["zqalice", "smith"],
    );
    assert_gone(
        "file C:\\Users\\zqalice smith\\AppData",
        &["zqalice", "smith"],
    );
    assert_gone("file /home/zqalice smith", &["zqalice", "smith"]);
    let quoted = plain("open \"/Users/zqalice smith\" now");
    assert!(
        !quoted.contains("zqalice") && !quoted.contains("smith"),
        "{quoted}"
    );
    assert!(
        quoted.contains(" now"),
        "the text after the quote stays: {quoted}"
    );
    let multi = plain("a /home/zqalice smith\nnext line stays");
    assert!(
        !multi.contains("zqalice") && !multi.contains("smith"),
        "{multi}"
    );
    assert!(
        multi.contains("next line stays"),
        "the next line stays: {multi}"
    );
}

// R15.4: `<drive>:\Documents and Settings\<name>`, with either separator.
#[test]
fn r15_4_documents_and_settings_folders() {
    assert_gone(
        "file C:\\Documents and Settings\\zqfrank\\Desktop",
        &["zqfrank"],
    );
    assert_gone(
        "file C:/Documents and Settings/zqfrank/Desktop",
        &["zqfrank"],
    );
    assert_gone("file d:\\Documents and Settings/zqfrank", &["zqfrank"]);
}

// R15.4: a UNC path: the server, the share and the name go.
#[test]
fn r15_4_unc_paths_go_with_server_share_and_name() {
    assert_gone(
        "file \\\\zqsrv\\zqshare\\zqgina\\docs",
        &["zqsrv", "zqshare", "zqgina"],
    );
}

// R15.5: the file endings are `txt`, `log`, `json`, `html`, `js`, `css`, `toml`, `yml` and `plist`.
#[test]
fn r15_5_the_file_endings_that_stay() {
    for name in [
        "notes.txt",
        "eepview.log",
        "data.json",
        "page.html",
        "app.js",
        "style.css",
        "Cargo.toml",
        "ci.yml",
        "Info.plist",
    ] {
        let text = format!("at {name} line 3");
        assert_eq!(plain(&text), text, "{name}");
    }
}

// R15.5: `rs`, `md` and `ts` names are removed in free text, also names of eepview's own files.
#[test]
fn r15_5_country_domain_endings_are_removed() {
    for name in ["apply.rs", "README.md", "app.ts", "lib.rs"] {
        assert_gone(&format!("at {name} line 3"), &[name]);
    }
}

// R15.6: `scrub_report` keeps `file=<name>` for a name of `SOURCE_FILES`.
#[test]
fn r15_6_scrub_report_keeps_known_file_tokens() {
    assert!(SOURCE_FILES.contains(&"gatekeeper.rs"));
    for text in [
        "ERROR panic file=gatekeeper.rs line=10 thread=gatekeeper",
        "ERROR panic file=lib.rs line=1 thread=main",
        "ERROR panic file=other line=1 thread=other",
    ] {
        assert_eq!(scrub_report(text), text);
    }
}

// R15.6: every other name in a `file=` token is scrubbed.
#[test]
fn r15_6_scrub_report_removes_unknown_file_tokens() {
    for text in [
        "ERROR panic file=zq-private-name.rs line=10",
        "ERROR panic file=forum.i2p line=10",
        "ERROR panic file=zqalice.example.com line=10",
        "ERROR panic file=Gatekeeper.RS line=10",
    ] {
        let out = scrub_report(text);
        assert!(!out.contains("zq-private-name"), "{out}");
        assert!(!out.contains("forum"), "{out}");
        assert!(!out.contains("zqalice"), "{out}");
        assert!(!out.contains("Gatekeeper.RS"), "{out}");
    }
}

// R15.6: only the `file=` token is spared. The same name elsewhere is free text.
#[test]
fn r15_6_a_bare_known_name_is_still_removed() {
    let out = scrub_report("see gatekeeper.rs and file=gatekeeper.rs");
    assert!(out.contains("file=gatekeeper.rs"), "{out}");
    assert!(!out.contains("see gatekeeper.rs"), "{out}");
    assert!(out.contains(REMOVED), "{out}");
}

// R15.6: `scrub_report` is `scrub` for everything else.
#[test]
fn r15_6_scrub_report_removes_what_scrub_removes() {
    for (case, secret) in [
        "https://example.com/a",
        "forum.i2p",
        "192.168.1.20",
        "bob@example.org",
        "/Users/zqalice/x",
        "0123456789abcdef0123456789abcdef",
    ]
    .into_iter()
    .enumerate()
    {
        let out = scrub_report(&format!("ERROR x file=lib.rs {secret} end"));
        assert!(
            !out.contains(secret),
            "injected value {case} ({} bytes) survives the scrub",
            secret.len()
        );
        assert!(
            out.contains("file=lib.rs"),
            "case {case}: the source name is gone ({} bytes out)",
            out.len()
        );
    }
}

fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

fn words() -> impl Strategy<Value = String> {
    "[A-Z ,;]{0,40}"
}

/// `plain` with every `.` and `/` written as `%2E` and `%2F`; `cases[i]` picks the case of the i-th char.
fn encode(plain: &str, cases: &[bool]) -> String {
    let mut out = String::new();
    for (i, c) in plain.chars().enumerate() {
        let upper = cases.get(i).copied().unwrap_or(false);
        match (c, upper) {
            ('.', true) => out.push_str("%2E"),
            ('.', false) => out.push_str("%2e"),
            ('/', true) => out.push_str("%2F"),
            ('/', false) => out.push_str("%2f"),
            _ => out.push(c),
        }
    }
    out
}

/// The file endings R15.5 keeps in free text.
const KEPT_ENDINGS: [&str; 9] = [
    "txt", "log", "json", "html", "js", "css", "toml", "yml", "plist",
];

/// A secret, and the same secret with its dots and slashes encoded.
fn encoded_secret() -> impl Strategy<Value = (String, String)> {
    let secret = prop_oneof![
        "zq[a-z]{3,10}\\.i2p(/[a-z]{1,8})?",
        ("zq[a-z]{3,10}", "[a-z]{2,6}")
            .prop_filter(
                "R15.5: these endings are file endings and stay",
                |(_, end)| { !KEPT_ENDINGS.contains(&end.as_str()) }
            )
            .prop_map(|(name, end)| format!("{name}.{end}")),
        (0u8..=255, 0u8..=255, 0u8..=255, 0u8..=255)
            .prop_map(|(a, b, c, d)| format!("{a}.{b}.{c}.{d}")),
        "/Users/zq[a-z]{3,10}",
    ];
    (secret, proptest::collection::vec(any::<bool>(), 0..64))
        .prop_map(|(plain, cases)| (encode(&plain, &cases), plain))
        .prop_map(|(hidden, plain)| (plain, hidden))
}

/// A hex run of 32 to 80 characters with at least one decimal digit and one letter.
fn hex_run() -> impl Strategy<Value = String> {
    ("[0-9]", "[a-fA-F]", "[0-9a-fA-F]{30,78}").prop_map(|(d, l, rest)| format!("{d}{l}{rest}"))
}

/// A hex run of one kind only: digits, or letters of one case (51 characters at most, below the base32 limit).
fn single_kind_hex_run() -> impl Strategy<Value = String> {
    prop_oneof!["[0-9]{32,80}", "[a-f]{32,51}", "[A-F]{32,51}"]
}

fn idn_host() -> impl Strategy<Value = String> {
    prop_oneof![
        "zq[a-z]{2,6}[éüöñ][a-z]{1,4}\\.[a-z]{2,6}",
        "zq[a-z]{3,8}\\.[éüö][a-z]{1,3}",
        "[éüö]zq[a-z]{3,8}\\.[a-z]{2,6}(/[a-z]{1,6})?",
    ]
}

fn spaced_home() -> impl Strategy<Value = String> {
    (
        prop_oneof![Just("/"), Just("\\")],
        prop_oneof![Just("Users"), Just("home")],
        prop_oneof![Just("/"), Just("\\")],
        "zq[a-z]{3,6}( zq[a-z]{3,6}){0,2}",
    )
        .prop_map(|(s1, dir, s2, name)| format!("{s1}{dir}{s2}{name}"))
}

fn unc_path() -> impl Strategy<Value = String> {
    ("zqsrv[a-z]{2,5}", "zqshare[a-z]{2,5}", "zqname[a-z]{2,5}")
        .prop_map(|(a, b, c)| format!("\\\\{a}\\{b}\\{c}\\docs"))
}

proptest! {
    #![proptest_config(config())]

    // R15.1: a secret written with `%2E` and `%2F` does not survive, encoded or decoded.
    #[test]
    fn r15_1_encoded_secrets_never_survive(b in words(), (clear, hidden) in encoded_secret(), a in words()) {
        let out = plain(&format!("{b} {hidden} {a}"));
        prop_assert!(!out.contains(&hidden), "{}", out);
        prop_assert!(!out.contains(&clear), "{}", out);
        prop_assert!(!out.contains("zq"), "{}", out);
    }

    // R15.2: an internationalised host name does not survive.
    #[test]
    fn r15_2_idn_host_never_survives(b in words(), v in idn_host(), a in words()) {
        let out = plain(&format!("{b} {v} {a}"));
        prop_assert!(!out.contains("zq"), "{}", out);
    }

    // R15.3: a run of 32 or more hex digits does not survive.
    #[test]
    fn r15_3_hex_run_never_survives(b in words(), v in hex_run(), a in words()) {
        let out = plain(&format!("{b} {v} {a}"));
        prop_assert!(!out.contains(&v), "{}", out);
    }

    // R15.3: a hex run with only digits, or only letters, stays.
    #[test]
    fn r15_3_single_kind_hex_run_stays(b in words(), v in single_kind_hex_run(), a in words()) {
        let text = format!("{b} {v} {a}");
        prop_assert_eq!(plain(&text), text);
    }

    // R15.4: a home folder with a name of several words does not survive, a name word included.
    #[test]
    fn r15_4_home_with_spaces_never_survives(v in spaced_home(), a in words()) {
        let out = plain(&format!("file {v}\n{a}"));
        prop_assert!(!out.contains("zq"), "{}", out);
    }

    // R15.4: a UNC path does not survive, server, share and name included.
    #[test]
    fn r15_4_unc_path_never_survives(b in words(), v in unc_path(), a in words()) {
        let out = plain(&format!("{b} {v} {a}"));
        prop_assert!(!out.contains("zq"), "{}", out);
    }

    // R15.6: a `file=<name>` token never keeps a name that is not in `SOURCE_FILES`.
    #[test]
    fn r15_6_unknown_file_names_never_survive(b in words(), name in "zq[a-z]{3,10}\\.(rs|md|ts|com|org|net|io|de)", a in words()) {
        prop_assume!(!SOURCE_FILES.contains(&name.as_str()));
        let out = scrub_report(&format!("{b} file={name} {a}"));
        prop_assert!(!out.contains("zq"), "{}", out);
    }

    // R15.6: with no `file=` token, `scrub_report` is `scrub`.
    #[test]
    fn r15_6_scrub_report_equals_scrub_without_a_file_token(text in "\\PC{0,200}") {
        prop_assume!(!text.contains("file="));
        prop_assert_eq!(scrub_report(&text), scrub(&text));
    }
}
