// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Router check 3 (`docs/wiki/router-checks.md`, V12): does the HTTP proxy of the router
//! list an outproxy?
//!
//! eepview reads the router's own tunnel configuration files. It only reads them, and it
//! opens no socket here. A detail names a file by its name only, never by its folder.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::console::{i2pd_config_files, java_config_dirs, properties};

/// The router type: the type of its console (Java I2P or i2pd).
pub use super::console::ConsoleKind as RouterKind;

/// The i2pd HTTP proxy port when `i2pd.conf` names none.
const I2PD_PROXY_PORT: u16 = 4444;
/// The Java I2P tunnel keys that list outproxies.
const JAVA_OUTPROXY_KEYS: [&str; 2] = ["proxyList", "option.i2ptunnel.httpclient.SSLOutproxies"];

/// What the router configuration says about the HTTP proxy on one port (V12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutproxyFinding {
    /// The HTTP proxy is in `file` (file name only) and lists no outproxy.
    Clear {
        /// The file name.
        file: String,
    },
    /// The HTTP proxy is in `file` and lists these outproxies.
    Listed {
        /// The file name.
        file: String,
        /// The outproxies, in file order, no repeats.
        outproxies: Vec<String>,
    },
    /// eepview could not tell: the reason.
    Unknown(String),
}

/// The keys of each tunnel of one Java I2P tunnel file, by tunnel number. `None` holds the
/// keys with no `tunnel.<n>.` prefix (a file of `i2ptunnel.config.d/`).
fn java_tunnels(config: &str) -> BTreeMap<Option<u32>, Vec<(&str, &str)>> {
    let mut tunnels: BTreeMap<Option<u32>, Vec<(&str, &str)>> = BTreeMap::new();
    for (key, value) in properties(config) {
        let (number, field) = tunnel_key(key);
        tunnels.entry(number).or_default().push((field, value));
    }
    tunnels
}

/// `tunnel.<n>.<field>` gives `(Some(n), field)`; any other key is `(None, key)`.
fn tunnel_key(key: &str) -> (Option<u32>, &str) {
    let numbered = key
        .strip_prefix("tunnel.")
        .and_then(|rest| rest.split_once('.'))
        .filter(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|(n, field)| Some((Some(n.parse().ok()?), field)));
    numbered.unwrap_or((None, key))
}

/// The last value of `key`: a later line wins, as in a Java properties file.
fn value<'a>(fields: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .rev()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}

/// `text` split on any of `separators`, trimmed, with empty parts and repeats dropped.
fn split_list(text: &str, separators: &[char]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split(|c: char| separators.contains(&c) || c.is_whitespace()) {
        let part = part.trim();
        if !part.is_empty() && !out.iter().any(|seen| seen == part) {
            out.push(part.to_owned());
        }
    }
    out
}

fn is_java_http_proxy(fields: &[(&str, &str)], port: u16) -> bool {
    value(fields, "type") == Some("httpclient")
        && value(fields, "listenPort").and_then(|p| p.parse::<u16>().ok()) == Some(port)
}

/// The outproxies of the `httpclient` tunnel on `port` in one Java I2P tunnel file
/// (`i2ptunnel.config` or one file of `i2ptunnel.config.d/`). `None`: no such tunnel.
#[must_use]
pub fn java_outproxies(config: &str, port: u16) -> Option<Vec<String>> {
    let tunnels = java_tunnels(config);
    let fields = tunnels.values().find(|f| is_java_http_proxy(f, port))?;
    let listed = JAVA_OUTPROXY_KEYS
        .iter()
        .filter_map(|key| value(fields, key))
        .collect::<Vec<_>>()
        .join(",");
    Some(split_list(&listed, &[',', ';']))
}

/// The `key = value` lines of the `[<name>]` section of an `i2pd.conf` text, comments
/// removed. `None` when the text has no such section.
fn section<'a>(text: &'a str, name: &str) -> Option<Vec<(&'a str, &'a str)>> {
    let mut inside = false;
    let mut out: Option<Vec<(&str, &str)>> = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let header = line.strip_prefix('[').and_then(|l| l.strip_suffix(']'));
        inside = header.map_or(inside, |found| found.trim() == name);
        if !inside {
            continue;
        }
        let fields = out.get_or_insert_with(Vec::new);
        let pair = header.is_none().then(|| line.split_once('=')).flatten();
        fields.extend(pair.map(|(k, v)| (k.trim(), v.trim())));
    }
    out
}

