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

// ---------------------------------------------------------------------------------------
// Diagnostics and bug reports (`docs/wiki/diagnostics-and-bug-reports.md`, R7 to R10).
// ---------------------------------------------------------------------------------------

/// The five report permissions of R8.1.
const REPORT_PERMISSIONS: [&str; 5] = [
    "allow-report-preview",
    "allow-report-open",
    "allow-diag-crash-status",
    "allow-diag-crash-dismiss",
    "allow-diag-logs-delete",
];

/// Every `.rs` file of the app crate and of the crates in `crates/*/src`.
fn all_rust_sources() -> io::Result<Vec<PathBuf>> {
    let mut out = rust_files(&root().join("src"))?;
    for entry in fs::read_dir(root().join("crates"))? {
        let src = entry?.path().join("src");
        if src.is_dir() {
            out.extend(rust_files(&src)?);
        }
    }
    Ok(out)
}

/// Every `.ts` file under `dir`, recursively.
fn ts_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            out.extend(ts_files(&path)?);
        } else if path.extension().is_some_and(|e| e == "ts") {
            out.push(path);
        }
    }
    Ok(out)
}

/// True when `line` holds `token` and the character before it is not part of a name.
fn has_token(line: &str, token: &str) -> bool {
    line.match_indices(token).any(|(at, _)| {
        line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
    })
}

// R9.1: only the diag module prints or logs.
#[test]
fn r9_1_only_the_diag_module_logs() {
    let tokens = [
        "println!",
        "eprintln!",
        "print!",
        "eprint!",
        "dbg!",
        "log::",
        "tracing::",
        "io::stdout",
        "io::stderr",
    ];
    for file in all_rust_sources().unwrap() {
        let name = rel(&file);
        if name.starts_with("src/diag/") {
            continue;
        }
        for line in code(&file).unwrap() {
            let hit = tokens.iter().find(|t| has_token(&line, t));
            assert!(
                hit.is_none(),
                "{hit:?} in {name}: {line} (only src/diag/ may log)"
            );
        }
    }
}

// R9.1: the scanner sees the tokens, and not look-alikes.
#[test]
fn r9_1_token_scan_finds_logging_calls() {
    assert!(has_token("    println!(\"x\");", "println!"));
    assert!(has_token("let _ = std::io::stderr();", "io::stderr"));
    assert!(has_token("log::info!(\"x\")", "log::"));
    assert!(!has_token("eprint!(\"x\")", "print!"));
    assert!(!has_token("changelog::x()", "log::"));
}

// R9.2: no `console.` in any `.ts` file under `src/ui/`, tests included.
#[test]
fn r9_2_no_console_in_ui_typescript() {
    let ui = root().join("../src/ui");
    for file in ts_files(&ui).unwrap() {
        let text = fs::read_to_string(&file).unwrap();
        let name = file
            .strip_prefix(&ui)
            .unwrap_or(&file)
            .display()
            .to_string();
        assert!(!has_token(&text, "console."), "console. in src/ui/{name}");
    }
}

/// The argument text of every call `diag::event(…)` in `text`. A call is `diag::event(`, or
/// a bare `event(Code::…` after `use … diag::event`.
fn event_calls(text: &str) -> Vec<String> {
    calls(text, "event")
}

/// The argument text of every call `diag::<name>(…)`, or a bare `<name>(Code::…` after a `use`.
fn calls(text: &str, name: &str) -> Vec<String> {
    let open = format!("{name}(");
    let mut out = Vec::new();
    for (at, _) in text.match_indices(&open) {
        let before = &text[..at];
        let rest = &text[at + open.len()..];
        let qualified = before.ends_with("diag::");
        let bare = !qualified
            && before
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace() || c == '(' || c == '{')
            && !before.trim_end().ends_with("fn")
            && rest.trim_start().starts_with("Code::");
        if qualified || bare {
            out.push(call_args(rest));
        }
    }
    out
}

/// The text up to the parenthesis that closes the call.
fn call_args(rest: &str) -> String {
    let mut depth = 1_usize;
    let mut end = rest.len();
    for (i, c) in rest.char_indices() {
        depth = match c {
            '(' => depth + 1,
            ')' => depth - 1,
            _ => depth,
        };
        if depth == 0 {
            end = i;
            break;
        }
    }
    rest[..end].to_owned()
}

