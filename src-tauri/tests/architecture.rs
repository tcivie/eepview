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
    let allowed = ["toolbar", "internal", "status", "popup"];
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

/// What the `popup` webview may do: events, and the commands of its four popups.
const POPUP_PERMISSIONS: [&str; 15] = [
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "allow-popup-size",
    "allow-popup-close",
    "allow-navigate",
    "allow-tab-new",
    "allow-tab-list",
    "allow-zoom-in",
    "allow-zoom-out",
    "allow-zoom-reset",
    "allow-router-status",
    "allow-router-stats",
    "allow-connection-pause",
    "allow-connection-resume",
    "allow-router-control",
];

#[test]
fn popup_gets_only_its_commands() {
    let caps = capabilities().unwrap();
    for (file, cap) in caps.iter().filter(|(_, c)| labels(c).contains(&"popup")) {
        assert_eq!(labels(cap), ["popup"], "{file}: popup shares a capability");
        let perms = permissions(cap);
        let bad: Vec<&&str> = perms
            .iter()
            .filter(|p| !POPUP_PERMISSIONS.contains(p))
            .collect();
        assert!(bad.is_empty(), "{file}: popup gets {bad:?}");
    }
}

#[test]
fn only_the_toolbar_opens_popups() {
    for (file, cap) in capabilities().unwrap() {
        if permissions(&cap).contains(&"allow-popup-open") {
            assert_eq!(labels(&cap), ["toolbar"], "{file}");
        }
    }
}

#[test]
fn the_popup_webview_is_bundled_and_transparent() {
    let chrome = code(&root().join("src/shell/chrome.rs"))
        .unwrap()
        .join("\n");
    let at = chrome
        .find("WebviewBuilder::new(\"popup\"")
        .expect("popup webview");
    let builder = &chrome[at..chrome[at..].find(';').map_or(chrome.len(), |e| at + e)];
    for call in [
        "WebviewUrl::App(POPUP_PAGE",
        ".transparent(true)",
        ".on_navigation(",
    ] {
        assert!(builder.contains(call), "the popup webview must use {call}");
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

#[test]
fn commands_that_take_the_app_are_async() {
    // A plain `fn` command runs on the main thread. Every command that takes the app reaches
    // the core, a store, a lock or the router, so it must be `async` and work off that thread.
    let lines = code(&root().join("src/shell/commands.rs")).unwrap();
    let mut checked = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "#[tauri::command]" {
            continue;
        }
        let sig: String = lines[i + 1..].iter().take(3).cloned().collect();
        if sig.contains("AppHandle") {
            assert!(sig.starts_with("pub async fn"), "sync command: {sig}");
        }
        checked += 1;
    }
    assert!(checked > 30, "found only {checked} commands");
}

// Site icons (docs/wiki/site-icons.md, R35 to R38). The implementer must not edit these.

/// The `[dependencies]` section of `Cargo.toml`.
fn dependencies() -> io::Result<String> {
    let manifest = fs::read_to_string(root().join("Cargo.toml"))?;
    let start = manifest.find("\n[dependencies]").unwrap_or(0);
    let rest = &manifest[start + 1..];
    let end = rest[1..].find("\n[").map_or(rest.len(), |i| i + 1);
    Ok(rest[..end].to_owned())
}

#[test]
fn r37_net_icons_connects_only_through_the_gatekeeper_it_receives() {
    let lines = code(&root().join("src/net/icons.rs")).unwrap();
    let text = lines.join("\n");
    for token in [
        "LoopbackAddr::",
        "TcpStream",
        "SocketAddr",
        "ToSocketAddrs",
        "VerifiedUpstream",
        "verify::",
        "UdpSocket",
    ] {
        assert!(
            !text.contains(token),
            "net/icons.rs must not use `{token}` (R37)"
        );
    }
    assert!(
        text.contains("Gatekeeper"),
        "net/icons.rs takes a Gatekeeper (R37)"
    );
    for literal in literals(&lines) {
        let lower = literal.to_ascii_lowercase();
        for loopback in ["localhost", "127.", "::1", "0.0.0.0"] {
            assert!(
                !lower.contains(loopback),
                "net/icons.rs names {loopback} in \"{literal}\""
            );
        }
        // The path `/favicon.ico` is the one dotted word that is not a host.
        let hosts: Vec<String> = foreign_hosts(&literal)
            .into_iter()
            .filter(|w| w != "favicon.ico")
            .collect();
        assert!(
            hosts.is_empty(),
            "net/icons.rs names {hosts:?} in \"{literal}\" (R37)"
        );
    }
}

#[test]
fn r37_the_gatekeeper_address_is_crate_private() {
    let text = code(&root().join("src/net/gatekeeper.rs"))
        .unwrap()
        .join("\n");
    assert!(text.contains("pub(crate) fn addr(&self) -> LoopbackAddr"));
    assert!(
        !text.contains("pub fn addr"),
        "Gatekeeper::addr must not be public (R37)"
    );
}

#[test]
fn r37_only_the_shell_runs_icon_fetches_and_the_core_and_sanitizer_stay_pure() {
    for file in users("net::icons").unwrap() {
        let allowed = file.starts_with("src/shell/") || file.starts_with("src/net/");
        assert!(
            allowed,
            "{file} names net::icons; only src/shell/ and src/net/ may (R37)"
        );
    }
    for file in rust_files(&root().join("src/core")).unwrap() {
        let text = code(&file).unwrap().join("\n");
        assert!(
            !text.contains("Gatekeeper"),
            "{} names the Gatekeeper (R37)",
            rel(&file)
        );
    }
    let sanitizer = code(&root().join("src/icons.rs")).unwrap().join("\n");
    assert!(
        !sanitizer.contains("std::net"),
        "src/icons.rs opens no socket (R37)"
    );
}

#[test]
fn r37_the_icon_feature_adds_no_http_client_crate() {
    let deps = dependencies().unwrap();
    for name in [
        "reqwest",
        "hyper",
        "ureq",
        "isahc",
        "curl",
        "attohttpc",
        "surf",
        "minreq",
        "tungstenite",
    ] {
        let used = deps.lines().any(|l| {
            l.trim_start().starts_with(&format!("{name} "))
                || l.trim_start().starts_with(&format!("{name}="))
        });
        assert!(!used, "Cargo.toml [dependencies] names {name} (R37)");
    }
}

#[test]
fn r38_only_the_sanitizer_decodes_images() {
    let only = ["src/icons.rs"];
    for token in [
        "image::",
        "ImageReader",
        "load_from_memory",
        "png::Decoder",
        "gif::Decoder",
        "jpeg_decoder",
        "zune_jpeg",
        "webp::",
        "lodepng",
        "resvg",
        "usvg",
    ] {
        only_in(token, &only).unwrap();
    }
    let sanitizer = users("image::").unwrap();
    assert_eq!(sanitizer, only, "the sanitizer decodes images (R38)");
}

#[test]
fn r38_no_other_image_decoder_is_a_direct_dependency() {
    let deps = dependencies().unwrap();
    for name in [
        "png",
        "gif",
        "jpeg-decoder",
        "zune-jpeg",
        "webp",
        "lodepng",
        "resvg",
        "usvg",
        "ico",
        "tiff",
    ] {
        let used = deps.lines().any(|l| {
            l.trim_start().starts_with(&format!("{name} "))
                || l.trim_start().starts_with(&format!("{name}="))
        });
        assert!(
            !used,
            "Cargo.toml [dependencies] names {name}; only `image` decodes (R38)"
        );
    }
}

#[test]
fn r35_the_content_security_policy_loads_images_only_from_the_bundle_and_data_urls() {
    let conf: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join("tauri.conf.json")).unwrap()).unwrap();
    let csp = conf["app"]["security"]["csp"].as_str().unwrap_or_default();
    let img = csp
        .split(';')
        .map(str::trim)
        .find(|d| d.starts_with("img-src"))
        .unwrap_or_else(|| panic!("the CSP has no img-src: {csp}"));
    let sources: Vec<&str> = img.split_whitespace().skip(1).collect();
    assert!(
        sources.contains(&"'self'") && sources.contains(&"data:"),
        "{img}"
    );
    for source in sources {
        assert!(
            ["'self'", "data:"].contains(&source),
            "img-src allows {source} (R35)"
        );
    }
}

