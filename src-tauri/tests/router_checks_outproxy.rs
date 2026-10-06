// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Check 3, "No outproxy" (`docs/wiki/router-checks.md`, requirement V12): the outproxies of the
//! HTTP proxy tunnel in the router configuration. The text readers run on text. `find_outproxy`
//! runs on real files in a temporary folder that the `env` closure points the router
//! configuration folders into. The tests never write outside that folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;

use eepview_lib::core::checks::{Outcome, outproxy_outcome};
use eepview_lib::net::console::{ConsoleKind, i2pd_config_files, java_config_dirs};
use eepview_lib::net::outproxy::{
    OutproxyFinding, config_stamps, find_outproxy, i2pd_outproxies, java_outproxies,
};

const PORT: u16 = 4444;

/// Helpers that may unwrap and panic (test support only).
#[cfg(test)]
mod support {
    use super::*;

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    pub(super) type Stamps = Vec<(PathBuf, Option<SystemTime>)>;

    /// A fresh folder in the system temp folder, removed on drop.
    pub(super) struct Sandbox {
        pub(super) root: PathBuf,
    }

    impl Sandbox {
        pub(super) fn new() -> Self {
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            let root = std::env::temp_dir()
                .join(format!("eepview-router-checks-{}-{n}", std::process::id()));
            fs::create_dir_all(&root).expect("a temporary folder");
            Self { root }
        }

        /// The variables of every OS, all inside the sandbox.
        pub(super) fn env(&self) -> impl Fn(&str) -> Option<String> {
            let root = self.root.clone();
            move |name| match name {
                "HOME" => Some(root.to_string_lossy().into_owned()),
                "LOCALAPPDATA" => Some(root.join("local").to_string_lossy().into_owned()),
                "APPDATA" => Some(root.join("roaming").to_string_lossy().into_owned()),
                _ => None,
            }
        }

        /// The Java I2P configuration folders of this OS that live in the sandbox, in order.
        pub(super) fn java_dirs(&self) -> Vec<PathBuf> {
            let all = java_config_dirs(&self.env());
            let inside: Vec<PathBuf> = all
                .into_iter()
                .filter(|d| d.starts_with(&self.root))
                .collect();
            assert!(!inside.is_empty(), "a Java I2P folder inside the sandbox");
            inside
        }

        pub(super) fn java_dir(&self) -> PathBuf {
            self.java_dirs().remove(0)
        }

        pub(super) fn i2pd_file(&self) -> PathBuf {
            let all = i2pd_config_files(&self.env());
            let inside = all.into_iter().find(|f| f.starts_with(&self.root));
            inside.expect("an i2pd.conf path inside the sandbox")
        }

        /// The i2pd.conf paths of this OS that live in the sandbox, in order.
        pub(super) fn i2pd_files(&self) -> Vec<PathBuf> {
            let all = i2pd_config_files(&self.env());
            let inside: Vec<PathBuf> = all
                .into_iter()
                .filter(|f| f.starts_with(&self.root))
                .collect();
            assert!(!inside.is_empty(), "an i2pd.conf path inside the sandbox");
            inside
        }

        pub(super) fn find(&self, kind: Option<ConsoleKind>, port: u16) -> OutproxyFinding {
            find_outproxy(kind, port, &self.env())
        }

        pub(super) fn stamps(&self, kind: Option<ConsoleKind>) -> Stamps {
            config_stamps(kind, &self.env())
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap_or_default();
        }
    }

    pub(super) fn put(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().expect("a parent folder")).expect("a folder");
        fs::write(path, text).expect("a file");
    }

    pub(super) fn assert_unknown_about_http_proxy(finding: &OutproxyFinding) {
        match finding {
            OutproxyFinding::Unknown(reason) => {
                let lower = reason.to_lowercase();
                assert!(
                    lower.contains("http proxy"),
                    "V12: no HTTP proxy found: {reason:?}"
                );
            }
            other => panic!("V12: expected unknown, got {other:?}"),
        }
    }

    /// The configurations disagree: unknown, and the reason says so and names the file.
    pub(super) fn assert_unknown_disagree(finding: &OutproxyFinding, file: &str) {
        match finding {
            OutproxyFinding::Unknown(reason) => {
                let lower = reason.to_lowercase();
                assert!(lower.contains("disagree"), "V12: disagree: {reason:?}");
                assert!(lower.contains(file), "V12: names the files: {reason:?}");
            }
            other => panic!("V12: expected unknown (disagree), got {other:?}"),
        }
    }

    /// Some stamp is for a path that ends with `tail`.
    pub(super) fn has_stamp(stamps: &Stamps, tail: &Path) -> bool {
        stamps.iter().any(|(path, _)| path.ends_with(tail))
    }

    /// Some stamp is for a path that ends with `tail`, and carries a modification time.
    pub(super) fn has_stamp_with_time(stamps: &Stamps, tail: &Path) -> bool {
        stamps
            .iter()
            .any(|(path, time)| path.ends_with(tail) && time.is_some())
    }

    /// Some stamp is for a Java I2P tunnel file or for a file in a tunnel folder.
    pub(super) fn names_java_file(stamps: &Stamps) -> bool {
        stamps.iter().any(|(path, _)| {
            path.file_name() == Some(std::ffi::OsStr::new("i2ptunnel.config"))
                || path
                    .components()
                    .any(|c| c.as_os_str() == "i2ptunnel.config.d")
        })
    }
}

