//! The architecture test of ADR 0001 (`docs/wiki/adr-0001-no-leak-architecture.md`).
//!
//! It reads the source and fails when a no-leak rule is broken: a remote webview built
//! outside the one factory, a socket outside `net/`, `unsafe` outside the platform bridge,
//! a host check outside the one predicate, or IPC granted to a `tab-*` webview.
//! Comment lines are skipped, so docs may name any token.

use std::fs;
use std::path::{Path, PathBuf};

/// The crate root (`src-tauri/`).
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// The path of `file` relative to the crate root, with `/` separators.
fn rel(file: &Path) -> String {
    let path = file.strip_prefix(root()).unwrap_or(file);
    path.to_string_lossy().replace('\\', "/")
}

/// The code lines of a file: comment lines dropped.
fn code(file: &Path) -> Vec<String> {
    fs::read_to_string(file)
        .unwrap()
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(str::to_owned)
        .collect()
}

/// Files of the app crate (`src/`) whose code contains `token`.
fn users(token: &str) -> Vec<String> {
    let mut found: Vec<String> = rust_files(&root().join("src"))
        .into_iter()
        .filter(|f| code(f).iter().any(|l| l.contains(token)))
        .map(|f| rel(&f))
        .collect();
    found.sort();
    found
}

/// Fails when `token` is used outside `allowed`.
fn only_in(token: &str, allowed: &[&str]) {
    for file in users(token) {
        assert!(
            allowed.contains(&file.as_str()),
            "`{token}` is used in {file}; only {allowed:?} may use it (ADR 0001)"
        );
    }
}

#[test]
fn one_factory_builds_remote_webviews() {
    let factories = ["src/shell/content.rs", "src/shell/chrome.rs"];
    only_in("WebviewBuilder::new", &factories);
    only_in("on_navigation", &factories);
    only_in("add_child", &factories);
    only_in("proxy_url", &["src/shell/content.rs"]);
    only_in("WebviewUrl::External", &["src/shell/content.rs"]);
    only_in("WebviewWindowBuilder", &[]);
}

#[test]
fn chrome_webviews_load_only_bundled_pages() {
    let chrome = code(&root().join("src/shell/chrome.rs"));
    for line in chrome.iter().filter(|l| l.contains("WebviewBuilder::new")) {
        assert!(line.contains("WebviewUrl::App"), "{line}");
    }
}

#[test]
fn content_factory_applies_every_layer() {
    let content = code(&root().join("src/shell/content.rs")).join("\n");
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
        for file in users(token) {
            assert!(file.starts_with("src/net/"), "`{token}` in {file}");
        }
    }
}

#[test]
fn one_host_predicate() {
    only_in("fn is_i2p_host", &["src/net/host.rs"]);
    only_in("ends_with(\".i2p\")", &["src/net/host.rs"]);
    only_in("ends_with(\"i2p\")", &["src/net/host.rs"]);
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
    for file in rust_files(&root().join("src")) {
        for line in code(&file) {
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
fn capabilities() -> Vec<(String, serde_json::Value)> {
    let dir = root().join("capabilities");
    fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| {
            let text = fs::read_to_string(&p).unwrap();
            (rel(&p), serde_json::from_str(&text).unwrap())
        })
        .collect()
}

#[test]
fn capabilities_never_reach_tabs() {
    let allowed = ["toolbar", "internal", "status"];
    for (file, cap) in capabilities() {
        let webviews = cap["webviews"].as_array().unwrap();
        for label in webviews.iter().map(|v| v.as_str().unwrap()) {
            assert!(allowed.contains(&label), "{file} names webview {label}");
        }
        assert!(cap.get("windows").is_none(), "{file} grants by window");
        assert!(cap.get("remote").is_none(), "{file} grants remote URLs");
    }
}

#[test]
fn status_bubble_gets_events_only() {
    for (file, cap) in capabilities() {
        let names: Vec<&str> = cap["webviews"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        if names.contains(&"status") {
            let perms = cap["permissions"].as_array().unwrap();
            for p in perms.iter().map(|p| p.as_str().unwrap()) {
                assert!(p.starts_with("core:event:"), "{file}: status gets {p}");
            }
        }
    }
}
