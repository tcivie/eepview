// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The router console: find it on loopback and prove that it is one
//! (`docs/wiki/router-console.md`).
//!
//! eepview shows router information but never changes the router configuration. It gives
//! quick links to the router's own console pages instead. This module finds the console:
//! the candidate ports come from the router configuration files and the defaults, and a
//! candidate counts only when one loopback `GET` answers with that console's marker.
//! Only [`probe`] and [`detect`] make a [`VerifiedConsole`].

use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::Url;

use super::loopback::LoopbackAddr;
use super::verify::{Answer, parse_answer};
use crate::nav;

/// The Java I2P console port when the configuration names none.
pub const JAVA_DEFAULT_PORT: u16 = 7657;
/// The i2pd web console port when the configuration names none.
pub const I2PD_DEFAULT_PORT: u16 = 7070;

/// The class of the Java I2P console client in `clients.config`.
const JAVA_RUNNER: &str = "net.i2p.router.web.RouterConsoleRunner";
/// Text in every Java I2P console page, in any language: the console stylesheet.
const JAVA_MARKERS: [&str; 2] = ["/themes/console/", "console.css"];
/// Text in every i2pd web console page, in any language: a menu link.
const I2PD_MARKERS: [&str; 1] = ["?page=i2p_tunnels"];
/// The probe waits this long for the console (a cold Java console is slow).
const TIMEOUT: Duration = Duration::from_secs(5);
/// Most bytes read from one probe answer.
const MAX_ANSWER: u64 = 512 * 1024;

/// The page paths of each router type, in the quick-link order.
const JAVA_PAGES: [(ConsolePage, &str); 5] = [
    (ConsolePage::Home, "/home"),
    (ConsolePage::Tunnels, "/tunnels"),
    (ConsolePage::AddressBook, "/dns"),
    (ConsolePage::Config, "/config"),
    (ConsolePage::Logs, "/logs"),
];
const I2PD_PAGES: [(ConsolePage, &str); 3] = [
    (ConsolePage::Home, "/"),
    (ConsolePage::Tunnels, "/?page=tunnels"),
    (ConsolePage::Config, "/?page=commands"),
];

/// Where each router keeps its configuration on this OS: an environment variable (empty
/// for an absolute path) and the path under it.
#[cfg(target_os = "macos")]
const JAVA_DIRS: [(&str, &str); 1] = [("HOME", "Library/Application Support/i2p")];
#[cfg(target_os = "macos")]
const I2PD_FILES: [(&str, &str); 1] = [("HOME", "Library/Application Support/i2pd/i2pd.conf")];
#[cfg(windows)]
const JAVA_DIRS: [(&str, &str); 2] = [("LOCALAPPDATA", "I2P"), ("APPDATA", "I2P")];
#[cfg(windows)]
const I2PD_FILES: [(&str, &str); 1] = [("APPDATA", "i2pd/i2pd.conf")];
#[cfg(not(any(target_os = "macos", windows)))]
const JAVA_DIRS: [(&str, &str); 2] = [("HOME", ".i2p"), ("", "/var/lib/i2p/i2p-config")];
#[cfg(not(any(target_os = "macos", windows)))]
const I2PD_FILES: [(&str, &str); 2] = [("HOME", ".i2pd/i2pd.conf"), ("", "/etc/i2pd/i2pd.conf")];

/// The router type of a console.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleKind {
    /// The Java I2P router console.
    Java,
    /// The i2pd web console.
    I2pd,
}

/// A console page that eepview links to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsolePage {
    /// The console start page.
    Home,
    /// The tunnels of this router.
    Tunnels,
    /// The address book.
    AddressBook,
    /// The router settings.
    Config,
    /// The router logs.
    Logs,
}

impl ConsolePage {
    /// Every page, in the quick-link order.
    pub const ALL: [Self; 5] = [
        Self::Home,
        Self::Tunnels,
        Self::AddressBook,
        Self::Config,
        Self::Logs,
    ];