/// The reason an argument text holds text, or `None` when it holds typed values only.
fn text_argument(args: &str) -> Option<&'static str> {
    [
        ("\"", "a string literal"),
        ("format!", "format!"),
        ("to_string", "to_string"),
        ("to_owned", "to_owned"),
        ("String", "String"),
    ]
    .into_iter()
    .find(|(token, _)| args.contains(token))
    .map(|(_, why)| why)
}

// R9.3: no call `diag::event(…)` has a string literal, `format!`, `to_string`, `to_owned` or
// `String` in its arguments.
#[test]
fn r9_3_no_text_argument_to_diag_event() {
    for file in all_rust_sources().unwrap() {
        let source = code(&file).unwrap().join("\n");
        for args in event_calls(&source) {
            let why = text_argument(&args);
            assert!(
                why.is_none(),
                "{} passes {why:?} to diag::event({args})",
                rel(&file)
            );
        }
    }
}

// R9.3: the scanner finds text arguments, also over several lines, and lets typed ones pass.
#[test]
fn r9_3_scan_finds_text_arguments() {
    let good = "diag::event(Code::Startup, &[Field::Line(1)]);";
    let bad = "diag::event(\n  Code::PageLoadFailed,\n  &[Field::Count(url.to_string())],\n);";
    let literal = "diag::event(Code::Startup, &[\"x\"]);";
    let bare = "use a::diag::event;\nevent(Code::Startup, &[format!(\"x\")]);";
    assert!(event_calls(good).iter().all(|a| text_argument(a).is_none()));
    assert!(event_calls(bad).iter().any(|a| text_argument(a).is_some()));
    assert!(
        event_calls(literal)
            .iter()
            .any(|a| text_argument(a).is_some())
    );
    assert!(event_calls(bare).iter().any(|a| text_argument(a).is_some()));
}

/// The capability file `capabilities/report.json`.
fn report_capability() -> Res<serde_json::Value> {
    let text = fs::read_to_string(root().join("capabilities/report.json"))?;
    Ok(serde_json::from_str(&text)?)
}

// R8.1: `capabilities/report.json` names only `internal`, and grants only the five permissions.
#[test]
fn r8_1_report_capability_is_internal_only_with_five_permissions() {
    let cap = report_capability().unwrap();
    assert_eq!(labels(&cap), ["internal"]);
    let mut granted = permissions(&cap);
    granted.sort_unstable();
    let mut expected = REPORT_PERMISSIONS.to_vec();
    expected.sort_unstable();
    assert_eq!(granted, expected);
}

// R8.2 and R9.4: no other capability names these permissions, so a `tab-*` webview and the
// toolbar and status webviews get none of them.
#[test]
fn r8_2_no_other_capability_names_the_report_permissions() {
    for (file, cap) in capabilities().unwrap() {
        if file == "capabilities/report.json" {
            continue;
        }
        for permission in permissions(&cap) {
            assert!(
                !REPORT_PERMISSIONS.contains(&permission),
                "{file} grants {permission}"
            );
        }
    }
}

// R8.2: the only capability with a report permission names only the internal webview, never a tab.
#[test]
fn r8_2_report_permissions_never_reach_tabs() {
    for (file, cap) in capabilities().unwrap() {
        let has = permissions(&cap)
            .iter()
            .any(|p| REPORT_PERMISSIONS.contains(p));
        if has {
            assert_eq!(labels(&cap), ["internal"], "{file}");
        }
        for label in labels(&cap) {
            assert!(!label.starts_with("tab"), "{file} names {label}");
        }
    }
}

// R8.3: no capability grants an `opener:` permission. JavaScript gets no opener command.
#[test]
fn r8_3_no_capability_grants_an_opener_permission() {
    for entry in fs::read_dir(root().join("capabilities")).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("opener"), "{} mentions opener", rel(&path));
    }
}

// R8.3: JavaScript has no opener API: no UI file imports or names the opener plugin.
#[test]
fn r8_3_ui_never_uses_the_opener_plugin() {
    let ui = root().join("../src/ui");
    for file in ts_files(&ui).unwrap() {
        let text = fs::read_to_string(&file).unwrap();
        for token in ["plugin-opener", "opener:", "plugin:opener"] {
            assert!(!text.contains(token), "{} names {token}", rel(&file));
        }
    }
}