/// The outproxies of the `[httpproxy]` section of one `i2pd.conf` when it serves `port`.
/// A text with no `[httpproxy]` section names no HTTP proxy.
#[must_use]
pub fn i2pd_outproxies(i2pd_conf: &str, port: u16) -> Option<Vec<String>> {
    let fields = section(i2pd_conf, "httpproxy")?;
    if value(&fields, "enabled").is_some_and(|v| v.eq_ignore_ascii_case("false")) {
        return None;
    }
    let serves = value(&fields, "port").map_or(Some(I2PD_PROXY_PORT), |p| p.parse().ok());
    (serves == Some(port)).then(|| split_list(value(&fields, "outproxy").unwrap_or(""), &[',']))
}

/// The tunnel files of a Java I2P configuration folder: `i2ptunnel.config.d/*` by name, then
/// `i2ptunnel.config`.
fn java_tunnel_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir.join("i2ptunnel.config.d"))
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    files.push(dir.join("i2ptunnel.config"));
    files
}

/// The name of `file`, without its folder.
fn file_name(file: &Path) -> String {
    file.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

type Found = Option<(String, Vec<String>)>;

/// The first file in `files` whose text `parse` finds an HTTP proxy in.
fn first_in(files: &[PathBuf], parse: impl Fn(&str) -> Option<Vec<String>>) -> Found {
    files.iter().find_map(|file| {
        let text = fs::read_to_string(file).ok()?;
        parse(&text).map(|list| (file_name(file), list))
    })
}

fn first_java(port: u16, env: &dyn Fn(&str) -> Option<String>) -> Found {
    let files: Vec<PathBuf> = java_config_dirs(env)
        .iter()
        .flat_map(|dir| java_tunnel_files(dir))
        .collect();
    first_in(&files, |text| java_outproxies(text, port))
}

fn first_i2pd(port: u16, env: &dyn Fn(&str) -> Option<String>) -> Found {
    first_in(&i2pd_config_files(env), |text| i2pd_outproxies(text, port))
}

fn finding(found: Found, port: u16) -> OutproxyFinding {
    match found {
        None => OutproxyFinding::Unknown(format!(
            "No router configuration with an HTTP proxy on port {port} was found."
        )),
        Some((file, outproxies)) if outproxies.is_empty() => OutproxyFinding::Clear { file },
        Some((file, outproxies)) => OutproxyFinding::Listed { file, outproxies },
    }
}

/// V12 over the files of this OS (the folders of `net::console::java_config_dirs` and the
/// files of `net::console::i2pd_config_files`, with the same `env`).
#[must_use]
pub fn find_outproxy(
    kind: Option<RouterKind>,
    port: u16,
    env: &dyn Fn(&str) -> Option<String>,
) -> OutproxyFinding {
    match kind {
        Some(RouterKind::Java) => finding(first_java(port, env), port),
        Some(RouterKind::I2pd) => finding(first_i2pd(port, env), port),
        None => match (first_java(port, env), first_i2pd(port, env)) {
            (Some(_), Some(_)) => OutproxyFinding::Unknown(format!(
                "Both a Java I2P and an i2pd configuration use port {port}, and the router \
                 type is not known."
            )),
            (java, i2pd) => finding(java.or(i2pd), port),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_keys_split_on_the_number() {
        assert_eq!(tunnel_key("tunnel.12.type"), (Some(12), "type"));
        assert_eq!(tunnel_key("type"), (None, "type"));
        assert_eq!(tunnel_key("tunnel.x.type"), (None, "tunnel.x.type"));
        assert_eq!(tunnel_key("tunnel..type"), (None, "tunnel..type"));
        assert_eq!(
            tunnel_key("tunnel.99999999999.type"),
            (None, "tunnel.99999999999.type")
        );
    }

    #[test]
    fn a_file_name_drops_the_folder() {
        assert_eq!(file_name(Path::new("/a/b/i2pd.conf")), "i2pd.conf");
        assert_eq!(file_name(Path::new("/")), "");
    }
}