#[test]
fn r36_the_icon_feature_adds_no_command_and_no_capability() {
    for entry in fs::read_dir(root().join("capabilities")).unwrap() {
        let text = fs::read_to_string(entry.unwrap().path())
            .unwrap()
            .to_ascii_lowercase();
        assert!(!text.contains("icon"), "a capability names an icon (R36)");
    }
    let commands = code(&root().join("src/shell/commands.rs")).unwrap();
    let marked = commands
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("#[tauri::command]"));
    for (at, _) in marked {
        let name = commands
            .iter()
            .skip(at)
            .find(|l| l.contains("fn "))
            .cloned();
        let name = name.unwrap_or_default();
        assert!(
            !name.to_ascii_lowercase().contains("icon"),
            "icon command: {name} (R36)"
        );
    }
    for file in users("generate_handler").unwrap() {
        let text = code(&root().join(&file))
            .unwrap()
            .join("\n")
            .to_ascii_lowercase();
        let open = text
            .find("generate_handler![")
            .map(|i| i + "generate_handler![".len());
        let list = open.and_then(|from| text[from..].find(']').map(|len| &text[from..from + len]));
        let list = list.unwrap_or_else(|| panic!("{file}: no handler list found"));
        assert!(
            !list.contains("icon"),
            "{file}: the handler list names an icon (R36)"
        );
    }
}

#[test]
fn r34_the_ipc_contract_declares_the_icons_changed_event_and_the_icon_fields() {
    let contract = fs::read_to_string(root().join("../src/ui/contract.ts")).unwrap();
    assert!(contract.contains("\"icons-changed\": null;"), "R34");
    for name in ["TabInfo", "Bookmark", "HistoryEntry"] {
        let start = contract
            .find(&format!("export type {name} = {{"))
            .unwrap_or_else(|| panic!("{name} is not declared"));
        let end = contract[start..]
            .find("\n};")
            .map_or(contract.len(), |i| start + i);
        assert!(
            contract[start..end].contains("icon: string | null;"),
            "{name} has no icon (R31)"
        );
    }
}
