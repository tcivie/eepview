<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# ADR 0001: No-leak architecture

Status: accepted. Shipped in [#29](https://github.com/tcivie/eepview/pull/29).

## Goal

A page in eepview can reach the I2P network and nothing else. This holds by construction, and CI fails if a guard is removed or weakened. No single mistake, refactor or removed line may open a clearnet or local path.

## Five independent layers

Each layer alone blocks a clearnet or local request. A leak needs all five to fail.

| # | Layer | Where | Blocks |
|---|---|---|---|
| L1 | **Gatekeeper proxy**: eepview's own HTTP proxy on `127.0.0.1:<random>`. Every content webview points at it, never at the router. It forwards a request only when its host passes `is_i2p_host` (lowercase, no trailing dot, no userinfo, IDN refused), and only to the verified router proxy. Everything else gets `403` with no upstream connection. `CONNECT` goes only to `*.i2p:80` (terminated and checked like plain HTTP) or `*.i2p:443` (relayed; closed on macOS, see below). | `src-tauri/src/net/gatekeeper.rs` | Clearnet through a router outproxy, IP literals, `localhost`, LAN |
| L2 | **Engine proxy**: `proxy_url` = the gatekeeper. On Windows `--proxy-server=<gatekeeper> --proxy-bypass-list=<-loopback> --force-webrtc-ip-handling-policy=disable_non_proxied_udp` are always set together. | `src-tauri/src/shell/content.rs` | Direct connections |
| L3a | **Page policy**: the gatekeeper adds a `Content-Security-Policy` to every response. Every fetch directive allows only `http(s)://*.i2p:*` (plus `data:` and `blob:` for images, fonts and media). | `src-tauri/src/net/rules.rs` | Loopback bypass of the proxy (spike S1), `fetch`, WebSocket, frames to IPs |
| L3b | **Engine request filter**: macOS `WKContentRuleList`; Windows a `WebResourceRequested` filter that answers 403. One rule set: block all, then allow `.i2p` and `about:`, `data:`, `blob:`. A content webview starts on `about:blank` and loads the page only after the filter is attached; if it cannot be attached, nothing loads. Linux: webkit2gtk has no binding for `WebKitUserContentFilter` yet; L3a holds there. | `src-tauri/src/shell/content.rs`, `src-tauri/src/net/rules.rs`, `src-tauri/crates/eepview-platform` | The same, inside the engine, even for a response without the policy |
| L4 | **Navigation guard**: `on_navigation` and the new-window handler allow only the same `.i2p` rule. | `content.rs`, `src-tauri/src/nav.rs` | Top-level navigation, popups, `file:`, `data:`, `javascript:`, custom schemes |
| L5 | **WebRTC off**: an init script in every frame removes the WebRTC constructors; WebKitGTK turns WebRTC and media capture off in its settings; Windows sets the UDP flag above. | `content.rs`, `eepview-platform` | UDP and STUN |

**JavaScript.** JS is on. The layers below JS hold with JS on, and the leak test runs with JS on. You can turn JS off per site, or for every site with `EEPVIEW_JS=off`.

One host predicate, `net::host::is_i2p_host`, is the single source of truth for L1, L3 and L4. It is pure and has a table test.

## Process isolation

Every engine runs page content in a separate, OS-sandboxed content process with no direct network access: `WKWebView` WebContent, `WebView2` renderer, WebKitGTK web process. All network traffic leaves through one network process. That network process is what the five layers confine. An OS-level layer that confines it from outside (L6) is on the [roadmap](roadmap.md).

## Construction rules

These make the wrong code hard to write.

1. **Sealed types.** `VerifiedUpstream` (the router proxy after VERIFY) and `Gatekeeper` have private fields. Only `verify` makes a `VerifiedUpstream`, and only `Gatekeeper::start(&VerifiedUpstream)` makes a gatekeeper. `ContentWebview::create(&Gatekeeper, …)` is the only function that builds a remote webview. No gatekeeper means no content webview, at compile time.
2. **Loopback-only sockets.** `LoopbackAddr` holds only `127.0.0.0/8` or `::1`. The gatekeeper, VERIFY and the stats client connect only through it. No module outside `net/` names a socket type.
3. **No HTTP client crates.** `deny.toml` bans `reqwest`, `hyper`, `ureq`, `isahc`, `curl`, `attohttpc`, `surf`, `minreq`, `tungstenite`, `tokio-tungstenite`.
4. **Architecture test** (`src-tauri/tests/architecture.rs`, a normal `cargo test`). It fails when:
   - `WebviewBuilder::new`, `on_navigation` or `add_child` appear outside `shell/content.rs` and `shell/chrome.rs`; `proxy_url` or `WebviewUrl::External` outside `content.rs`; `chrome.rs` builds a webview with anything but `WebviewUrl::App`;
   - a socket type appears outside `net/`;
   - `unsafe` appears in the app crate (it lives only in `crates/eepview-platform`, whose lints are checked too);
   - `content.rs` stops calling a layer: `gatekeeper.url()`, `proxy_url`, `windows_proxy_args`, `rules::content_rule_list`, `rules::engine_allows`, `attach_rules`, `nav::guard`, `webrtc_off`, the `about:blank` first load;
   - a capability names a webview other than `toolbar`, `internal` or `status`, grants by window or remote URL, or gives `status` more than events.
5. **Fail closed at runtime.** The gatekeeper runs only while VERIFY passes (every 5 s). When the router goes down, or you pause, the gatekeeper closes and every `tab-*` webview is destroyed.

## The platform bridge

`src-tauri/crates/eepview-platform` is the only crate with `unsafe`. It wraps the engine APIs that Tauri does not expose: the rule list, the link under the mouse, native back, forward, stop and find, and engine hardening (macOS fraud-check lookups off; Windows autofill, password saving and SmartScreen off). Its lints deny `unsafe_op_in_unsafe_fn`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block` and `missing_safety_doc`. Every `unsafe` block holds one call and a `// SAFETY:` line. The app crate keeps `unsafe_code = "deny"` and calls only safe functions. The plan is to upstream each function to wry or Tauri and delete our copy.

## Implementation notes

- **TLS tunnels are closed on macOS.** A `CONNECT *.i2p:443` response cannot carry the page policy (L3a), and `WKWebView` skips the proxy for loopback. With the rule list (L3b) in place this could open later.
- The `internal` webview's `on_navigation` lives in `chrome.rs`: it keeps bundled pages inside the bundle and sends any other URL through `navigate`.
- `EEPVIEW_PROXY`, `EEPVIEW_START_URL`, `EEPVIEW_EXIT_AFTER` and `EEPVIEW_JS` exist in the release binary, so the leak test runs the real binary. They cannot weaken a layer: the upstream still has to pass VERIFY, and the gatekeeper still enforces `.i2p`.

## Leak test

A permanent cross-OS leak test runs the real binary against a fake I2P proxy and loopback, LAN and UDP canaries. See [Leak test](leak-test.md).

## Out of scope

OS-level network isolation of the network process (sandbox profiles, network namespaces, firewall rules). See L6 on the [roadmap](roadmap.md).