use support::{
    Sandbox, Stamps, assert_unknown_about_http_proxy, assert_unknown_disagree, has_stamp,
    has_stamp_with_time, names_java_file, put,
};

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| String::from(*s)).collect()
}

fn java_http(listen_port: u16, proxy_list: &str) -> String {
    format!(
        "tunnel.0.type=httpclient\ntunnel.0.listenPort={listen_port}\ntunnel.0.proxyList={proxy_list}\n"
    )
}

fn i2pd_http(port: u16, outproxy: &str) -> String {
    format!("[httpproxy]\nenabled = true\nport = {port}\noutproxy = {outproxy}\n")
}

fn listed(file: &str, outproxies: &[&str]) -> OutproxyFinding {
    OutproxyFinding::Listed {
        file: file.into(),
        outproxies: strings(outproxies),
    }
}

fn clear(file: &str) -> OutproxyFinding {
    OutproxyFinding::Clear { file: file.into() }
}

// ---------------------------------------------------------------------------------------------
// V12, Java I2P: one file of text.

#[test]
fn v12_java_an_http_proxy_with_no_outproxy_gives_an_empty_list() {
    let text = "tunnel.0.type=httpclient\ntunnel.0.listenPort=4444\n";
    assert_eq!(java_outproxies(text, PORT), Some(vec![]), "V12");
    assert_eq!(
        java_outproxies(&java_http(PORT, ""), PORT),
        Some(vec![]),
        "V12: empty proxyList"
    );
}

#[test]
fn v12_java_the_proxy_list_gives_the_outproxies() {
    let found = java_outproxies(&java_http(PORT, "exit.i2p"), PORT);
    assert_eq!(found, Some(strings(&["exit.i2p"])), "V12");
}

#[test]
fn v12_java_the_proxy_list_splits_on_comma_semicolon_and_white_space() {
    let text = java_http(PORT, "a.i2p, b.i2p;c.i2p d.i2p\te.i2p");
    let want = Some(strings(&["a.i2p", "b.i2p", "c.i2p", "d.i2p", "e.i2p"]));
    assert_eq!(java_outproxies(&text, PORT), want, "V12");
}

#[test]
fn v12_java_empty_parts_and_repeats_are_dropped_and_the_order_is_kept() {
    let text = java_http(PORT, "b.i2p,,;a.i2p,b.i2p ; ;c.i2p,a.i2p");
    let want = Some(strings(&["b.i2p", "a.i2p", "c.i2p"]));
    assert_eq!(java_outproxies(&text, PORT), want, "V12");
}

#[test]
fn v12_java_the_ssl_outproxies_option_counts_too() {
    let mut text = java_http(PORT, "a.i2p");
    text.push_str("tunnel.0.option.i2ptunnel.httpclient.SSLOutproxies=ssl-one.i2p,ssl-two.i2p\n");
    let found = java_outproxies(&text, PORT).expect("an HTTP proxy");
    for name in ["a.i2p", "ssl-one.i2p", "ssl-two.i2p"] {
        assert!(found.iter().any(|o| o == name), "V12: {name} in {found:?}");
    }
    assert_eq!(found.len(), 3, "V12: {found:?}");
    let only_ssl = "tunnel.0.type=httpclient\ntunnel.0.listenPort=4444\n\
                    tunnel.0.option.i2ptunnel.httpclient.SSLOutproxies=ssl.i2p\n";
    assert_eq!(
        java_outproxies(only_ssl, PORT),
        Some(strings(&["ssl.i2p"])),
        "V12"
    );
}

#[test]
fn v12_java_only_an_httpclient_tunnel_is_the_http_proxy() {
    for kind in [
        "connectclient",
        "socksclient",
        "client",
        "ircclient",
        "httpserver",
    ] {
        let text = format!(
            "tunnel.0.type={kind}\ntunnel.0.listenPort=4444\ntunnel.0.proxyList=exit.i2p\n"
        );
        assert_eq!(
            java_outproxies(&text, PORT),
            None,
            "V12: {kind} is not the HTTP proxy"
        );
    }
}