// R8.3: the opener plugin starts with `open_js_links_on_click(false)`.
#[test]
fn r8_3_opener_plugin_does_not_open_js_links() {
    let mut uses_plugin = false;
    let mut disabled = false;
    for file in rust_files(&root().join("src")).unwrap() {
        let source = code(&file).unwrap().join("\n");
        uses_plugin |= source.contains("tauri_plugin_opener");
        disabled |= source.contains("open_js_links_on_click(false)");
    }
    assert!(
        uses_plugin,
        "the shell must use tauri-plugin-opener for the issue URL"
    );
    assert!(
        disabled,
        "the plugin must start with open_js_links_on_click(false)"
    );
}

// R8.4: only `src/shell/report.rs` calls `open_url` and `reveal_item_in_dir`.
#[test]
fn r8_4_only_report_rs_opens_urls_and_reveals_files() {
    only_in("open_url", &["src/shell/report.rs"]).unwrap();
    only_in("reveal_item_in_dir", &["src/shell/report.rs"]).unwrap();
}

// R7 + R8.4: `report.rs` is the caller, it checks the URL with `is_issue_url`, and it writes
// into the Downloads folder.
#[test]
fn r7_report_command_checks_the_url_and_the_folder() {
    let report = code(&root().join("src/shell/report.rs"))
        .unwrap()
        .join("\n");
    for token in ["open_url", "reveal_item_in_dir", "is_issue_url"] {
        assert!(
            report.contains(token),
            "src/shell/report.rs must use {token}"
        );
    }
    assert!(
        report.to_lowercase().contains("download"),
        "the report file goes to Downloads"
    );
}

// R7: every report command is registered, so the build makes its permission.
#[test]
fn r7_ipc_commands_are_registered() {
    let build = fs::read_to_string(root().join("build.rs")).unwrap();
    for command in [
        "report_preview",
        "report_open",
        "diag_crash_status",
        "diag_crash_dismiss",
        "diag_logs_delete",
    ] {
        assert!(
            build.contains(&format!("\"{command}\"")),
            "build.rs lacks {command}"
        );
    }
}

// R8.5 and R2.5: no HTTP client crate, and no socket in the diag module.
#[test]
fn r8_5_no_http_client_and_no_socket_in_diag() {
    let manifest = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    for krate in [
        "reqwest",
        "hyper",
        "ureq",
        "isahc",
        "curl",
        "attohttpc",
        "surf",
        "minreq",
    ] {
        assert!(
            !manifest.contains(&format!("{krate} =")),
            "Cargo.toml names {krate}"
        );
    }
    for file in rust_files(&root().join("src")).unwrap() {
        if !rel(&file).starts_with("src/diag/") {
            continue;
        }
        let source = code(&file).unwrap().join("\n");
        for token in [
            "TcpStream",
            "TcpListener",
            "UdpSocket",
            "ToSocketAddrs",
            "reqwest",
        ] {
            assert!(!source.contains(token), "{} uses {token}", rel(&file));
        }
    }
}

// R10: every code of the table is recorded somewhere outside the diag module.
#[test]
fn r10_every_event_code_is_recorded_by_the_app() {
    let codes = [
        "Startup",
        "Shutdown",
        "StartFailed",
        "VerifyPassed",
        "VerifyFailed",
        "RouterUp",
        "RouterDown",
        "GatekeeperRefused",
        "GatekeeperStartFailed",
        "PageLoadFailed",
        "WebviewCreateFailed",
        "EngineCallFailed",
        "FindFailed",
        "ZoomFailed",
        "StoreCorrupt",
        "ThreadFailed",
        "ReportOpened",
        "ReportFailed",
    ];
    let mut sources = String::new();
    for file in rust_files(&root().join("src")).unwrap() {
        if !rel(&file).starts_with("src/diag/") {
            sources.push_str(&code(&file).unwrap().join("\n"));
        }
    }
    for code_name in codes {
        assert!(
            sources.contains(&format!("Code::{code_name}")),
            "Code::{code_name} is never recorded outside src/diag/"
        );
    }
}

// R10: the report codes are recorded by the report command, the refusal by the gatekeeper.
#[test]
fn r10_events_sit_where_the_table_puts_them() {
    let report = code(&root().join("src/shell/report.rs"))
        .unwrap()
        .join("\n");
    for name in ["Code::ReportOpened", "Code::ReportFailed"] {
        assert!(
            report.contains(name),
            "src/shell/report.rs must record {name}"
        );
    }
    let mut refused_in_net = false;
    for file in rust_files(&root().join("src/net")).unwrap() {
        refused_in_net |= code(&file)
            .unwrap()
            .join("\n")
            .contains("Code::GatekeeperRefused");
    }
    assert!(
        refused_in_net,
        "the gatekeeper must record Code::GatekeeperRefused"
    );
}