    /// The page of a page key (`home`, `tunnels`, `addressbook`, `config`, `logs`).
    #[must_use]
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|page| page.key() == key)
    }

    /// The page key, as the IPC contract spells it.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Tunnels => "tunnels",
            Self::AddressBook => "addressbook",
            Self::Config => "config",
            Self::Logs => "logs",
        }
    }
}

/// The path of `page` on a console of type `kind`. `None` when that router has no such page.
#[must_use]
pub fn page_path(kind: ConsoleKind, page: ConsolePage) -> Option<&'static str> {
    pages_of(kind)
        .iter()
        .find(|(p, _)| *p == page)
        .map(|(_, path)| *path)
}

fn pages_of(kind: ConsoleKind) -> &'static [(ConsolePage, &'static str)] {
    match kind {
        ConsoleKind::Java => &JAVA_PAGES,
        ConsoleKind::I2pd => &I2PD_PAGES,
    }
}

/// The path the probe asks for.
#[must_use]
pub const fn probe_path(kind: ConsoleKind) -> &'static str {
    match kind {
        ConsoleKind::Java => "/home",
        ConsoleKind::I2pd => "/",
    }
}

const fn markers(kind: ConsoleKind) -> &'static [&'static str] {
    match kind {
        ConsoleKind::Java => &JAVA_MARKERS,
        ConsoleKind::I2pd => &I2PD_MARKERS,
    }
}

/// `key=value` lines of a Java properties file. Comments and blank lines are dropped.
fn properties(text: &str) -> Vec<(&str, &str)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect()
}

/// The client number `<n>` of a `clientApp.<n>.<field>` key.
fn client_of<'a>(key: &'a str, field: &str) -> Option<&'a str> {
    key.strip_prefix("clientApp.")?
        .strip_suffix(field)?
        .strip_suffix('.')
}

/// The console port of one Java I2P `clients.config` text: the first plain HTTP port in the
/// `args` of the `RouterConsoleRunner` client. `None` when it does not start on load.
#[must_use]
pub fn java_console_port(clients_config: &str) -> Option<u16> {
    let props = properties(clients_config);
    let client = props
        .iter()
        .find(|(k, v)| *v == JAVA_RUNNER && client_of(k, "main").is_some())
        .and_then(|(k, _)| client_of(k, "main"))?;
    let value = |field: &str| {
        let key = format!("clientApp.{client}.{field}");
        props.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    };
    if value("startOnLoad").is_some_and(|v| v.eq_ignore_ascii_case("false")) {
        return None;
    }
    first_http_port(value("args")?)
}

/// The first port in console arguments that is not a TLS port (`-s <port>`).
fn first_http_port(args: &str) -> Option<u16> {
    let mut tokens = args.split_whitespace();
    while let Some(token) = tokens.next() {
        if token == "-s" {
            tokens.next();
        } else if let Some(port) = token.parse::<u16>().ok().filter(|p| *p != 0) {
            return Some(port);
        }
    }
    None
}

/// The web console port of one `i2pd.conf` text: `port` in the `[http]` section. `None`
/// when that section sets `enabled = false` or names no port.
#[must_use]
pub fn i2pd_console_port(i2pd_conf: &str) -> Option<u16> {
    let http = http_section(i2pd_conf);
    let value = |key: &str| http.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
    if value("enabled").is_some_and(|v| v.eq_ignore_ascii_case("false")) {
        return None;
    }
    value("port")?.parse().ok().filter(|p| *p != 0)
}

/// The `key = value` lines of the `[http]` section, comments removed.
fn http_section(text: &str) -> Vec<(&str, &str)> {
    let mut section = "";
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim();
        } else if section == "http" {
            out.extend(line.split_once('=').map(|(k, v)| (k.trim(), v.trim())));
        }
    }
    out
}