#[test]
fn v12_java_only_the_tunnel_on_the_proxy_port_counts() {
    let other = java_http(4445, "exit.i2p");
    assert_eq!(java_outproxies(&other, PORT), None, "V12: another port");
    assert_eq!(
        java_outproxies(&other, 4445),
        Some(strings(&["exit.i2p"])),
        "V12: that port"
    );
    assert_eq!(java_outproxies("", PORT), None, "V12: empty file");
    assert_eq!(
        java_outproxies("tunnel.0.type=httpclient\n", PORT),
        None,
        "V12: no port"
    );
}

#[test]
fn v12_java_the_keys_of_one_tunnel_never_mix_with_another() {
    let text = "tunnel.0.type=httpclient\ntunnel.0.listenPort=4445\ntunnel.0.proxyList=other.i2p\n\
                tunnel.1.type=httpclient\ntunnel.1.listenPort=4444\n\
                tunnel.2.type=httpclient\ntunnel.2.listenPort=4446\ntunnel.2.proxyList=third.i2p\n";
    assert_eq!(
        java_outproxies(text, PORT),
        Some(vec![]),
        "V12: tunnel 1 has none"
    );
    assert_eq!(
        java_outproxies(text, 4445),
        Some(strings(&["other.i2p"])),
        "V12: tunnel 0"
    );
    assert_eq!(
        java_outproxies(text, 4446),
        Some(strings(&["third.i2p"])),
        "V12: tunnel 2"
    );
}

#[test]
fn v12_java_a_tunnel_file_holds_one_tunnel_with_plain_or_numbered_keys() {
    let plain = "type=httpclient\nlistenPort=4444\nproxyList=exit.i2p\n";
    assert_eq!(
        java_outproxies(plain, PORT),
        Some(strings(&["exit.i2p"])),
        "V12: plain keys"
    );
    let numbered = "tunnel.7.type=httpclient\ntunnel.7.listenPort=4444\ntunnel.7.proxyList=x.i2p\n";
    assert_eq!(
        java_outproxies(numbered, PORT),
        Some(strings(&["x.i2p"])),
        "V12: tunnel.<n>"
    );
    let ssl = "type=httpclient\nlistenPort=4444\noption.i2ptunnel.httpclient.SSLOutproxies=s.i2p\n";
    assert_eq!(
        java_outproxies(ssl, PORT),
        Some(strings(&["s.i2p"])),
        "V12: plain SSL key"
    );
}

#[test]
fn v12_java_comment_lines_start_with_hash_or_bang() {
    let text = "# tunnel.0.proxyList=hash.i2p\n! tunnel.0.proxyList=bang.i2p\n\
                tunnel.0.type=httpclient\ntunnel.0.listenPort=4444\n";
    assert_eq!(
        java_outproxies(text, PORT),
        Some(vec![]),
        "V12: comments are not keys"
    );
    let commented = "# tunnel.0.type=httpclient\n! tunnel.0.listenPort=4444\n";
    assert_eq!(
        java_outproxies(commented, PORT),
        None,
        "V12: a commented tunnel is no tunnel"
    );
}

// ---------------------------------------------------------------------------------------------
// V12, i2pd: one file of text.

#[test]
fn v12_i2pd_the_outproxy_value_splits_on_comma_trimmed() {
    let text = i2pd_http(PORT, "http://exit-one.i2p:4444 , http://exit-two.i2p:4444");
    let want = Some(strings(&[
        "http://exit-one.i2p:4444",
        "http://exit-two.i2p:4444",
    ]));
    assert_eq!(i2pd_outproxies(&text, PORT), want, "V12");
}

#[test]
fn v12_i2pd_empty_parts_are_dropped() {
    let text = i2pd_http(PORT, ", http://a.i2p ,, ,http://b.i2p,");
    let want = Some(strings(&["http://a.i2p", "http://b.i2p"]));
    assert_eq!(i2pd_outproxies(&text, PORT), want, "V12");
}

#[test]
fn v12_i2pd_a_proxy_with_no_outproxy_gives_an_empty_list() {
    assert_eq!(
        i2pd_outproxies(&i2pd_http(PORT, ""), PORT),
        Some(vec![]),
        "V12: empty value"
    );
    let none = "[httpproxy]\nenabled = true\nport = 4444\n";
    assert_eq!(
        i2pd_outproxies(none, PORT),
        Some(vec![]),
        "V12: no outproxy key"
    );
}

