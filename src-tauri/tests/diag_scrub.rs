// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests R4.1 to R4.4 of `docs/wiki/diagnostics-and-bug-reports.md`:
//! the scrubber. Public API only. Examples state the rule; the properties inject random
//! private values into random text and check that none survives.

use eepview_lib::diag::{self, REMOVED, scrub_with};
use proptest::prelude::*;

fn plain(text: &str) -> String {
    scrub_with(text, None, None)
}

fn assert_removed(text: &str, secret: &str) {
    let out = plain(text);
    assert!(!out.contains(secret), "`{secret}` survives in `{out}`");
    assert!(out.contains(REMOVED), "no marker in `{out}`");
}

// R4.2: the marker text.
#[test]
fn r4_2_marker_is_removed_in_brackets() {
    assert_eq!(REMOVED, "[removed]");
}

// R4.2: text with nothing private is left as it is.
#[test]
fn r4_2_other_text_is_unchanged() {
    let text = "The router answered, then the tab stopped loading (twice).";
    assert_eq!(plain(text), text);
    assert_eq!(plain(""), "");
}

// R4.2: a URL goes, from the scheme to white space or one of `"'<>`.
#[test]
fn r4_2_url_is_removed_up_to_a_delimiter() {
    assert_removed("see https://example.com/a?b=1 now", "example.com");
    let out = plain("see https://example.com/a?b=1 now");
    assert!(out.starts_with("see ") && out.ends_with(" now"), "{out}");
    assert_removed("open <http://forum.i2p/x> please", "forum.i2p");
    assert_removed(
        "a \"ftp://files.example.net/readme\" b",
        "files.example.net",
    );
    assert!(!plain("go to http://a.example/x then").contains("://"));
}

// R4.2: a name that ends in `.i2p`, in any case, with its port, path and query.
#[test]
fn r4_2_i2p_name_is_removed_with_port_path_query() {
    assert_removed("visit forum.i2p now", "forum");
    assert_removed("visit FORUM.I2P now", "FORUM");
    assert_removed("visit forum.i2p:8080/path/x?y=1 now", "forum.i2p");
    let out = plain("visit forum.i2p:8080/path/x?y=1 now");
    assert!(!out.contains("8080") && !out.contains("path"), "{out}");
    assert_removed(
        "visit abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrst.b32.i2p now",
        "b32.i2p",
    );
}

// R4.2: a host name of two or more labels with a last label of 2 to 24 letters.
#[test]
fn r4_2_host_name_is_removed() {
    assert_removed("peer www.example.com failed", "example");
    assert_removed("peer mail.corp.example.org failed", "corp");
    assert_removed("peer a.zz failed", "a.zz");
}

