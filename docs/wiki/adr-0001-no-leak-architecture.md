<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# ADR 0001: No-leak architecture

Status: accepted. Shipped in [#29](https://github.com/tcivie/eepview/pull/29). Amended in [#91](https://github.com/tcivie/eepview/pull/91): `CONNECT` ports, interim heads, TLS relay timeouts.

## Goal

A page in eepview can reach the I2P network and nothing else. This holds by construction, and CI fails if a guard is removed or weakened. No single mistake, refactor or removed line may open a clearnet or local path.

## Five independent layers

Each layer alone blocks a clearnet or local request. A leak needs all five to fail.

| # | Layer | Where | Blocks |
|---|---|---|---|
| L1 | **Gatekeeper proxy**: eepview's own HTTP proxy on `127.0.0.1:<random>`. Every content webview points at it, never at the router. It forwards a request only when its host passes `is_i2p_host` (lowercase, no trailing dot, no userinfo, IDN refused), and only to the verified router proxy. Everything else gets `403` with no upstream connection. `CONNECT` goes only to `*.i2p`. Port 443 is relayed, only on Windows (see below). Any other port 1–65535 is terminated and checked like plain HTTP: each request goes to the `CONNECT` host and port. | `src-tauri/src/net/gatekeeper.rs` | Clearnet through a router outproxy (so the outproxy never matters), IP literals, `localhost`, LAN |
| L2 | **Engine proxy**: `proxy_url` = the gatekeeper. On Windows `--proxy-server=<gatekeeper> --proxy-bypass-list=<-loopback> --force-webrtc-ip-handling-policy=disable_non_proxied_udp` are always set together. | `src-tauri/src/shell/content.rs` | Direct connections |
| L3a | **Page policy**: the gatekeeper adds a `Content-Security-Policy` to every response. Only the final response head reaches the engine: a 1xx interim head never does. Every fetch directive allows only `http(s)://*.i2p:*` (plus `data:` and `blob:` for images, fonts and media). | `src-tauri/src/net/rules.rs` | Loopback bypass of the proxy (spike S1), `fetch`, WebSocket, frames to IPs |
| L3b | **Engine request filter**: macOS `WKContentRuleList`; Windows a `WebResourceRequested` filter that answers 403. One rule set: block all, then allow `.i2p` and `about:`, `data:`, `blob:`. A content webview starts on `about:blank` and loads the page only after the filter is attached; if it cannot be attached, nothing loads. Linux: webkit2gtk has no binding for `WebKitUserContentFilter` yet; L3a holds there. | `src-tauri/src/shell/content.rs`, `src-tauri/src/net/rules.rs`, `src-tauri/crates/eepview-platform` | The same, inside the engine, even for a response without the policy |
| L4 | **Navigation guard**: `on_navigation` and the new-window handler allow only the same `.i2p` rule. | `content.rs`, `src-tauri/src/nav.rs` | Top-level navigation, popups, `file:`, `data:`, `javascript:`, custom schemes |
| L5 | **WebRTC off**: an init script in every frame removes the WebRTC constructors; WebKitGTK turns WebRTC and media capture off in its settings; Windows sets the UDP flag above. | `content.rs`, `eepview-platform` | UDP and STUN |

**JavaScript.** JS is on. The layers below JS hold with JS on, and the leak test runs with JS on. You can turn JS off per site, or for every site with `EEPVIEW_JS=off`.

One host predicate, `net::host::is_i2p_host`, is the single source of truth for L1, L3 and L4. It is pure and has a table test.

## Process isolation

Every engine runs page content in a separate, OS-sandboxed content process with no direct network access: `WKWebView` WebContent, `WebView2` renderer, WebKitGTK web process. All network traffic leaves through one network process. That network process is what the five layers confine. An OS-level layer that confines it from outside (L6) is on the [roadmap](roadmap.md).

**Gatekeeper answers** (owner decision). A non-`.i2p` host gets 403, a malformed request 400 (two `Content-Length` headers count as malformed), and a chunked request body 411. None of them opens an upstream connection, and the connection closes. An `.i2p` URL may carry an explicit port 1–65535 and is forwarded with it. When the client ends before the request body does, the gatekeeper closes both sides at once.

**`CONNECT` ports** (amended in [#91](https://github.com/tcivie/eepview/pull/91)). A `CONNECT` port must be 1–65535, in digits. Port 443 is the TLS relay. Every other port is plain HTTP, the same as port 80: the gatekeeper reads each request in the tunnel, checks it, and sends it to the `CONNECT` host and port only. A `Host` header that names another host or another port gets 403. macOS WebKit sends every `http://` load as `CONNECT host:port`, so without this rule an eepsite on another port does not load there. Port 0, a port over 65535 and a port that is not digits get 403.

**Interim heads** (amended in [#91](https://github.com/tcivie/eepview/pull/91)). The router can send 1xx interim heads before the final response head. Each one is unsolicited, because the gatekeeper removes `Expect` from the request. The gatekeeper discards each 1xx head and reads the next head. Only the final head gets the page policy and goes to the engine, then the body. A `101 Switching Protocols` gets the 502 refusal and is never relayed. More than 8 interim heads get the 502 refusal too. The same rule applies to the router's answer to a TLS `CONNECT`.

**Gatekeeper timing and close** (owner decision).

- **Dead router.** The connect to the router proxy times out after 500 ms. When the router is gone, the client gets the 502 within 1 s on every OS. Without this bound, Windows retries a refused loopback connect for about 2 s. The read and write timeouts on an open upstream connection do not change.
- **TLS relay** (amended in [#91](https://github.com/tcivie/eepview/pull/91)). The 60 s head timeout and the 5 min write timeout on the engine side apply until the relay starts. Then the engine side has no timeout: a long download, a long poll or a silent engine does not end the tunnel. The router side keeps its 5 min timeouts, so a router that hangs cannot hold a connection forever.
- **Connection limit.** The gatekeeper handles at most 256 connections at once. A connection over that limit gets `503 Service Unavailable` before its request is read, and closes like the other early answers below. At most 256 of these busy answers drain at once; beyond that a busy answer closes without the drain.
- **Early answers reach the client.** The gatekeeper can answer before it has read the whole request: the busy 503, and a 403, 400 or 411 sent while request bytes are still unread. After such an answer it closes in this order:
  1. It shuts down its write side, so the client sees the end of the answer.
  2. It reads and discards the unread input, until the client closes, 1 s passes, or 64 KiB have been read, whichever comes first.
  3. It closes the socket.

  If unread input is still in the buffer when the socket closes, the OS sends a reset in place of a normal close. On Windows the reset makes the client drop the answer it already received. With this order, the client gets the full answer, and one connection costs at most 1 s and 64 KiB.

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
   - a capability names a webview other than `toolbar`, `internal` or `status`, grants by window or remote URL, or gives `status` more than events;
   - the router console view breaks its own rules (see [Router console exception](#router-console-exception)).

   The router console view is the one other remote webview. The test allows it only in `src/shell/console.rs`: `WebviewBuilder::new`, `on_navigation` and `add_child` (with `content.rs` and `chrome.rs`), `WebviewUrl::External` (with `content.rs`, and only for `about:blank`), and no window builder at all (`WindowBuilder` and `WebviewWindowBuilder` are banned there: the console view is a child of the main window, shown by the console tab). `proxy_url` stays in `content.rs` only.
5. **Fail closed at runtime.** The gatekeeper runs only while VERIFY passes. VERIFY asks the router proxy for `http://proxy.i2p/` every 5 s and needs 200 with "I2P HTTP proxy OK". It never asks for a non-`.i2p` host: on a router with an outproxy, that request would itself reach the clearnet. The architecture test fails if `verify.rs` or `gatekeeper.rs` names a non-`.i2p` host. When the router goes down, or you pause, the gatekeeper closes and every `tab-*` webview is destroyed.

## Router console exception

Amended in [#54](https://github.com/tcivie/eepview/pull/54). eepview shows router information but never changes the router configuration. It opens the router's own console pages instead, in one `console` webview. That view loads a loopback page, so it is an exception to the goal above. It is confined like this:

- **Sealed input.** `ConsoleWebview::open(&VerifiedConsole, …)` in `src/shell/console.rs` is the only constructor. Only the detector in `src/net/console.rs` makes a `VerifiedConsole`, after one loopback `GET` shows that console's marker.
- **One origin.** The view loads only `http://127.0.0.1:<detected port>`. It starts on `about:blank` and loads the page only after an engine rule list is attached that allows only that origin and `about:`, `data:`, `blob:` (macOS and Windows; fail closed). Linux has no engine filter yet, the same limit as L3b for tabs.
- **Navigation guard.** The console origin stays. An `http(s)://*.i2p` link opens in a normal tab through the tab guard (L4). Anything else is cancelled. New windows are never engine windows.
- **No IPC, no proxy, WebRTC off.** No capability names `console`. It has no `proxy_url` and never sees the gatekeeper. WebRTC is removed in every frame, downloads are refused, and it runs incognito.
- **Tabs unchanged.** The `tab-*` webviews keep all five layers. A `tab-*` webview never gets a loopback URL.
- **No probe at start.** Detection runs only when a page that shows the console links opens, so the leak test sees no extra socket.

See [Router console](router-console.md) for the requirements (R1–R30). The console view shows in the console tab since [#76](https://github.com/tcivie/eepview/pull/76); the confinement above is unchanged.

## The platform bridge

`src-tauri/crates/eepview-platform` is the only crate with `unsafe`. It wraps the engine APIs that Tauri does not expose: the rule list, the link under the mouse, native back, forward, stop and find, and engine hardening (macOS fraud-check lookups off; Windows autofill, password saving and SmartScreen off). Its lints deny `unsafe_op_in_unsafe_fn`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block` and `missing_safety_doc`. Every `unsafe` block holds one call and a `// SAFETY:` line. The app crate keeps `unsafe_code = "deny"` and calls only safe functions. The plan is to upstream each function to wry or Tauri and delete our copy.

## Implementation notes

- **TLS tunnels open only on Windows.** A relayed `CONNECT *.i2p:443` response cannot carry the page policy (L3a). Windows has the engine filter (L3b), so tunnels open there. Linux has no engine filter yet, and `WKWebView` skips the proxy for loopback, so tunnels stay closed on Linux and macOS (fail closed). macOS could open them later, now that its rule list is in place.
- **Accept errors back off** (50 ms doubling to 1 s), so a persistent error such as no free file descriptors costs no CPU.
- **The blocked page replaces a tab only on a click.** The engines report frame navigations like top-level ones, with no frame flag. A refused navigation to the link under the mouse shows the blocked page; any other refused navigation (a frame, a script, a redirect) only shows a toast. Page events from a hidden web view are ignored while the tab shows an internal page.
- The `internal` webview's `on_navigation` lives in `chrome.rs`: it keeps bundled pages inside the bundle and sends any other URL through `navigate`.
- **App Transport Security is off for web content only.** The macOS bundle sets `NSAllowsArbitraryLoadsInWebContent`, because ATS refuses every `http://` page load and eepsites are `http://`. This weakens no layer: L2 still sends every content request to the gatekeeper on loopback, and L1 still forwards only `.i2p` hosts. The Rust code uses plain sockets, which ATS never covers. See [Browser shell](browser-shell.md) (B1).
- `EEPVIEW_PROXY`, `EEPVIEW_START_URL`, `EEPVIEW_EXIT_AFTER` and `EEPVIEW_JS` exist in the release binary, so the leak test runs the real binary. They cannot weaken a layer: the upstream still has to pass VERIFY, and the gatekeeper still enforces `.i2p`.

## Leak test

A permanent cross-OS leak test runs the real binary against a fake I2P proxy and loopback, LAN and UDP canaries. See [Leak test](leak-test.md).

## Out of scope

OS-level network isolation of the network process (sandbox profiles, network namespaces, firewall rules). See L6 on the [roadmap](roadmap.md).
