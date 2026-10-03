// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The second privacy layer: it removes anything that looks private from the final report
//! text. The first layer is the typed log, which holds no text at all.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;

/// What a removed span becomes.
pub const REMOVED: &str = "[removed]";

/// File name endings that are not host names.
const FILE_ENDINGS: [&str; 12] = [
    "rs", "txt", "log", "json", "html", "ts", "js", "css", "md", "toml", "yml", "plist",
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
    let bytes = text.as_bytes();
    let mut spans = urls(bytes);
    spans.extend(dotted_names(bytes));
    spans.extend(base32_runs(bytes));
    spans.extend(base64_runs(bytes));
    spans.extend(ipv6(bytes));
    spans.extend(emails(bytes));
    spans.extend(home_paths(bytes));
    for word in identity_words(user, host) {
        spans.extend(words(bytes, word.as_bytes()));
    }
    replace(text, spans)
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

/// `/Users/<name>`, `/home/<name>` and `<drive>:\Users\<name>`.
fn home_paths(bytes: &[u8]) -> Vec<Span> {
    let mut out = Vec::new();
    for prefix in [&b"/Users/"[..], b"/home/", b":\\Users\\"] {
        out.extend(
            find_all(bytes, prefix)
                .into_iter()
                .map(|at| home_span(bytes, at, prefix)),
        );
    }
    out
}

/// The span of a home folder prefix at `at` and the user name after it. The drive letter
/// before `:\Users\` belongs to it.
fn home_span(bytes: &[u8], at: usize, prefix: &[u8]) -> Span {
    let name = |b: u8| !ends_span(b) && !b"/\\".contains(&b);
    let drive = at >= 2 && bytes[at - 1] == b':' && bytes[at - 2].is_ascii_alphabetic();
    let start = if prefix[0] == b':' {
        at.saturating_sub(1)
    } else if drive {
        at - 2
    } else {
        at
    };
    (start, run_end(bytes, at + prefix.len(), name))
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
        assert_eq!(
            clean("at \\\\?\\C:\\Users\\al\\x"),
            "at \\\\?\\[removed]\\x"
        );
    }

    #[test]
    fn identity_names_come_from_the_system() {
        let _ = scrub("text");
        assert!(identity_words(None, None).is_empty());
    }
}
