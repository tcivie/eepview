# No-leak architecture

Status: shipped in [#29](https://github.com/tcivie/eepview/pull/29).

Five independent layers keep the web engine away from the clearnet and from local services. A leak needs all five to fail. The full decision is [ADR 0001](adr-0001-no-leak-architecture.md).

## How it works

| Layer | What it does |
|---|---|
| L1 gatekeeper | eepview's own proxy on loopback. It forwards only `.i2p` requests, and only to the verified router proxy. |
| L2 engine proxy | Every web tab uses the gatekeeper as its proxy. |
| L3 request filter | A page policy on every response (L3a), and an engine rule list attached before the first load (L3b, macOS and Windows). |
| L4 navigation guard | Only `http(s)://*.i2p` may load in a tab or open a new one. |
| L5 WebRTC off | WebRTC sends UDP outside the proxy, so it is removed in every frame. |

The router console view is not a leak path for eepsites:

| View | Why it is not a leak path |
|---|---|
| `console` (router console) | It loads only the detected console origin, `http://127.0.0.1:<port>`, after an engine rule list that allows only that origin (macOS, Windows). An eepsite can never load in it: an `.i2p` link opens in a normal tab through L4, and anything else is cancelled. No `tab-*` webview can reach it, because the tabs keep L1–L5 and loopback stays blocked for them. It has no IPC, WebRTC is off, and no probe runs at start. See [Router console](router-console.md) and [ADR 0001](adr-0001-no-leak-architecture.md#router-console-exception). |

### The one clearnet action: Report a problem

| Path | Why it is not a leak |
|---|---|
| "Open a GitHub issue" on `eepview://report` | It runs only after the user's click. eepview opens no socket: the system browser opens the page, outside eepview and outside I2P. The URL always starts with the fixed prefix `https://github.com/tcivie/eepview/issues/new`, and its text is the scrubbed preview the user just read. Only the `internal` webview may call it, and JavaScript gets no opener permission. See [Diagnostics and bug reports](diagnostics-and-bug-reports.md). |

JavaScript is on. The layers sit below JavaScript, so they hold with it on. You can turn it off per site.

## How to use / run locally

- `cargo test --workspace` in `src-tauri` runs the architecture test (`tests/architecture.rs`), which fails if a layer is removed.
- The [leak test](leak-test.md) runs the real binary against canaries.

## Limits

- No OS-level layer yet (L6, see the [roadmap](roadmap.md)).
- Linux has no engine rule list yet (L3b); the page policy covers it. The console view has no page policy, so on Linux it relies on its navigation guard and the router's own pages.

## History

- 2026-10-03 — The report path, the only clearnet action, after a click — [#56](https://github.com/tcivie/eepview/pull/56)
- 2026-10-03 — Five layers, the platform bridge and the architecture test — [#29](https://github.com/tcivie/eepview/pull/29)
- 2026-10-03 — Router console view: not a leak path for eepsites — [#54](https://github.com/tcivie/eepview/pull/54)
