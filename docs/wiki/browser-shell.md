# Browser shell

Status: shipped in [#29](https://github.com/tcivie/eepview/pull/29).

The Rust side of the browser, in `src-tauri/`: tabs, navigation, bookmarks, history, find in page, zoom, the router watcher and the gatekeeper proxy. The UI talks to it through the [IPC contract](ipc-contract.md).

## How it works

- One window holds several webviews: `toolbar`, `internal` (eepview pages), `status` (the link bubble) and one `tab-*` webview per web tab. See the [IPC contract](ipc-contract.md).
- `core/` is a pure state machine. Each command returns effects (emit an event, load a tab, call the engine). The shell carries them out on the main thread and never holds the core lock while it calls a webview.
- Every 5 s the router watcher runs VERIFY on the router proxy: `http://proxy.i2p/` must answer "I2P HTTP proxy OK", and `http://example.com/` must fail with a 5xx (no outproxy). Only then does the gatekeeper run and web tabs load.
- Back, forward, stop, hard reload and find use the engine's native API, so they work with page JavaScript off. Reload and zoom use Tauri.
- New windows (`target=_blank`, `window.open`) open as new tabs. Downloads are refused with a toast.
- Pause closes the gatekeeper and every web tab. Resume runs VERIFY again.
- Stores are JSON files with a `version` field, written atomically: bookmarks, history (at most 10 000 entries, off when `history.enabled` is false), settings, and per-site zoom and JavaScript.
- Security: see [No-leak architecture](no-leak-architecture.md) and [ADR 0001](adr-0001-no-leak-architecture.md).

## How to use / run locally

- `npm run tauri dev` with an I2P router on `127.0.0.1:4444` that has no outproxy.
- `EEPVIEW_PROXY=127.0.0.1:<port>` picks another router proxy. `EEPVIEW_START_URL`, `EEPVIEW_EXIT_AFTER`, `EEPVIEW_JS=off` and `EEPVIEW_LOG=1` are listed in the [IPC contract](ipc-contract.md#environment-and-command-line).
- Tests: `cargo test --workspace` in `src-tauri`.

## Limits

- Find on Windows uses an app-injected script (`WebView2` has no native find with a count).
- The Linux engine filter (L3b) waits for a webkit2gtk binding; the page policy (L3a) holds there.
- HTTPS eepsites do not load on macOS: TLS tunnels stay closed there (ADR 0001).
- The first-run setup flow and router control are Phase 2 and Phase 3.

## History

- 2026-10-03 — Browser shell: tabs, navigation, bookmarks, history, find, gatekeeper, pause and resume — [#29](https://github.com/tcivie/eepview/pull/29)
