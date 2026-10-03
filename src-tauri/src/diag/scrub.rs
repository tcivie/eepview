// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The second privacy layer: it removes anything that looks private from the final report
//! text. The first layer is the typed log, which holds no text at all.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;

use super::types::SOURCE_FILES;

/// What a removed span becomes.
pub const REMOVED: &str = "[removed]";

/// File name endings that are not host names.
const FILE_ENDINGS: [&str; 9] = [
    "txt", "log", "json", "html", "js", "css", "toml", "yml", "plist",
];

/// Characters that end a URL or a path.
fn ends_span(b: u8) -> bool {
    b.is_ascii_whitespace() || b"\"'<>".contains(&b)
}

/// A byte span `[start, end)`.
type Span = (usize, usize);

/// [`scrub_with`] with the current user name and machine host name.
#[must_use]
pub fn scrub(text: &str) -> String {
    let (user, host) = identity();
    scrub_with(text, user.as_deref(), host.as_deref())
}

/// The user name and host name of this machine, read once.
fn identity() -> &'static (Option<String>, Option<String>) {
    static IDENTITY: OnceLock<(Option<String>, Option<String>)> = OnceLock::new();
    IDENTITY.get_or_init(|| (user_name(), host_name()))
}

fn user_name() -> Option<String> {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .filter(|s| !s.trim().is_empty())
}

fn host_name() -> Option<String> {
    let env = ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|k| std::env::var(k).ok());
    let from_file = || std::fs::read_to_string("/etc/hostname").ok();
    env.or_else(from_file)
        .or_else(host_command)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// `hostname`, the command every system has.
fn host_command() -> Option<String> {
    let mut command = std::process::Command::new("hostname");
    no_console(&mut command);
    let out = command.output().ok()?;
    String::from_utf8(out.stdout).ok()
}

/// Windows: no console window flashes up for a helper command.
#[cfg(windows)]
pub(crate) fn no_console(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub(crate) fn no_console(_command: &mut std::process::Command) {}

/// Replaces every private-looking span of `text` with [`REMOVED`].
#[must_use]
pub fn scrub_with(text: &str, user: Option<&str>, host: Option<&str>) -> String {
    let decoded = decode_escapes(text);
    let text = decoded.as_str();
    let bytes = text.as_bytes();
    let mut spans = urls(bytes);
    spans.extend(dotted_names(bytes));
    spans.extend(base32_runs(bytes));
    spans.extend(base64_runs(bytes));
    spans.extend(ipv6(bytes));
    spans.extend(emails(bytes));
    spans.extend(hex_runs(bytes));
    spans.extend(idn_hosts(text));
    spans.extend(home_paths(bytes));
    spans.extend(unc_paths(bytes));
    for word in identity_words(user, host) {
        spans.extend(words(bytes, word.as_bytes()));
    }
    replace(text, spans)
}

/// [`scrub`], except that a `file=<name>` token with a name from [`SOURCE_FILES`] stays: it
/// is a typed panic location from a closed list, not free text (R15.6).
#[must_use]
pub fn scrub_report(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, token, after)) = next_source_token(rest) {
        out.push_str(&scrub(before));
        out.push_str(token);
        rest = after;
    }
    out.push_str(&scrub(rest));
    out
}

/// The next `file=<name>` token with a known name, set apart by white space: the text
/// before it, the token and the text after it.
fn next_source_token(text: &str) -> Option<(&str, &str, &str)> {
    let mut from = 0;
    while let Some(at) = text[from..].find("file=").map(|i| from + i) {
        let end = text[at..]
            .find(char::is_whitespace)
            .map_or(text.len(), |i| at + i);
        let name = &text[at + 5..end];
        let apart = at == 0 || text[..at].ends_with(char::is_whitespace);
        if apart && SOURCE_FILES.binary_search(&name).is_ok() {
            return Some((&text[..at], &text[at..end], &text[end..]));
        }
        from = at + 5;
    }
    None
}

/// `%2E` and `%2F` (any case) as `.` and `/` (R15.1).
fn decode_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let code = rest.get(at + 1..at + 3).map(str::to_ascii_uppercase);
        match code.as_deref() {
            Some("2E") => out.push('.'),
            Some("2F") => out.push('/'),
            _ => {
                out.push('%');
                rest = &rest[at + 1..];
                continue;
            }
        }
        rest = &rest[at + 3..];
    }
    out.push_str(rest);
    out
}

