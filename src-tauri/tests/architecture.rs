// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! The architecture test of ADR 0001 (`docs/wiki/adr-0001-no-leak-architecture.md`).
//!
//! It reads the source and fails when a no-leak rule is broken: a remote webview built
//! outside the one factory, a socket outside `net/`, `unsafe` outside the platform bridge,
//! a host check outside the one predicate, or IPC granted to a `tab-*` webview.
//! Comment lines are skipped, so docs may name any token.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

type Res<T> = Result<T, Box<dyn Error>>;

/// The crate root (`src-tauri/`).
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            out.extend(rust_files(&path)?);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(out)
}

/// The path of `file` relative to the crate root, with `/` separators.
fn rel(file: &Path) -> String {
    let path = file.strip_prefix(root()).unwrap_or(file);
    path.to_string_lossy().replace('\\', "/")
}

/// The code lines of a file: comment lines dropped.
fn code(file: &Path) -> io::Result<Vec<String>> {
    Ok(fs::read_to_string(file)?
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(str::to_owned)
        .collect())
}

/// Files of the app crate (`src/`) whose code contains `token`.
fn users(token: &str) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    for file in rust_files(&root().join("src"))? {
        if code(&file)?.iter().any(|l| l.contains(token)) {
            found.push(rel(&file));
        }
    }
    found.sort();
    Ok(found)
}

/// Fails when `token` is used outside `allowed`.
fn only_in(token: &str, allowed: &[&str]) -> io::Result<()> {
    for file in users(token)? {
        assert!(
            allowed.contains(&file.as_str()),
            "`{token}` is used in {file}; only {allowed:?} may use it (ADR 0001)"
        );
    }
    Ok(())
}

#[test]
fn one_factory_builds_remote_webviews() {
    let factories = ["src/shell/content.rs", "src/shell/chrome.rs"];
    only_in("WebviewBuilder::new", &factories).unwrap();
    only_in("on_navigation", &factories).unwrap();
    only_in("add_child", &factories).unwrap();
    only_in("proxy_url", &["src/shell/content.rs"]).unwrap();
    only_in("WebviewUrl::External", &["src/shell/content.rs"]).unwrap();
    only_in("WebviewWindowBuilder", &[]).unwrap();
}

#[test]
fn chrome_webviews_load_only_bundled_pages() {
    let chrome = code(&root().join("src/shell/chrome.rs")).unwrap();
    for line in chrome.iter().filter(|l| l.contains("WebviewBuilder::new")) {
        assert!(line.contains("WebviewUrl::App"), "{line}");
    }
}

#[test]
fn content_factory_applies_every_layer() {
    let content = code(&root().join("src/shell/content.rs"))
        .unwrap()
        .join("\n");
    for call in [
        "gatekeeper.url()",
        ".proxy_url(",
        "windows_proxy_args(",
        "rules::content_rule_list()",
        "rules::engine_allows",
        "eepview_platform::attach_rules(",
        "nav::guard",
        "initialization_script_for_all_frames(webrtc_off())",
        "NewWindowResponse::Deny",
        "WebviewUrl::External(blank)",
    ] {
        assert!(content.contains(call), "content.rs must call {call}");
    }
}

#[test]
fn sockets_live_only_in_net() {
    for token in ["TcpStream", "TcpListener", "UdpSocket", "ToSocketAddrs"] {
        for file in users(token).unwrap() {
            assert!(file.starts_with("src/net/"), "`{token}` in {file}");
        }
    }
}

#[test]
fn one_host_predicate() {
    only_in("fn is_i2p_host", &["src/net/host.rs"]).unwrap();
    only_in("ends_with(\".i2p\")", &["src/net/host.rs"]).unwrap();
    only_in("ends_with(\"i2p\")", &["src/net/host.rs"]).unwrap();
}

#[test]
fn unsafe_only_in_the_platform_bridge() {
    let markers = [
        "unsafe {",
        "unsafe fn",
        "unsafe impl",
        "unsafe extern",
        "#[unsafe(",
    ];
    for file in rust_files(&root().join("src")).unwrap() {
        for line in code(&file).unwrap() {
            let hit = markers.iter().find(|m| line.contains(*m));
            assert!(hit.is_none(), "{hit:?} in {}: {line}", rel(&file));
        }
    }
    let manifest = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    assert!(manifest.contains("unsafe_code = \"deny\""));
}

#[test]
fn bridge_rules_are_strict() {
    let manifest = fs::read_to_string(root().join("crates/eepview-platform/Cargo.toml")).unwrap();
    for lint in [
        "unsafe_op_in_unsafe_fn = \"deny\"",
        "undocumented_unsafe_blocks = \"deny\"",
        "multiple_unsafe_ops_per_block = \"deny\"",
        "missing_safety_doc = \"deny\"",
    ] {
        assert!(manifest.contains(lint), "eepview-platform must set {lint}");
    }
}