/// The paths of `table` on this OS. A path whose variable is not set is left out.
fn paths(table: &[(&str, &str)], env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for (var, rest) in table {
        let base = if var.is_empty() {
            Some(PathBuf::new())
        } else {
            env(var).filter(|v| !v.is_empty()).map(PathBuf::from)
        };
        out.extend(base.map(|b| rest.split('/').fold(b, |path, part| path.join(part))));
    }
    out
}

/// The Java I2P configuration folders of this OS.
#[must_use]
pub fn java_config_dirs(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    paths(&JAVA_DIRS, env)
}

/// The `i2pd.conf` files of this OS.
#[must_use]
pub fn i2pd_config_files(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    paths(&I2PD_FILES, env)
}

/// The console ports in a Java I2P configuration folder: every file in `clients.config.d/`
/// by name, then `clients.config`.
#[must_use]
pub fn java_ports_in(dir: &Path) -> Vec<u16> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir.join("clients.config.d"))
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    files.push(dir.join("clients.config"));
    files
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok())
        .filter_map(|text| java_console_port(&text))
        .collect()
}

/// The web console port in an `i2pd.conf` file.
#[must_use]
pub fn i2pd_port_in(file: &Path) -> Option<u16> {
    fs::read_to_string(file)
        .ok()
        .and_then(|text| i2pd_console_port(&text))
}

/// The candidates, in order: configured Java I2P ports, the configured i2pd port, then the
/// two defaults. No duplicates.
#[must_use]
pub fn candidates(env: &dyn Fn(&str) -> Option<String>) -> Vec<(ConsoleKind, u16)> {
    let java = java_config_dirs(env)
        .into_iter()
        .flat_map(|d| java_ports_in(&d));
    let i2pd = i2pd_config_files(env)
        .into_iter()
        .filter_map(|f| i2pd_port_in(&f));
    let all = java
        .map(|p| (ConsoleKind::Java, p))
        .chain(i2pd.map(|p| (ConsoleKind::I2pd, p)))
        .chain([
            (ConsoleKind::Java, JAVA_DEFAULT_PORT),
            (ConsoleKind::I2pd, I2PD_DEFAULT_PORT),
        ]);
    let mut out = Vec::new();
    for candidate in all {
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

/// True when `answer` is a `200` page with every marker of a `kind` console.
#[must_use]
pub fn judge(kind: ConsoleKind, answer: &Answer) -> bool {
    matches!(answer, Ok((200, body)) if markers(kind).iter().all(|m| body.contains(m)))
}

/// One loopback `GET` of the probe page of `kind` on `127.0.0.1:<port>`.
#[must_use]
pub fn probe(kind: ConsoleKind, port: u16) -> Option<VerifiedConsole> {
    let addr = LoopbackAddr::new(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).ok()?;
    let answer = get(addr, probe_path(kind));
    if !judge(kind, &answer) {
        return None;
    }
    let version = answer
        .ok()
        .and_then(|(_, body)| console_version(kind, &body));
    Some(VerifiedConsole {
        kind,
        port,
        version,
    })
}

/// The router version in a console page: Java I2P `console.css?<version>`, i2pd the first
/// `:</b> <version><br>` (its label is translated). Display only.
#[must_use]
pub fn console_version(kind: ConsoleKind, body: &str) -> Option<String> {
    match kind {
        ConsoleKind::Java => body
            .split("console.css?")
            .skip(1)
            .find_map(|rest| version_at(rest, "")),
        ConsoleKind::I2pd => body
            .split(":</b> ")
            .skip(1)
            .find_map(|rest| version_at(rest, "<br>")),
    }
}

/// The digits-and-dots version at the start of `text`, when `end` follows it.
fn version_at(text: &str, end: &str) -> Option<String> {
    let len = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let version = &text[..len];
    let valid = version.starts_with(|c: char| c.is_ascii_digit())
        && version.contains('.')
        && !version.ends_with('.');
    (valid && text[len..].starts_with(end)).then(|| version.to_owned())
}

/// The first candidate that passes [`probe`].
#[must_use]
pub fn detect(candidates: &[(ConsoleKind, u16)]) -> Option<VerifiedConsole> {
    candidates
        .iter()
        .find_map(|(kind, port)| probe(*kind, *port))
}

/// [`detect`] over the [`candidates`] of the process environment.
#[must_use]
pub fn detect_here() -> Option<VerifiedConsole> {
    detect(&candidates(&|var| std::env::var(var).ok()))
}

/// `GET <path>` on `addr`, no redirects followed.
fn get(addr: LoopbackAddr, path: &str) -> Answer {
    let mut stream = addr.connect(TIMEOUT).map_err(|e| format!("{addr}: {e}"))?;
    let head = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nUser-Agent: eepview\r\n\
         Accept: text/html\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(head.as_bytes())
        .map_err(|e| format!("{addr}: {e}"))?;
    let mut raw = Vec::new();
    // A timeout after some bytes still leaves a usable answer.
    let _ = (&mut stream).take(MAX_ANSWER).read_to_end(&mut raw);
    parse_answer(&raw).ok_or_else(|| format!("{addr}: not an HTTP answer"))
}

/// What the console view does with a navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleNav {
    /// Same origin: load it in the console view.
    Stay,
    /// An I2P site: open it in a normal tab instead.
    OpenTab,
    /// Anything else: drop it.
    Cancel,
}

/// A console that passed the probe. Only [`probe`] and [`detect`] make one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedConsole {
    kind: ConsoleKind,
    port: u16,
    version: Option<String>,
}

