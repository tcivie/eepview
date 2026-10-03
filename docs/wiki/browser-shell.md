# Browser shell

Status: shipped in [#29](https://github.com/tcivie/eepview/pull/29).

The Rust side of the browser, in `src-tauri/`: tabs, navigation, bookmarks, history, find in page, zoom, the router watcher and the gatekeeper proxy. The UI talks to it through the [IPC contract](ipc-contract.md).

## How it works

- One window holds several webviews: `toolbar`, `internal` (eepview pages), `status` (the link bubble) and one `tab-*` webview per web tab. See the [IPC contract](ipc-contract.md).
- `core/` is a pure state machine. Each command returns effects (emit an event, load a tab, call the engine). The shell carries them out on the main thread and never holds the core lock while it calls a webview.
- Every 5 s the router watcher runs VERIFY on the router proxy: `http://proxy.i2p/` must answer "I2P HTTP proxy OK". Only then does the gatekeeper run and web tabs load. VERIFY never asks for a clearnet host. A router outproxy is harmless: the gatekeeper forwards only `.i2p` hosts.
- Back, forward, stop, hard reload and find use the engine's native API, so they work with page JavaScript off. Reload and zoom use Tauri.
- New windows (`target=_blank`, `window.open`) open as new tabs. Downloads are refused with a toast.
- Pause closes the gatekeeper and every web tab. Resume runs VERIFY again.
- Stores are JSON files with a `version` field, written atomically: bookmarks, history (at most 10 000 entries, off when `history.enabled` is false), settings, and per-site zoom and JavaScript.
- Security: see [No-leak architecture](no-leak-architecture.md) and [ADR 0001](adr-0001-no-leak-architecture.md).

## Requirements (owner decisions)

- Only an `http(s)` URL on a `.i2p` host enters tab state or reaches an engine. `Core::load` is the one place a load is made, and it checks the rule again, so no other path can replay a clearnet URL.
- Address bar: blank input is ignored; the host is lower-cased and one trailing dot dropped; one word with no dot, no colon and no scheme searches; anything else that is not on `*.i2p` is `not-i2p`; unparseable input is `invalid`.
- `.i2p` URLs may carry an explicit port 1–65535 (http and https). The gatekeeper forwards them; `CONNECT` stays limited to `:80` and `:443`.
- Back and forward follow the standard per-tab session history: a new navigation clears the forward list, and back and forward never leave the tab's own history.
- `bookmark_add` refuses a non-I2P URL with `not-i2p`.
- Hand-edited store files are not trusted: a settings homepage that is not an I2P site or an internal page falls back to `eepview://home`; history keeps only I2P entries, newest first, at most 10 000.
- Gatekeeper: duplicate `Content-Length` headers are refused with 400; a request body cut short closes both sides at once.

## How to use / run locally

- `npm run tauri dev` with an I2P router on `127.0.0.1:4444`.
- `EEPVIEW_PROXY=127.0.0.1:<port>` picks another router proxy. `EEPVIEW_START_URL`, `EEPVIEW_EXIT_AFTER`, and `EEPVIEW_JS=off` are listed in the [IPC contract](ipc-contract.md#environment-and-command-line).
- Tests: `cargo test --workspace` in `src-tauri`.

## Limits

- Find on Windows uses an app-injected script (`WebView2` has no native find with a count).
- The Linux engine filter (L3b) waits for a webkit2gtk binding; the page policy (L3a) holds there.
- HTTPS eepsites load only on Windows: TLS tunnels stay closed on macOS and Linux (ADR 0001).
- The first-run setup flow and router control are Phase 2 and Phase 3.

## History

- 2026-10-03 — Browser shell: tabs, navigation, bookmarks, history, find, gatekeeper, pause and resume — [#29](https://github.com/tcivie/eepview/pull/29)