/// Every capability file as JSON.
fn capabilities() -> Res<Vec<(String, serde_json::Value)>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root().join("capabilities"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "json") {
            let text = fs::read_to_string(&path)?;
            out.push((rel(&path), serde_json::from_str(&text)?));
        }
    }
    Ok(out)
}

/// The webview labels of a capability.
fn labels(cap: &serde_json::Value) -> Vec<&str> {
    cap["webviews"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

/// The permission names of a capability.
fn permissions(cap: &serde_json::Value) -> Vec<&str> {
    cap["permissions"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

#[test]
fn capabilities_never_reach_tabs() {
    let allowed = ["toolbar", "internal", "status"];
    for (file, cap) in capabilities().unwrap() {
        assert!(!labels(&cap).is_empty(), "{file} names no webview");
        for label in labels(&cap) {
            assert!(allowed.contains(&label), "{file} names webview {label}");
        }
        assert!(cap.get("windows").is_none(), "{file} grants by window");
        assert!(cap.get("remote").is_none(), "{file} grants remote URLs");
    }
}

#[test]
fn status_bubble_gets_events_only() {
    let caps = capabilities().unwrap();
    let status = caps.iter().filter(|(_, c)| labels(c).contains(&"status"));
    for (file, cap) in status {
        let perms = permissions(cap);
        let bad: Vec<&&str> = perms
            .iter()
            .filter(|p| !p.starts_with("core:event:"))
            .collect();
        assert!(bad.is_empty(), "{file}: status gets {bad:?}");
    }
}

/// The string literals of the code lines of a file (naive: text between double quotes).
fn literals(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let parts: Vec<&str> = line.split('"').collect();
        out.extend(parts.iter().skip(1).step_by(2).map(|p| (*p).to_owned()));
    }
    out
}

/// Host-like words in a literal that are neither `.i2p` names nor loopback addresses.
fn foreign_hosts(literal: &str) -> Vec<String> {
    let lower = literal.to_ascii_lowercase();
    let words = lower.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'));
    words
        .map(|w| w.trim_matches('.'))
        .filter(|w| w.contains('.'))
        .filter(|w| is_host_like(w) && !is_i2p_or_loopback(w))
        .map(str::to_owned)
        .collect()
}

/// True for an `.i2p` name or a `127.x` address.
fn is_i2p_or_loopback(word: &str) -> bool {
    let last = word.rsplit('.').next().unwrap_or_default();
    last == "i2p" || word.starts_with("127.")
}

/// A dotted name with an alphabetic last label, or a dotted-quad IPv4 address.
fn is_host_like(word: &str) -> bool {
    let labels: Vec<&str> = word.split('.').collect();
    let last = labels.last().copied().unwrap_or_default();
    let named = last.len() >= 2 && last.chars().all(|c| c.is_ascii_alphabetic());
    let quad = labels.len() == 4 && labels.iter().all(|l| l.parse::<u8>().is_ok());
    named || quad
}

#[test]
fn verify_and_gatekeeper_name_no_clearnet_host() {
    for file in ["src/net/verify.rs", "src/net/gatekeeper.rs"] {
        let lines = code(&root().join(file)).unwrap();
        for literal in literals(&lines) {
            let hosts = foreign_hosts(&literal);
            assert!(hosts.is_empty(), "{file} names {hosts:?} in \"{literal}\"");
        }
    }
}

#[test]
fn host_scan_finds_clearnet_names() {
    assert_eq!(
        foreign_hosts("GET http://example.com/ HTTP/1.1"),
        ["example.com"]
    );
    assert_eq!(foreign_hosts("10.0.0.1:80"), ["10.0.0.1"]);
    assert!(foreign_hosts("http://proxy.i2p/ HTTP/1.1 127.0.0.1").is_empty());
    assert!(foreign_hosts("I2P HTTP proxy OK, text/html").is_empty());
}

#[test]
fn nested_webview_calls_leave_the_with_webview_closure() {
    // A Tauri webview call inside a `with_webview` closure deadlocks on Linux and Windows
    // (the dispatcher holds its window lock). These call sites must hop to another thread.
    for (file, call) in [
        ("src/shell/content.rs", "webview.navigate(url)"),
        ("src/shell/engine.rs", "find_by_script(&live"),
        ("src/shell/view.rs", "store_buttons(&handle"),
    ] {
        let text = code(&root().join(file)).unwrap().join("\n");
        let at = text
            .find(call)
            .unwrap_or_else(|| panic!("{file}: {call} not found"));
        let before = &text[at.saturating_sub(120)..at];
        assert!(
            before.contains("outside(move ||"),
            "{file}: {call} must run in apply::outside"
        );
    }
}