impl VerifiedConsole {
    /// The router type.
    #[must_use]
    pub fn kind(&self) -> ConsoleKind {
        self.kind
    }

    /// The console port on `127.0.0.1`.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The router version read from the probe page, display only.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// `http://127.0.0.1:<port>`.
    #[must_use]
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The URL of `page`. `None` when this router has no such page.
    #[must_use]
    pub fn url(&self, page: ConsolePage) -> Option<Url> {
        let path = page_path(self.kind, page)?;
        Url::parse(&format!("{}{path}", self.origin())).ok()
    }

    /// The pages this router has, in the quick-link order.
    #[must_use]
    pub fn pages(&self) -> Vec<ConsolePage> {
        pages_of(self.kind).iter().map(|(page, _)| *page).collect()
    }

    /// True for a URL on the console origin: `http`, `127.0.0.1`, the console port.
    fn owns(&self, url: &Url) -> bool {
        url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port_or_known_default() == Some(self.port)
            && url.username().is_empty()
            && url.password().is_none()
    }

    /// Where a navigation of the console view goes.
    #[must_use]
    pub fn route(&self, url: &Url) -> ConsoleNav {
        if self.owns(url) {
            ConsoleNav::Stay
        } else if matches!(url.scheme(), "http" | "https") && nav::guard(url) {
            ConsoleNav::OpenTab
        } else {
            ConsoleNav::Cancel
        }
    }

    /// The contract `ConsoleInfo` of this console.
    #[must_use]
    pub fn info(&self) -> ConsoleInfo {
        ConsoleInfo {
            found: true,
            kind: Some(self.kind),
            origin: Some(self.origin()),
            pages: self.pages(),
            version: self.version.clone(),
        }
    }
}

/// The contract `ConsoleInfo` type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleInfo {
    /// A console was found.
    pub found: bool,
    /// Its router type.
    pub kind: Option<ConsoleKind>,
    /// `http://127.0.0.1:<port>`, for display only.
    pub origin: Option<String>,
    /// The pages it has, in the quick-link order.
    pub pages: Vec<ConsolePage>,
    /// The router version read from the console, display only.
    pub version: Option<String>,
}

impl ConsoleInfo {
    /// No console.
    #[must_use]
    pub fn none() -> Self {
        Self {
            found: false,
            kind: None,
            origin: None,
            pages: Vec::new(),
            version: None,
        }
    }
}

#[cfg(test)]
mod tests;