/// The words of the user and host names to remove: 3 characters or more.
fn identity_words(user: Option<&str>, host: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = user.into_iter().map(str::to_owned).collect();
    if let Some(host) = host {
        out.push(host.to_owned());
        out.extend(host.split('.').next().map(str::to_owned));
    }
    out.into_iter()
        .map(|w| w.trim().to_owned())
        .filter(|w| w.chars().count() >= 3)
        .collect()
}

/// `text` with the merged spans replaced.
fn replace(text: &str, mut spans: Vec<Span>) -> String {
    spans.sort_unstable();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (start, end) in spans {
        if end <= at {
            continue;
        }
        if start > at {
            out.push_str(&text[at..start]);
            out.push_str(REMOVED);
        } else if at == 0 && start == 0 {
            out.push_str(REMOVED);
        }
        at = end;
    }
    out.push_str(&text[at..]);
    out
}

/// The end of the run of bytes from `from` that pass `keep`.
fn run_end(bytes: &[u8], from: usize, keep: impl Fn(u8) -> bool) -> usize {
    bytes[from..]
        .iter()
        .position(|b| !keep(*b))
        .map_or(bytes.len(), |n| from + n)
}

/// The start of the run of bytes before `to` that pass `keep`.
fn run_start(bytes: &[u8], to: usize, keep: impl Fn(u8) -> bool) -> usize {
    bytes[..to]
        .iter()
        .rposition(|b| !keep(*b))
        .map_or(0, |n| n + 1)
}

/// Maximal runs of bytes that pass `keep`.
fn runs(bytes: &[u8], keep: impl Fn(u8) -> bool + Copy) -> Vec<Span> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if keep(bytes[i]) {
            let end = run_end(bytes, i, keep);
            out.push((i, end));
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// `scheme://…` up to white space or a quote.
fn urls(bytes: &[u8]) -> Vec<Span> {
    let scheme = |b: u8| b.is_ascii_alphanumeric() || b"+.-".contains(&b);
    let mut out = Vec::new();
    for at in find_all(bytes, b"://") {
        let start = run_start(bytes, at, scheme);
        let end = run_end(bytes, at, |b| !ends_span(b));
        out.push((start, end));
    }
    out
}

/// Every start of `needle` in `bytes`, ASCII case ignored.
fn find_all(bytes: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || bytes.len() < needle.len() {
        return Vec::new();
    }
    (0..=bytes.len() - needle.len())
        .filter(|&i| bytes[i..i + needle.len()].eq_ignore_ascii_case(needle))
        .collect()
}

fn is_label_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b".-_".contains(&b)
}

/// `.i2p` names, other host names and IPv4 addresses, with a port or path after them.
fn dotted_names(bytes: &[u8]) -> Vec<Span> {
    let mut out = Vec::new();
    for (start, end) in runs(bytes, is_label_byte) {
        let (start, end) = trim(bytes, start, end);
        let Ok(word) = std::str::from_utf8(&bytes[start..end]) else {
            continue;
        };
        if is_i2p(word) || is_host(word) || is_ipv4(word) {
            out.push((start, run_end(bytes, end, |b| !ends_span(b))));
        }
    }
    out
}

/// The span without leading and trailing dots, dashes and underscores.
fn trim(bytes: &[u8], mut start: usize, mut end: usize) -> Span {
    let edge = |b: u8| b".-_".contains(&b);
    while start < end && edge(bytes[start]) {
        start += 1;
    }
    while end > start && edge(bytes[end - 1]) {
        end -= 1;
    }
    (start, end)
}

fn is_i2p(word: &str) -> bool {
    let bytes = word.as_bytes();
    bytes.len() > 4 && bytes[bytes.len() - 4..].eq_ignore_ascii_case(b".i2p")
}

/// Two or more labels, the last one 2 to 24 letters, and not a source or data file name.
fn is_host(word: &str) -> bool {
    let labels: Vec<&str> = word.split('.').collect();
    let last = labels.last().copied().unwrap_or_default();
    let tld = (2..=24).contains(&last.len()) && last.bytes().all(|b| b.is_ascii_alphabetic());
    let filled = labels.iter().all(|l| !l.is_empty());
    let file = FILE_ENDINGS.contains(&last.to_ascii_lowercase().as_str());
    labels.len() >= 2 && tld && filled && !file
}