#[test]
fn v12_i2pd_the_port_defaults_to_4444() {
    let text = "[httpproxy]\nenabled = true\n";
    assert_eq!(
        i2pd_outproxies(text, 4444),
        Some(vec![]),
        "V12: the default port"
    );
    assert_eq!(
        i2pd_outproxies(text, 4445),
        None,
        "V12: not the default port"
    );
    assert_eq!(
        i2pd_outproxies("[httpproxy]\n", 4444),
        Some(vec![]),
        "V12: enabled is not false"
    );
}

#[test]
fn v12_i2pd_the_proxy_serves_the_port_of_its_port_key() {
    let text = i2pd_http(4445, "http://exit.i2p");
    assert_eq!(i2pd_outproxies(&text, PORT), None, "V12: another port");
    assert_eq!(
        i2pd_outproxies(&text, 4445),
        Some(strings(&["http://exit.i2p"])),
        "V12"
    );
}

#[test]
fn v12_i2pd_enabled_false_is_no_http_proxy() {
    for line in ["enabled = false", "enabled=false"] {
        let text = format!("[httpproxy]\n{line}\nport = 4444\noutproxy = http://exit.i2p\n");
        assert_eq!(i2pd_outproxies(&text, PORT), None, "V12: {line}");
    }
}

#[test]
fn v12_i2pd_only_the_httpproxy_section_counts() {
    let text = "[http]\nenabled = true\nport = 4444\noutproxy = http://wrong.i2p\n\
                [socksproxy]\nport = 4444\noutproxy = http://socks.i2p\n\
                [httpproxy]\nport = 4444\n\
                [other]\noutproxy = http://later.i2p\n";
    assert_eq!(
        i2pd_outproxies(text, PORT),
        Some(vec![]),
        "V12: keys of other sections"
    );
    let outside = "outproxy = http://top.i2p\nport = 4444\n";
    assert_eq!(
        i2pd_outproxies(outside, PORT),
        None,
        "V12: keys before any section"
    );
    assert_eq!(
        i2pd_outproxies("[http]\nport = 4444\n", PORT),
        None,
        "V12: no [httpproxy]"
    );
    assert_eq!(i2pd_outproxies("", PORT), None, "V12: empty file");
}

#[test]
fn v12_i2pd_a_comment_line_starts_with_hash() {
    let text = "[httpproxy]\n# outproxy = http://hidden.i2p\nport = 4444\n";
    assert_eq!(
        i2pd_outproxies(text, PORT),
        Some(vec![]),
        "V12: commented key"
    );
    let off = "[httpproxy]\n# enabled = true\nenabled = false\nport = 4444\n";
    assert_eq!(
        i2pd_outproxies(off, PORT),
        None,
        "V12: commented key does not undo"
    );
}

#[test]
fn v12_i2pd_a_dotted_key_before_the_first_section_is_a_key_of_httpproxy() {
    let text = "httpproxy.enabled = true\nhttpproxy.port = 4444\n\
                httpproxy.outproxy = http://a.i2p , http://b.i2p\n";
    let want = Some(strings(&["http://a.i2p", "http://b.i2p"]));
    assert_eq!(i2pd_outproxies(text, PORT), want, "V12: only dotted keys");
    assert_eq!(
        i2pd_outproxies("httpproxy.port = 4445\n", 4445),
        Some(vec![]),
        "V12: a file with only a dotted port has an HTTP proxy"
    );
    assert_eq!(
        i2pd_outproxies("httpproxy.port = 4445\n", PORT),
        None,
        "V12: the dotted port is the port"
    );
    assert_eq!(
        i2pd_outproxies("httpproxy.enabled = true\n", PORT),
        Some(vec![]),
        "V12: the default port"
    );
}

#[test]
fn v12_i2pd_dotted_keys_and_the_section_add_up() {
    let text = "httpproxy.outproxy = http://dotted.i2p\n[httpproxy]\nport = 4444\n";
    assert_eq!(
        i2pd_outproxies(text, PORT),
        Some(strings(&["http://dotted.i2p"])),
        "V12: dotted outproxy, section port"
    );
    let other = "httpproxy.port = 4445\n[httpproxy]\noutproxy = http://section.i2p\n";
    assert_eq!(
        i2pd_outproxies(other, 4445),
        Some(strings(&["http://section.i2p"])),
        "V12: dotted port, section outproxy"
    );
}

#[test]
fn v12_i2pd_enabled_is_off_for_false_zero_no_and_off_in_any_case() {
    for value in ["false", "0", "no", "off", "OFF", "Off", "FALSE", "No"] {
        let section = format!("[httpproxy]\nenabled = {value}\nport = 4444\n");
        assert_eq!(
            i2pd_outproxies(&section, PORT),
            None,
            "V12: enabled = {value}"
        );
        let dotted = format!("httpproxy.enabled = {value}\nhttpproxy.port = 4444\n");
        assert_eq!(
            i2pd_outproxies(&dotted, PORT),
            None,
            "V12: httpproxy.enabled = {value}"
        );
    }
}

