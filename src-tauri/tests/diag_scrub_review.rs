// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the review additions to R4.2 and R4.4 of
//! `docs/wiki/diagnostics-and-bug-reports.md`: the base64 run rule and Windows home paths
//! written with forward slashes. Public API only.

use eepview_lib::diag::{REMOVED, scrub_with};
use proptest::prelude::*;

fn plain(text: &str) -> String {
    scrub_with(text, None, None)
}

fn assert_removed(text: &str, secret: &str) {
    let out = plain(text);
    assert!(!out.contains(secret), "`{secret}` survives in `{out}`");
    assert!(out.contains(REMOVED), "no marker in `{out}`");
}

/// A router hash: 44 base64 characters with both cases.
const HASH_44: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789+/AbCdEf";

// R4.2: a run of 44 or more base64 characters with both cases is removed (a router hash).
#[test]
fn r4_2_base64_run_of_44_with_both_cases_goes() {
    assert_eq!(HASH_44.len(), 44);
    assert_removed(&format!("router {HASH_44} down"), HASH_44);
}

// R4.2: an I2P destination (about 516 characters, with `-` and `~`) is removed.
#[test]
fn r4_2_base64_destination_goes() {
    let destination: String = "Ab9-~Zy8".repeat(65) + "AAAA";
    assert!(destination.len() >= 516);
    assert_removed(&format!("dest {destination} end"), "Ab9-~Zy8");
    let out = plain(&format!("dest {destination} end"));
    assert!(out.starts_with("dest ") && out.ends_with(" end"), "{out}");
}

// R4.2: the run may end with `=` padding.
#[test]
fn r4_2_base64_run_with_padding_goes() {
    let run = format!("{}=", &HASH_44[..43]);
    assert_removed(&format!("blob {run} end"), &run);
}

// R4.2: a run longer than 44 goes whole, not only its first 44 characters.
#[test]
fn r4_2_longer_base64_run_goes_whole() {
    let run = format!("{HASH_44}QrStUvWxYz");
    let out = plain(&format!("blob {run} end"));
    assert!(!out.contains("QrStUvWxYz"), "{out}");
}

// R4.2: a 43-character run stays: it is below the 44 limit.
#[test]
fn r4_2_base64_run_of_43_stays() {
    let run = &HASH_44[..43];
    let text = format!("blob {run} end");
    assert_eq!(plain(&text), text);
}

// R4.2: a run with one case only is not base64 by this rule: 51 `a` stay (a hex run needs a digit and a letter, R15.3).
#[test]
fn r4_2_run_of_51_a_stays() {
    let text = format!("word {} end", "a".repeat(51));
    assert_eq!(plain(&text), text);
}

// R4.2: 44 or more letters of one case stay (below the base32 run of 52).
#[test]
fn r4_2_single_case_runs_stay() {
    for run in [
        "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqr",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQR",
    ] {
        assert_eq!(run.len(), 44);
        let text = format!("word {run} end");
        assert_eq!(plain(&text), text, "{run}");
    }
}

// R4.2: words with spaces between them are not one run, whatever their case.
#[test]
fn r4_2_mixed_case_words_with_spaces_stay() {
    let text = "The Router Answered With An Unexpected Status Code And Then Stopped Loading";
    assert_eq!(plain(text), text);
}

// R4.2: a Windows home path with forward slashes goes, the drive letter and the name included.
#[test]
fn r4_2_windows_home_with_forward_slashes_goes() {
    for text in [
        "file C:/Users/zqalice/AppData/Local/eepview",
        "file d:/Users/zqalice/Downloads",
        "file C:/Users/zqalice",
    ] {
        let out = plain(text);
        assert!(!out.contains("zqalice"), "{out}");
        assert!(
            !out.contains("C:") && !out.contains("d:"),
            "the drive letter survives in {out}"
        );
        assert!(out.contains(REMOVED), "{out}");
    }
}

// R4.2: a home path after `\\?\` goes, with either kind of slash.
#[test]
fn r4_2_windows_extended_length_home_goes() {
    for text in [
        "file \\\\?\\C:\\Users\\zqbob\\AppData\\x",
        "file \\\\?\\C:/Users/zqbob/AppData/x",
    ] {
        let out = plain(text);
        assert!(!out.contains("zqbob"), "{out}");
        assert!(!out.contains("C:"), "{out}");
    }
}

// R4.2: the existing backslash form still goes, drive letter included.
#[test]
fn r4_2_windows_home_with_backslashes_still_goes() {
    let out = plain("file C:\\Users\\zqcarol\\AppData\\Local");
    assert!(!out.contains("zqcarol"), "{out}");
    assert!(!out.contains("C:"), "{out}");
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

/// A base64 run of 44 to 120 characters with an upper-case and a lower-case letter in it.
fn destination() -> impl Strategy<Value = String> {
    ("[A-Z]", "[a-z]", "[A-Za-z0-9+/=~-]{42,118}").prop_map(|(u, l, rest)| format!("{u}{l}{rest}"))
}

fn windows_home() -> impl Strategy<Value = String> {
    prop_oneof![
        "[C-F]:/Users/zq[a-z]{3,10}",
        "[C-F]:/Users/zq[a-z]{3,10}/AppData/Local",
        "\\\\\\\\\\?\\\\[C-F]:/Users/zq[a-z]{3,10}",
        "\\\\\\\\\\?\\\\[C-F]:\\\\Users\\\\zq[a-z]{3,10}",
    ]
}

proptest! {
    #![proptest_config(config())]

    // R4.4: an injected base64 destination does not survive.
    #[test]
    fn r4_4_injected_base64_destination_never_survives(b in words(), v in destination(), a in words()) {
        let out = plain(&format!("{b} {v} {a}"));
        prop_assert!(!out.contains(&v), "{}", out);
    }

    // R4.2: an injected Windows home path with forward slashes does not survive, name included.
    #[test]
    fn r4_2_injected_windows_home_never_survives(b in words(), v in windows_home(), a in words()) {
        let out = plain(&format!("{b} {v} {a}"));
        prop_assert!(!out.contains("zq"), "{}", out);
        prop_assert!(!out.contains(":/"), "{}", out);
    }
}