fn is_ipv4(word: &str) -> bool {
    word.split('.').count() == 4 && word.parse::<Ipv4Addr>().is_ok()
}

/// Runs of 52 or more base32 characters.
fn base32_runs(bytes: &[u8]) -> Vec<Span> {
    let b32 = |b: u8| b.is_ascii_alphabetic() || b"234567".contains(&b);
    runs(bytes, b32)
        .into_iter()
        .filter(|(s, e)| e - s >= 52)
        .collect()
}

/// Runs of 44 or more base64 characters: I2P destinations and router hashes.
fn base64_runs(bytes: &[u8]) -> Vec<Span> {
    let b64 = |b: u8| b.is_ascii_alphanumeric() || b"+/=-~".contains(&b);
    runs(bytes, b64)
        .into_iter()
        .filter(|(s, e)| e - s >= 44 && mixed_case(&bytes[*s..*e]))
        .collect()
}

/// True when `run` holds an upper-case and a lower-case letter, as random base64 does.
fn mixed_case(run: &[u8]) -> bool {
    run.iter().any(u8::is_ascii_uppercase) && run.iter().any(u8::is_ascii_lowercase)
}

/// IPv6 addresses, with a zone.
fn ipv6(bytes: &[u8]) -> Vec<Span> {
    let class = |b: u8| b.is_ascii_alphanumeric() || b":.%".contains(&b);
    runs(bytes, class)
        .into_iter()
        .filter(|(s, e)| is_ipv6(&bytes[*s..*e]))
        .collect()
}

fn is_ipv6(word: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(word) else {
        return false;
    };
    let colons = text.matches(':').count();
    let address = text.split('%').next().unwrap_or_default();
    colons >= 2 && address.parse::<Ipv6Addr>().is_ok()
}

/// `name@domain`.
fn emails(bytes: &[u8]) -> Vec<Span> {
    let local = |b: u8| !ends_span(b) && !b"@(),;".contains(&b);
    let domain = |b: u8| b.is_ascii_alphanumeric() || b".-[]".contains(&b);
    let mut out = Vec::new();
    for (at, _) in bytes.iter().enumerate().filter(|(_, b)| **b == b'@') {
        let start = run_start(bytes, at, local);
        let end = run_end(bytes, at + 1, domain);
        if start < at && end > at + 1 {
            out.push((start, end));
        }
    }
    out
}

/// True for `/` and `\`.
fn is_sep(b: u8) -> bool {
    b == b'/' || b == b'\\'
}

/// A home folder name ends at a separator, a line end or a quote; spaces belong to it.
fn is_name_byte(b: u8) -> bool {
    !is_sep(b) && !b"\r\n\"'<>".contains(&b)
}

/// `<sep>Users<sep><name>`, `<sep>home<sep><name>` and `<sep>Documents and Settings<sep>
/// <name>`, with either separator, and the drive letter before them (R15.4).
fn home_paths(bytes: &[u8]) -> Vec<Span> {
    let mut out = Vec::new();
    for folder in [&b"Users"[..], b"home", b"Documents and Settings"] {
        for at in find_all(bytes, folder) {
            out.extend(home_span(bytes, at, folder.len()));
        }
    }
    out
}

/// The span of a home folder word at `at`, when separators surround it.
fn home_span(bytes: &[u8], at: usize, len: usize) -> Option<Span> {
    let before = at.checked_sub(1).filter(|&i| is_sep(bytes[i]))?;
    let after = at + len;
    if !bytes.get(after).is_some_and(|b| is_sep(*b)) {
        return None;
    }
    let drive = before >= 2 && bytes[before - 1] == b':' && bytes[before - 2].is_ascii_alphabetic();
    let start = if drive { before - 2 } else { before };
    Some((start, run_end(bytes, after + 1, is_name_byte)))
}

/// UNC paths `\\<server>\<share>\<name>` (R15.4).
fn unc_paths(bytes: &[u8]) -> Vec<Span> {
    find_all(bytes, b"\\\\")
        .into_iter()
        .filter(|&at| at == 0 || bytes[at - 1] != b'\\')
        .filter_map(|at| {
            let server = run_end(bytes, at + 2, is_name_byte);
            let share = run_end(bytes, (server + 1).min(bytes.len()), is_name_byte);
            let name = run_end(bytes, (share + 1).min(bytes.len()), is_name_byte);
            (server > at + 2 && share > server + 1).then_some((at, name))
        })
        .collect()
}