#[test]
fn v12_i2pd_enabled_is_on_for_any_other_value() {
    for value in ["true", "1", "yes", "on", "ON", "True"] {
        let section = format!("[httpproxy]\nenabled = {value}\nport = 4444\n");
        assert_eq!(
            i2pd_outproxies(&section, PORT),
            Some(vec![]),
            "V12: enabled = {value}"
        );
    }
}

#[test]
fn v12_i2pd_a_dotted_key_after_a_section_header_belongs_to_that_section() {
    let alone = "[other]\nhttpproxy.enabled = true\nhttpproxy.port = 4444\n";
    assert_eq!(
        i2pd_outproxies(alone, PORT),
        None,
        "V12: keys of [other] are not an HTTP proxy"
    );
    let outproxy = "[httpproxy]\nport = 4444\n[other]\nhttpproxy.outproxy = http://x.i2p\n";
    assert_eq!(
        i2pd_outproxies(outproxy, PORT),
        Some(vec![]),
        "V12: the dotted outproxy in [other] is not an outproxy of the HTTP proxy"
    );
    let off = "[httpproxy]\nport = 4444\n[other]\nhttpproxy.enabled = false\n";
    assert_eq!(
        i2pd_outproxies(off, PORT),
        Some(vec![]),
        "V12: the dotted enabled in [other] does not switch the HTTP proxy off"
    );
    let port = "[httpproxy]\nport = 4444\n[other]\nhttpproxy.port = 4445\n";
    assert_eq!(
        i2pd_outproxies(port, 4445),
        None,
        "V12: the dotted port in [other] is not the proxy port"
    );
}

// ---------------------------------------------------------------------------------------------
// V12, the files of this OS (find_outproxy on real folders).

#[test]
fn v12_files_no_router_configuration_gives_unknown() {
    let sandbox = Sandbox::new();
    for kind in [None, Some(ConsoleKind::Java), Some(ConsoleKind::I2pd)] {
        assert_unknown_about_http_proxy(&sandbox.find(kind, PORT));
    }
}

#[test]
fn v12_files_java_http_proxy_with_no_outproxy_is_clear_and_named_by_file_name() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, ""),
    );
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        found,
        clear("i2ptunnel.config"),
        "V12: the file name only, not the folder"
    );
}

#[test]
fn v12_files_java_http_proxy_with_outproxies_is_listed() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, "exit.i2p,b.i2p"),
    );
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        found,
        listed("i2ptunnel.config", &["exit.i2p", "b.i2p"]),
        "V12"
    );
}

#[test]
fn v12_files_java_reads_the_tunnel_config_folder_by_file_name() {
    let sandbox = Sandbox::new();
    let tunnels = sandbox.java_dir().join("i2ptunnel.config.d");
    put(
        &tunnels.join("00-http-proxy-tunnel.config"),
        "type=httpclient\nlistenPort=4444\n",
    );
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        found,
        clear("00-http-proxy-tunnel.config"),
        "V12: name only, not the folder"
    );
}

#[test]
fn v12_files_java_files_of_the_folder_come_in_order_of_file_name() {
    let sandbox = Sandbox::new();
    let tunnels = sandbox.java_dir().join("i2ptunnel.config.d");
    put(
        &tunnels.join("b-tunnel.config"),
        "type=httpclient\nlistenPort=4444\nproxyList=b.i2p\n",
    );
    put(
        &tunnels.join("a-tunnel.config"),
        "type=httpclient\nlistenPort=4444\nproxyList=a.i2p\n",
    );
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        found,
        listed("a-tunnel.config", &["a.i2p"]),
        "V12: the first file wins"
    );
}

#[test]
fn v12_files_java_the_folder_comes_before_i2ptunnel_config() {
    let sandbox = Sandbox::new();
    let dir = sandbox.java_dir();
    put(&dir.join("i2ptunnel.config"), &java_http(PORT, "old.i2p"));
    put(
        &dir.join("i2ptunnel.config.d").join("z.config"),
        "type=httpclient\nlistenPort=4444\n",
    );
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        found,
        clear("z.config"),
        "V12: i2ptunnel.config.d first, then i2ptunnel.config"
    );
}

#[test]
fn v12_files_java_a_file_without_the_proxy_is_skipped() {
    let sandbox = Sandbox::new();
    let dir = sandbox.java_dir();
    put(
        &dir.join("i2ptunnel.config.d").join("a.config"),
        "type=httpclient\nlistenPort=4999\n",
    );
    put(
        &dir.join("i2ptunnel.config.d").join("b.config"),
        "type=connectclient\nlistenPort=4444\n",
    );
    put(&dir.join("i2ptunnel.config"), &java_http(PORT, "exit.i2p"));
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        found,
        listed("i2ptunnel.config", &["exit.i2p"]),
        "V12: the first HTTP proxy"
    );
}

