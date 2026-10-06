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
use std::time::SystemTime;

use super::console::{i2pd_config_files, java_config_dirs, properties, split_files};

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

/// The i2pd section of the HTTP proxy.
const SECTION: &str = "httpproxy";

/// The name of a `[<name>]` section header line.
fn header(line: &str) -> Option<&str> {
    line.strip_prefix('[')
        .and_then(|l| l.strip_suffix(']'))
        .map(str::trim)
}

/// The key and value of `line` when it is a key of the HTTP proxy: inside `[httpproxy]`, or
/// `httpproxy.<key>` before the first section.
fn proxy_key<'a>(section: Option<&str>, line: &'a str) -> Option<(&'a str, &'a str)> {
    let (key, value) = line.split_once('=')?;
    let (key, value) = (key.trim(), value.trim());
    match section {
        Some(SECTION) => Some((key, value)),
        Some(_) => None,
        None => key.strip_prefix("httpproxy.").map(|k| (k, value)),
    }
}

/// The keys of the HTTP proxy in an `i2pd.conf` text, comments removed. `None` when the text
/// has no `[httpproxy]` section and no `httpproxy.<key>` key.
fn httpproxy_fields(text: &str) -> Option<Vec<(&str, &str)>> {
    let mut section = None;
    let mut seen = false;
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if let Some(name) = header(line) {
            section = Some(name);
            seen |= name == SECTION;
        } else if let Some(pair) = proxy_key(section, line) {
            seen = true;
            out.push(pair);
        }
    }
    seen.then_some(out)
}

/// An i2pd boolean that turns an option off.
fn is_off(value: &str) -> bool {
    ["false", "0", "no", "off"]
        .iter()
        .any(|off| value.eq_ignore_ascii_case(off))
}

/// The outproxies of the HTTP proxy of one `i2pd.conf` when it serves `port`: the
/// `[httpproxy]` section, or the `httpproxy.<key>` keys before the first section.
#[must_use]
pub fn i2pd_outproxies(i2pd_conf: &str, port: u16) -> Option<Vec<String>> {
    let fields = httpproxy_fields(i2pd_conf)?;
    if value(&fields, "enabled").is_some_and(is_off) {
        return None;
    }
    let serves = value(&fields, "port").map_or(Some(I2PD_PROXY_PORT), |p| p.parse().ok());
    (serves == Some(port)).then(|| split_list(value(&fields, "outproxy").unwrap_or(""), &[',']))
}

/// The name of `file`, without its folder.
fn file_name(file: &Path) -> String {
    file.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// An HTTP proxy found: the file name and its outproxies.
type Hit = (String, Vec<String>);

/// The first file in `files` whose text `parse` finds an HTTP proxy in.
fn first_in(files: &[PathBuf], parse: impl Fn(&str) -> Option<Vec<String>>) -> Option<Hit> {
    files.iter().find_map(|file| {
        let text = fs::read_to_string(file).ok()?;
        parse(&text).map(|list| (file_name(file), list))
    })
}

/// The file groups of a router type: one per Java I2P folder, one per `i2pd.conf`.
fn groups(kind: RouterKind, env: &dyn Fn(&str) -> Option<String>) -> Vec<Vec<PathBuf>> {
    match kind {
        RouterKind::Java => java_config_dirs(env)
            .iter()
            .map(|dir| split_files(dir, "i2ptunnel.config"))
            .collect(),
        RouterKind::I2pd => i2pd_config_files(env)
            .into_iter()
            .map(|f| vec![f])
            .collect(),
    }
}

/// The first HTTP proxy on `port` of each group of `kind`.
fn hits(kind: RouterKind, port: u16, env: &dyn Fn(&str) -> Option<String>) -> Vec<Hit> {
    let parse = |text: &str| match kind {
        RouterKind::Java => java_outproxies(text, port),
        RouterKind::I2pd => i2pd_outproxies(text, port),
    };
    groups(kind, env)
        .iter()
        .filter_map(|files| first_in(files, parse))
        .collect()
}

fn decide(found: &[Hit], port: u16) -> OutproxyFinding {
    let Some((file, outproxies)) = found.first() else {
        return OutproxyFinding::Unknown(format!(
            "No router configuration with an HTTP proxy on port {port} was found."
        ));
    };
    if found.iter().any(|(_, other)| other != outproxies) {
        let files: Vec<&str> = found.iter().map(|(f, _)| f.as_str()).collect();
        return OutproxyFinding::Unknown(format!(
            "The router configurations disagree about the HTTP proxy on port {port}: {}.",
            files.join(", ")
        ));
    }
    let file = file.clone();
    if outproxies.is_empty() {
        OutproxyFinding::Clear { file }
    } else {
        OutproxyFinding::Listed {
            file,
            outproxies: outproxies.clone(),
        }
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
    if let Some(kind) = kind {
        return decide(&hits(kind, port, env), port);
    }
    let (java, i2pd) = (
        hits(RouterKind::Java, port, env),
        hits(RouterKind::I2pd, port, env),
    );
    if !java.is_empty() && !i2pd.is_empty() {
        return OutproxyFinding::Unknown(format!(
            "Both a Java I2P and an i2pd configuration use port {port}, and the router type \
             is not known."
        ));
    }
    decide(if java.is_empty() { &i2pd } else { &java }, port)
}

/// The files [`find_outproxy`] reads for `kind`, each with its modification time (`None`
/// when it cannot be read). Equal stamps mean an equal finding.
#[must_use]
pub fn config_stamps(
    kind: Option<RouterKind>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Vec<(PathBuf, Option<SystemTime>)> {
    let kinds = kind.map_or(vec![RouterKind::Java, RouterKind::I2pd], |k| vec![k]);
    kinds
        .into_iter()
        .flat_map(|k| groups(k, env))
        .flatten()
        .map(|file| {
            let modified = fs::metadata(&file).and_then(|m| m.modified()).ok();
            (file, modified)
        })
        .collect()
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
