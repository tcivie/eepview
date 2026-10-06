// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Check 3, "No outproxy" (`docs/wiki/router-checks.md`, requirement V12): the outproxies of the
//! HTTP proxy tunnel in the router configuration. The text readers run on text. `find_outproxy`
//! runs on real files in a temporary folder that the `env` closure points the router
//! configuration folders into. The tests never write outside that folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use eepview_lib::core::checks::{Outcome, outproxy_outcome};
use eepview_lib::net::console::{ConsoleKind, i2pd_config_files, java_config_dirs};
use eepview_lib::net::outproxy::{
    OutproxyFinding, find_outproxy, i2pd_outproxies, java_outproxies,
};

const PORT: u16 = 4444;

/// Helpers that may unwrap and panic (test support only).
#[cfg(test)]
mod support {
    use super::*;

    static NEXT: AtomicUsize = AtomicUsize::new(0);

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

        pub(super) fn find(&self, kind: Option<ConsoleKind>, port: u16) -> OutproxyFinding {
            find_outproxy(kind, port, &self.env())
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
}

use support::{Sandbox, assert_unknown_about_http_proxy, put};

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
fn v12_files_java_the_first_folder_wins() {
    let sandbox = Sandbox::new();
    for (n, dir) in sandbox.java_dirs().iter().enumerate() {
        let proxies = if n == 0 { "" } else { "later.i2p" };
        put(&dir.join("i2ptunnel.config"), &java_http(PORT, proxies));
    }
    let found = sandbox.find(Some(ConsoleKind::Java), PORT);
    assert_eq!(
        found,
        clear("i2ptunnel.config"),
        "V12: the first folder, in the order of R2"
    );
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