#[test]
fn v12_files_java_folders_that_disagree_give_unknown() {
    let sandbox = Sandbox::new();
    let dirs = sandbox.java_dirs();
    for (n, dir) in dirs.iter().enumerate() {
        let proxies = if n == 0 { "" } else { "later.i2p" };
        put(&dir.join("i2ptunnel.config"), &java_http(PORT, proxies));
    }
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    if dirs.len() >= 2 {
        assert_unknown_disagree(&found, "i2ptunnel.config");
    } else {
        assert_eq!(
            found,
            clear("i2ptunnel.config"),
            "V12: one folder, no clash"
        );
    }
}

#[test]
fn v12_files_java_folders_that_agree_give_the_first_folders_file() {
    let sandbox = Sandbox::new();
    for (n, dir) in sandbox.java_dirs().iter().enumerate() {
        let text = java_http(PORT, "same.i2p");
        if n == 0 {
            put(&dir.join("i2ptunnel.config.d").join("first.config"), &text);
        } else {
            put(&dir.join("i2ptunnel.config"), &text);
        }
    }
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        found,
        listed("first.config", &["same.i2p"]),
        "V12: equal lists, the first file in the order of R2 is named"
    );
}

#[test]
fn v12_files_java_folders_that_agree_on_no_outproxy_give_clear_of_the_first_file() {
    let sandbox = Sandbox::new();
    for (n, dir) in sandbox.java_dirs().iter().enumerate() {
        let text = java_http(PORT, "");
        if n == 0 {
            put(&dir.join("i2ptunnel.config"), &text);
        } else {
            put(&dir.join("i2ptunnel.config.d").join("later.config"), &text);
        }
    }
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(found, clear("i2ptunnel.config"), "V12: first folder's file");
}

#[test]
fn v12_files_i2pd_files_that_disagree_give_unknown() {
    let sandbox = Sandbox::new();
    let files = sandbox.i2pd_files();
    for (n, file) in files.iter().enumerate() {
        let proxies = if n == 0 { "" } else { "http://later.i2p" };
        put(file, &i2pd_http(PORT, proxies));
    }
    let found = sandbox.find(Some(ConsoleKind::I2pd), PORT);
    if files.len() >= 2 {
        assert_unknown_disagree(&found, "i2pd.conf");
    } else {
        assert_eq!(found, clear("i2pd.conf"), "V12: one file, no clash");
    }
}

#[test]
fn v12_files_i2pd_reads_a_file_with_only_dotted_keys() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.i2pd_file(),
        "httpproxy.enabled = true\nhttpproxy.port = 4444\n",
    );
    assert_eq!(
        sandbox.find(Some(ConsoleKind::I2pd), PORT),
        clear("i2pd.conf"),
        "V12: dotted keys only"
    );
    put(
        &sandbox.i2pd_file(),
        "httpproxy.enabled = off\nhttpproxy.port = 4444\n",
    );
    assert_unknown_about_http_proxy(&sandbox.find(Some(ConsoleKind::I2pd), PORT));
}

#[test]
fn v12_files_the_proxy_port_picks_the_tunnel() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(4445, "exit.i2p"),
    );
    assert_unknown_about_http_proxy(&sandbox.find(Some(ConsoleKind::Java), PORT));
    let found = sandbox.find(Some(ConsoleKind::Java), 4445);
    assert_eq!(found, listed("i2ptunnel.config", &["exit.i2p"]), "V12");
}

#[test]
fn v12_files_an_unreadable_file_counts_as_no_http_proxy() {
    let sandbox = Sandbox::new();
    // A folder in the place of the file cannot be read as text.
    fs::create_dir_all(sandbox.java_dir().join("i2ptunnel.config")).expect("a folder");
    assert_unknown_about_http_proxy(&sandbox.find(Some(ConsoleKind::Java), PORT));
    put(
        &sandbox
            .java_dir()
            .join("i2ptunnel.config.d")
            .join("a.config"),
        "type=httpclient\nlistenPort=4444\n",
    );
    assert_eq!(
        sandbox.find(Some(ConsoleKind::Java), PORT),
        clear("a.config"),
        "V12: skip it"
    );
}