// R2.6: `run()` installs the panic hook and calls `diag::init` with `default_log_dir`
// before the Tauri builder, so `StartFailed` and an early panic reach the disk.
#[test]
fn r2_6_diag_starts_before_the_tauri_builder() {
    let mut checked = false;
    for file in rust_files(&root().join("src")).unwrap() {
        let source = code(&file).unwrap().join("\n");
        let Some(builder) = source.find("tauri::Builder") else {
            continue;
        };
        checked = true;
        for call in ["install_panic_hook", "default_log_dir", "diag::init("] {
            let at = source.find(call);
            assert!(
                at.is_some_and(|at| at < builder),
                "{}: `{call}` must come before tauri::Builder",
                rel(&file)
            );
        }
    }
    assert!(checked, "no file builds the app with tauri::Builder");
}

// R9.3 + R12.2: no call `diag::count(…)` has text in its arguments either.
#[test]
fn r12_2_no_text_argument_to_diag_count() {
    for file in all_rust_sources().unwrap() {
        let source = code(&file).unwrap().join("\n");
        for args in calls(&source, "count") {
            let why = text_argument(&args);
            assert!(
                why.is_none(),
                "{} passes {why:?} to diag::count({args})",
                rel(&file)
            );
        }
    }
}

// R12.2: per-request results are counted, never recorded: `diag::event` is never called with
// `GatekeeperRefused` or `PageLoadFailed`.
#[test]
fn r12_2_per_request_codes_are_never_events() {
    for file in all_rust_sources().unwrap() {
        let source = code(&file).unwrap().join("\n");
        let bad = event_calls(&source)
            .into_iter()
            .find(|args| args.contains("GatekeeperRefused") || args.contains("PageLoadFailed"));
        assert!(
            bad.is_none(),
            "{} records a per-request result with diag::event({bad:?}); use diag::count",
            rel(&file)
        );
    }
}

// R12.2: the gatekeeper counts each refusal, each router 5xx answer and each failed forwarded request.
#[test]
fn r12_2_the_gatekeeper_counts_its_per_request_results() {
    let mut source = String::new();
    for file in rust_files(&root().join("src/net")).unwrap() {
        source.push_str(&code(&file).unwrap().join("\n"));
    }
    let counted = calls(&source, "count").join("\n");
    for (code_name, field) in [
        ("GatekeeperRefused", "Field::Refuse"),
        ("PageLoadFailed", "Field::Status"),
        ("PageLoadFailed", "Field::Error"),
    ] {
        let found = calls(&source, "count")
            .iter()
            .any(|args| args.contains(code_name) && args.contains(field));
        assert!(
            found,
            "src/net must call diag::count(Code::{code_name}, {field}(…)); saw: {counted}"
        );
    }
}

// R14.2: the report file is created with `create_new`, and `report_open` checks the Downloads
// folder after `canonicalize` (R14.3).
#[test]
fn r14_report_file_is_created_new_and_checked_after_canonicalize() {
    let report = code(&root().join("src/shell/report.rs"))
        .unwrap()
        .join("\n");
    assert!(
        report.contains("create_new"),
        "the report file must be created with create_new"
    );
    assert!(
        report.contains("canonicalize"),
        "report_open must canonicalize both paths"
    );
    assert!(
        report.contains("ReportFailed"),
        "a path outside Downloads records ReportFailed"
    );
}

// R14.2 (Unix): the log files and the crash marker are opened with `O_NOFOLLOW`, and
// R14.1: the modes 0600 and 0700 are set.
#[test]
fn r14_diag_opens_without_following_links_and_sets_modes() {
    let mut source = String::new();
    for file in rust_files(&root().join("src/diag")).unwrap() {
        source.push_str(&code(&file).unwrap().join("\n"));
    }
    assert!(
        source.contains("NOFOLLOW"),
        "src/diag must open with O_NOFOLLOW"
    );
    assert!(source.contains("0o600"), "files get mode 0600");
    assert!(source.contains("0o700"), "the folder gets mode 0700");
}

// R13.2: the report file name holds no date: `report.rs` does not build one from the time.
#[test]
fn r13_2_report_file_name_has_no_date() {
    let report = code(&root().join("src/shell/report.rs"))
        .unwrap()
        .join("\n");
    assert!(
        report.contains("file_name("),
        "report_open names the file with report::file_name"
    );
}