// R4.2 + R15.5: a file name with a known ending is not a host name. `rs`, `md` and `ts` are no endings.
#[test]
fn r4_2_known_file_names_stay() {
    for name in [
        "eepview.log",
        "notes.txt",
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

// R4.2: a run of 52 or more base32 characters goes, in any case.
#[test]
fn r4_2_base32_run_of_52_goes() {
    let run = "abcdefghijklmnopqrstuvwxyz234567abcdefghijklmnopqrst";
    assert_eq!(run.len(), 52);
    assert_removed(&format!("hash {run} end"), run);
    assert_removed(
        &format!("hash {} end", run.to_uppercase()),
        &run.to_uppercase(),
    );
    let longer = format!("{run}abcdef");
    assert_removed(&format!("hash {longer} end"), &longer);
}

// R4.2: a run shorter than 52 stays. 51 times `a` is a hex run too, but with no digit it stays (R15.3).
#[test]
fn r4_2_base32_run_of_51_stays() {
    let run = "a".repeat(51);
    let text = format!("word {run} end");
    assert_eq!(plain(&text), text);
}

// R15.5: `rs`, `md` and `ts` are country domains, so in free text these names are removed.
#[test]
fn r15_5_rs_md_ts_names_are_host_names_in_free_text() {
    for name in [
        "apply.rs",
        "main.rs",
        "gatekeeper.rs",
        "README.md",
        "app.ts",
    ] {
        let out = plain(&format!("at {name} line 3"));
        assert!(!out.contains(name), "`{name}` survives in `{out}`");
        assert!(out.contains(REMOVED), "{out}");
    }
}

// R4.2: an IPv4 address goes.
#[test]
fn r4_2_ipv4_address_is_removed() {
    assert_removed("peer 192.168.1.20 failed", "192.168.1.20");
    assert_removed("peer 10.0.0.1:4444 failed", "10.0.0.1");
    assert_removed("peer 255.255.255.255 failed", "255.255.255.255");
    assert_removed("peer 0.0.0.0 failed", "0.0.0.0");
}

// R4.2: four numbers above 255 are not an address.
#[test]
fn r4_2_out_of_range_quad_is_not_an_address() {
    let text = "build 256.1.1.1 ready";
    assert_eq!(plain(text), text);
}

// R4.2: an IPv6 address goes, also in brackets and with a zone.
#[test]
fn r4_2_ipv6_address_is_removed() {
    assert_removed("peer 2001:db8::1 failed", "2001:db8");
    assert_removed(
        "peer 2001:0db8:85a3:0000:0000:8a2e:0370:7334 failed",
        "8a2e",
    );
    assert_removed("peer [::1]:8080 failed", "::1");
    assert_removed("peer fe80::1%en0 failed", "fe80");
    assert_removed("peer [fe80::1%25en0]:80 failed", "fe80");
}

// R4.2: an email address goes.
#[test]
fn r4_2_email_address_is_removed() {
    assert_removed("mail bob@example.org please", "bob@");
    assert_removed(
        "mail first.last+tag@mail.example.co.uk please",
        "first.last",
    );
}

// R4.2: a home folder path goes, the name included.
#[test]
fn r4_2_home_paths_are_removed_with_the_name() {
    assert_removed("file /Users/zqalice/Library/Logs/x", "zqalice");
    assert_removed("file /home/zqbob/.config/eepview", "zqbob");
    assert_removed(
        "file C:\\Users\\zqcarol\\AppData\\Local\\eepview",
        "zqcarol",
    );
    assert_removed("file d:\\Users\\zqdave\\Downloads", "zqdave");
}

// R4.2: the user name goes, any case, whole words only.
#[test]
fn r4_2_user_name_goes_as_a_whole_word() {
    let out = scrub_with("hello zorbax is here", Some("zorbax"), None);
    assert!(!out.contains("zorbax"), "{out}");
    let upper = scrub_with("hello ZORBAX is here", Some("zorbax"), None);
    assert!(!upper.to_lowercase().contains("zorbax"), "{upper}");
    let inside = "hello zorbaxes and xzorbax are here";
    assert_eq!(scrub_with(inside, Some("zorbax"), None), inside);
}

// R4.2: a name of fewer than 3 characters is not scrubbed.
#[test]
fn r4_2_short_user_name_stays() {
    let text = "al is al";
    assert_eq!(scrub_with(text, Some("al"), None), text);
}

// R4.2: the host name goes, and for a dotted host name its first label goes too.
#[test]
fn r4_2_host_name_goes_with_its_first_label() {
    let out = scrub_with("on mybox.local and mybox now", None, Some("mybox.local"));
    assert!(!out.contains("mybox"), "{out}");
    let plain_host = scrub_with("on devbox now", None, Some("devbox"));
    assert!(!plain_host.contains("devbox"), "{plain_host}");
}

// R4.3: version numbers and times stay.
#[test]
fn r4_3_versions_and_times_stay() {
    for text in [
        "eepview 0.1.0",
        "macOS 16.0",
        "WebKit 621.1.15.10.7",
        "Windows 10.0.26100",
        "at 12:00:00 today",
        "2026-10-03T12:00:00Z INFO startup",
    ] {
        assert_eq!(plain(text), text);
    }
}

// R4.1: `scrub` also removes the user name of the current account.
#[test]
fn r4_1_scrub_removes_the_current_user_name() {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();
    if user.len() < 3 || !user.chars().all(|c| c.is_ascii_alphanumeric()) {
        return;
    }
    let text = format!("hello {user} world");
    assert!(!diag::scrub(&text).contains(&user));
}

// R4.1: `scrub` also removes everything `scrub_with` removes.
#[test]
fn r4_1_scrub_removes_urls_too() {
    let out = diag::scrub("see https://example.com/a now");
    assert!(!out.contains("example.com"), "{out}");
}

fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// Words of capital letters: they hold no digit, no dot and no lower-case letter, so they
/// cannot hold an injected value.
fn words() -> impl Strategy<Value = String> {
    "[A-Z ,;]{0,40}"
}

fn url() -> impl Strategy<Value = String> {
    "https?://zq[a-z]{4,10}\\.[a-z]{2,6}(/[a-z0-9]{1,8}){0,2}(\\?[a-z]{1,4}=[0-9]{1,3})?"
}

fn i2p() -> impl Strategy<Value = String> {
    prop_oneof![
        "zq[a-z]{3,10}\\.i2p(:[0-9]{2,4})?(/[a-z]{1,8})?",
        "ZQ[A-Z]{3,10}\\.I2P",
        "zq[a-z]{3,10}\\.i2p\\?[a-z]{1,4}=[0-9]{1,3}",
    ]
}

fn base32() -> impl Strategy<Value = String> {
    "[a-z2-7]{52,60}"
}

fn ipv4() -> impl Strategy<Value = String> {
    (0u8..=255, 0u8..=255, 0u8..=255, 0u8..=255).prop_map(|(a, b, c, d)| format!("{a}.{b}.{c}.{d}"))
}

fn ipv6() -> impl Strategy<Value = String> {
    prop_oneof![
        proptest::collection::vec("[0-9a-f]{1,4}", 8).prop_map(|g| g.join(":")),
        ("[0-9a-f]{1,4}", "[0-9a-f]{1,4}").prop_map(|(a, b)| format!("{a}::{b}")),
        "[0-9a-f]{1,4}".prop_map(|a| format!("[{a}::1]")),
        "[0-9a-f]{1,4}".prop_map(|a| format!("{a}::1%en0")),
    ]
}

fn email() -> impl Strategy<Value = String> {
    "zq[a-z]{3,8}@[a-z]{3,8}\\.[a-z]{2,4}"
}

fn home_path() -> impl Strategy<Value = String> {
    prop_oneof![
        "/Users/zq[a-z]{3,10}",
        "/home/zq[a-z]{3,10}",
        "/Users/zq[a-z]{3,10}/Library/Logs",
        "[C-F]:\\\\Users\\\\zq[a-z]{3,10}",
    ]
}

fn name() -> impl Strategy<Value = String> {
    "zq[a-z]{3,8}"
}

/// `before`, the value and `after`, set apart by spaces.
fn inject(before: &str, value: &str, after: &str) -> String {
    format!("{before} {value} {after}")
}

proptest! {
    #![proptest_config(config())]

    // R4.4: an injected URL does not survive, and no `://` is left.
    #[test]
    fn r4_4_injected_url_never_survives(b in words(), v in url(), a in words()) {
        let out = plain(&inject(&b, &v, &a));
        prop_assert!(!out.contains(&v), "{}", out);
        prop_assert!(!out.contains("://"), "{}", out);
    }

    // R4.4: an injected `.i2p` name does not survive, in any case.
    #[test]
    fn r4_4_injected_i2p_name_never_survives(b in words(), v in i2p(), a in words()) {
        let out = plain(&inject(&b, &v, &a));
        prop_assert!(!out.contains(&v), "{}", out);
        prop_assert!(!out.to_ascii_lowercase().contains(".i2p"), "{}", out);
    }

    // R4.2: an injected run of 52 or more base32 characters does not survive.
    #[test]
    fn r4_2_injected_base32_run_never_survives(b in words(), v in base32(), a in words()) {
        let out = plain(&inject(&b, &v, &a));
        prop_assert!(!out.contains(&v), "{}", out);
    }

    // R4.4: an injected IPv4 address does not survive.
    #[test]
    fn r4_4_injected_ipv4_never_survives(b in words(), v in ipv4(), a in words()) {
        let out = plain(&inject(&b, &v, &a));
        prop_assert!(!out.contains(&v), "{}", out);
    }

    // R4.4: an injected IPv6 address does not survive.
    #[test]
    fn r4_4_injected_ipv6_never_survives(b in words(), v in ipv6(), a in words()) {
        let out = plain(&inject(&b, &v, &a));
        prop_assert!(!out.contains(&v), "{}", out);
    }

    // R4.4: an injected email address does not survive.
    #[test]
    fn r4_4_injected_email_never_survives(b in words(), v in email(), a in words()) {
        let out = plain(&inject(&b, &v, &a));
        prop_assert!(!out.contains(&v), "{}", out);
        prop_assert!(!out.contains('@'), "{}", out);
    }

    // R4.4: an injected home folder path does not survive, and neither does the name in it.
    #[test]
    fn r4_4_injected_home_path_never_survives(b in words(), v in home_path(), a in words()) {
        let out = plain(&inject(&b, &v, &a));
        prop_assert!(!out.contains(&v), "{}", out);
        prop_assert!(!out.contains("zq"), "{}", out);
    }

    // R4.2: the user name never survives, in any case.
    #[test]
    fn r4_2_user_name_never_survives(b in words(), user in name(), a in words()) {
        let text = inject(&b, &user.to_uppercase(), &inject(&a, &user, ""));
        let out = scrub_with(&text, Some(&user), None);
        prop_assert!(!out.to_lowercase().contains(&user), "{}", out);
    }

    // R4.2: the host name and its first label never survive.
    #[test]
    fn r4_2_host_name_never_survives(b in words(), label in name(), a in words()) {
        let host = format!("{label}.local");
        let text = inject(&b, &host, &inject(&label, &a, ""));
        let out = scrub_with(&text, None, Some(&host));
        prop_assert!(!out.to_lowercase().contains(&label), "{}", out);
    }

    // R4.2: text with no private value passes through unchanged.
    #[test]
    fn r4_2_plain_words_are_unchanged(text in "[A-Za-z ,;]{0,40}") {
        prop_assert_eq!(plain(&text), text);
    }

    // R4: the scrubber accepts any text.
    #[test]
    fn r4_scrub_accepts_any_text(text in "\\PC{0,300}") {
        let _ = scrub_with(&text, Some("zorbax"), Some("mybox.local"));
    }
}