#[test]
fn v12_files_i2pd_http_proxy_is_read_from_i2pd_conf() {
    let sandbox = Sandbox::new();
    put(&sandbox.i2pd_file(), &i2pd_http(PORT, ""));
    let found = sandbox.find(Some(ConsoleKind::I2pd), PORT);
    assert_eq!(
        found,
        clear("i2pd.conf"),
        "V12: the file name only, not the folder"
    );
    put(
        &sandbox.i2pd_file(),
        &i2pd_http(PORT, "http://exit.i2p:4444"),
    );
    let found = sandbox.find(Some(ConsoleKind::I2pd), PORT);
    assert_eq!(found, listed("i2pd.conf", &["http://exit.i2p:4444"]), "V12");
}

#[test]
fn v12_files_i2pd_without_the_proxy_on_the_port_gives_unknown() {
    let sandbox = Sandbox::new();
    put(&sandbox.i2pd_file(), &i2pd_http(4445, ""));
    assert_unknown_about_http_proxy(&sandbox.find(Some(ConsoleKind::I2pd), PORT));
    put(
        &sandbox.i2pd_file(),
        "[httpproxy]\nenabled = false\nport = 4444\n",
    );
    assert_unknown_about_http_proxy(&sandbox.find(Some(ConsoleKind::I2pd), PORT));
}

#[test]
fn v12_files_a_router_type_reads_only_its_own_files() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, "java-exit.i2p"),
    );
    assert_unknown_about_http_proxy(&sandbox.find(Some(ConsoleKind::I2pd), PORT));
    let other = Sandbox::new();
    put(&other.i2pd_file(), &i2pd_http(PORT, "http://i2pd-exit.i2p"));
    assert_unknown_about_http_proxy(&other.find(Some(ConsoleKind::Java), PORT));
}

#[test]
fn v12_files_an_unknown_type_reads_both_kinds_of_files() {
    let java = Sandbox::new();
    put(
        &java.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, "java-exit.i2p"),
    );
    assert_eq!(
        java.find(None, PORT),
        listed("i2ptunnel.config", &["java-exit.i2p"]),
        "V12"
    );
    let i2pd = Sandbox::new();
    put(&i2pd.i2pd_file(), &i2pd_http(PORT, ""));
    assert_eq!(i2pd.find(None, PORT), clear("i2pd.conf"), "V12");
}

#[test]
fn v12_files_an_unknown_type_with_both_on_the_port_is_unknown() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, ""),
    );
    put(&sandbox.i2pd_file(), &i2pd_http(PORT, ""));
    match sandbox.find(None, PORT) {
        OutproxyFinding::Unknown(reason) => {
            let lower = reason.to_lowercase();
            assert!(
                lower.contains("java") && lower.contains("i2pd"),
                "V12: {reason:?}"
            );
        }
        other => panic!("V12: both types use the port, expected unknown, got {other:?}"),
    }
}

#[test]
fn v12_files_an_unknown_type_with_one_proxy_on_the_port_finds_that_one() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(4445, "java-exit.i2p"),
    );
    put(&sandbox.i2pd_file(), &i2pd_http(PORT, ""));
    assert_eq!(
        sandbox.find(None, PORT),
        clear("i2pd.conf"),
        "V12: only i2pd serves the port"
    );
    let found = sandbox.find(None, 4445);
    assert_eq!(
        found,
        listed("i2ptunnel.config", &["java-exit.i2p"]),
        "V12: only Java does"
    );
}

#[test]
fn v12_files_both_types_on_the_port_give_a_decision_once_the_type_is_known() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, "java-exit.i2p"),
    );
    put(&sandbox.i2pd_file(), &i2pd_http(PORT, ""));
    let java = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        java,
        listed("i2ptunnel.config", &["java-exit.i2p"]),
        "V12: Java type"
    );
    assert_eq!(
        sandbox.find(Some(ConsoleKind::I2pd), PORT),
        clear("i2pd.conf"),
        "V12: i2pd type"
    );
}

#[test]
fn v12_files_a_detail_names_the_file_by_name_and_never_by_folder() {
    let sandbox = Sandbox::new();
    let root = sandbox.root.to_string_lossy().into_owned();
    let tunnels = sandbox.java_dir().join("i2ptunnel.config.d");
    put(
        &tunnels.join("00-http.config"),
        &java_http(PORT, "exit.i2p"),
    );
    let java = sandbox.find(Some(ConsoleKind::Java), PORT);
    let detail = detail_of(&outproxy_outcome(&java));
    assert!(
        detail.contains("00-http.config"),
        "V12: names the file: {detail:?}"
    );
    assert!(!detail.contains(&root), "V12: never the folder: {detail:?}");
    assert!(
        !detail.contains("i2ptunnel.config.d"),
        "V12: never the folder: {detail:?}"
    );
    put(&sandbox.i2pd_file(), &i2pd_http(PORT, ""));
    let i2pd = sandbox.find(Some(ConsoleKind::I2pd), PORT);
    let detail = detail_of(&outproxy_outcome(&i2pd));
    assert!(
        detail.contains("i2pd.conf") && !detail.contains(&root),
        "V12: {detail:?}"
    );
}