/// Runs of 32 or more hex digits (R15.3).
fn hex_runs(bytes: &[u8]) -> Vec<Span> {
    runs(bytes, |b| b.is_ascii_hexdigit())
        .into_iter()
        .filter(|(s, e)| e - s >= 32 && mixed_hex(&bytes[*s..*e]))
        .collect()
}

/// True when a hex run holds a decimal digit and a letter, as a hash does.
fn mixed_hex(run: &[u8]) -> bool {
    run.iter().any(u8::is_ascii_digit) && run.iter().any(u8::is_ascii_alphabetic)
}

/// Words with a dot and a non-ASCII letter: internationalised host names (R15.2).
fn idn_hosts(text: &str) -> Vec<Span> {
    let word = |c: char| c.is_alphanumeric() || c == '.' || c == '-';
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
        match (word(c), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.extend(idn_span(text, s, i));
                start = None;
            }
            _ => {}
        }
    }
    out
}

fn idn_span(text: &str, start: usize, end: usize) -> Option<Span> {
    let w = &text[start..end];
    let idn = w.contains('.') && w.chars().any(|c| !c.is_ascii() && c.is_alphabetic());
    idn.then(|| (start, run_end(text.as_bytes(), end, |b| !ends_span(b))))
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// Whole-word matches of `word`, ASCII case ignored.
fn words(bytes: &[u8], word: &[u8]) -> Vec<Span> {
    find_all(bytes, word)
        .into_iter()
        .map(|at| (at, at + word.len()))
        .filter(|(s, e)| {
            let before = *s == 0 || !is_word_byte(bytes[s - 1]);
            let after = *e == bytes.len() || !is_word_byte(bytes[*e]);
            before && after
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(text: &str) -> String {
        scrub_with(text, Some("gleb"), Some("box-7.local"))
    }

    #[test]
    fn spans_at_the_start_and_overlaps_merge() {
        assert_eq!(clean("http://a.i2p/x"), "[removed]");
        assert_eq!(clean("a@b.example.com x"), "[removed] x");
        assert_eq!(clean("GLEB on box-7"), "[removed] on [removed]");
        assert_eq!(clean("glebx"), "glebx");
        assert_eq!(scrub_with("ab", Some("ab"), None), "ab");
    }

    #[test]
    fn destinations_and_windows_paths_go() {
        let hash = "AbCd0123456789-~AbCd0123456789-~AbCd01234567";
        assert_eq!(clean(&format!("x {hash} y")), "x [removed] y");
        assert_eq!(clean("at C:/Users/al/x"), "at [removed]/x");
        assert_eq!(clean("at \\\\?\\C:\\Users\\al\\x"), "at [removed]\\x");
    }

    #[test]
    fn review_rules_remove_more() {
        assert_eq!(clean("see stats%2Ei2p%2Fx z"), "see [removed] z");
        assert_eq!(
            clean("h 0123456789abcdef0123456789abcdef z"),
            "h [removed] z"
        );
        assert_eq!(clean("at bücher.example/x y"), "at [removed] y");
        assert_eq!(
            clean("C:\\Documents and Settings\\Jo Ann\\a"),
            "[removed]\\a"
        );
        assert_eq!(clean("/home/Jo Ann/a"), "[removed]/a");
        assert_eq!(clean("\\\\srv\\share\\bob\\a"), "[removed]\\a");
        assert_eq!(clean("crate main.rs"), "crate [removed]");
        assert_eq!(clean("100%25 and %zz"), "100%25 and %zz");
    }

    #[test]
    fn report_keeps_known_source_names() {
        let name = SOURCE_FILES[0];
        let text = format!("file={name} line=3 file=evil.rs at http://a.i2p/");
        let out = scrub_report(&text);
        assert!(out.starts_with(&format!("file={name} line=3 ")), "{out}");
        assert!(!out.contains("evil.rs") && !out.contains("a.i2p"), "{out}");
    }

    #[test]
    fn identity_names_come_from_the_system() {
        let _ = scrub("text");
        assert!(identity_words(None, None).is_empty());
    }
}