fn detail_of(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Passed(detail) => detail.clone().unwrap_or_default(),
        Outcome::Failed(detail) | Outcome::NotChecked(detail) => detail.clone(),
    }
}

#[test]
fn v12_files_the_result_of_a_find_gives_the_outcome_of_the_spec() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, "exit.i2p"),
    );
    let failed = outproxy_outcome(&sandbox.find(Some(ConsoleKind::Java), PORT));
    assert!(
        matches!(failed, Outcome::Failed(_)),
        "V12: an outproxy fails: {failed:?}"
    );
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, ""),
    );
    let passed = outproxy_outcome(&sandbox.find(Some(ConsoleKind::Java), PORT));
    assert!(
        matches!(passed, Outcome::Passed(Some(_))),
        "V12: no outproxy passes: {passed:?}"
    );
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(4999, ""),
    );
    let none = outproxy_outcome(&sandbox.find(Some(ConsoleKind::Java), PORT));
    assert!(
        matches!(none, Outcome::NotChecked(_)),
        "V12: no HTTP proxy: {none:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// V12, the stamps of the files that find_outproxy reads.

#[test]
fn v12_stamps_include_a_tunnel_file_in_the_java_folder_with_its_time() {
    let sandbox = Sandbox::new();
    let tail = Path::new("i2ptunnel.config.d").join("a.config");
    put(
        &sandbox.java_dir().join(&tail),
        "type=httpclient\nlistenPort=4444\n",
    );
    let stamps = sandbox.stamps(Some(ConsoleKind::Java));
    assert!(has_stamp_with_time(&stamps, &tail), "V12: {stamps:?}");
    let unknown_kind = sandbox.stamps(None);
    assert!(
        has_stamp_with_time(&unknown_kind, &tail),
        "V12: an unknown type lists the Java files too: {unknown_kind:?}"
    );
}

#[test]
fn v12_stamps_include_i2ptunnel_config_when_it_exists() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, ""),
    );
    let stamps = sandbox.stamps(Some(ConsoleKind::Java));
    assert!(
        has_stamp_with_time(&stamps, Path::new("i2ptunnel.config")),
        "V12: {stamps:?}"
    );
}

#[test]
fn v12_stamps_change_when_a_new_tunnel_file_is_written() {
    let sandbox = Sandbox::new();
    let folder = sandbox.java_dir().join("i2ptunnel.config.d");
    let empty = sandbox.stamps(Some(ConsoleKind::Java));
    put(
        &folder.join("a.config"),
        "type=httpclient\nlistenPort=4444\n",
    );
    let one = sandbox.stamps(Some(ConsoleKind::Java));
    assert_ne!(empty, one, "V12: the first tunnel file changes the stamps");
    put(
        &folder.join("b.config"),
        "type=httpclient\nlistenPort=4445\n",
    );
    let two = sandbox.stamps(Some(ConsoleKind::Java));
    assert_ne!(one, two, "V12: another tunnel file changes the stamps");
    assert_eq!(
        two,
        sandbox.stamps(Some(ConsoleKind::Java)),
        "V12: nothing written, the same stamps"
    );
}

#[test]
fn v12_stamps_of_i2pd_list_no_java_file() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, ""),
    );
    put(
        &sandbox
            .java_dir()
            .join("i2ptunnel.config.d")
            .join("a.config"),
        "type=httpclient\nlistenPort=4444\n",
    );
    put(&sandbox.i2pd_file(), &i2pd_http(PORT, ""));
    let stamps: Stamps = sandbox.stamps(Some(ConsoleKind::I2pd));
    assert!(!names_java_file(&stamps), "V12: no Java file: {stamps:?}");
    assert!(
        has_stamp_with_time(&stamps, Path::new("i2pd.conf")),
        "V12: the i2pd file: {stamps:?}"
    );
}

#[test]
fn v12_stamps_of_java_list_no_i2pd_file() {
    let sandbox = Sandbox::new();
    put(
        &sandbox.java_dir().join("i2ptunnel.config"),
        &java_http(PORT, ""),
    );
    put(&sandbox.i2pd_file(), &i2pd_http(PORT, ""));
    let stamps = sandbox.stamps(Some(ConsoleKind::Java));
    assert!(
        !has_stamp(&stamps, Path::new("i2pd.conf")),
        "V12: no i2pd file: {stamps:?}"
    );
    let all = sandbox.stamps(None);
    assert!(
        has_stamp(&all, Path::new("i2pd.conf")),
        "V12: an unknown type lists the i2pd files too: {all:?}"
    );
}
