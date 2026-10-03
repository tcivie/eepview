<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# IPC contract

Version 1.2. The Rust shell (`src-tauri/`) and the UI (`src/ui/`) build against this page.
Change it in a PR that changes both sides, or keep the old form working as a shim.

## History

- v1: first contract.
- v1.1: argument names, `chrome_set_height`, `platform`, `router_stats`, `bookmarks_export_file`, history cursors, `link-hover`, `fullscreen-changed`, `toast`.
- v1.2: `connection_pause`, `connection_resume`, `router_control`, `RouterStatus.paused` and `.managed`, `RouterStats.history`.
- Shipped in [#29](https://github.com/tcivie/eepview/pull/29).

## Window layout

One OS window with several webviews (Tauri `unstable` multi-webview).

| Webview label | Loads | IPC | Notes |
|---|---|---|---|
| `toolbar` | `src/ui/toolbar.html` (bundled) | yes | Top strip, 84 px: tab row (44 px) and nav row. 124 px with the find bar open. It sits above the content in z-order. |
| `internal` | `src/ui/<page>.html` (bundled) | yes | Shown when the active tab is an internal page. |
| `status` | `src/ui/status.html` (bundled) | events only | The link-hover bubble, bottom left, over the content. |
| `tab-<id>-<n>` | remote `http(s)://*.i2p/` | NO | One per web tab. Only the active one is visible. |

On macOS the window has an overlay title bar with a hidden title. The traffic lights sit in the tab row. Windows and Linux keep the native decorations.

Internal pages: `eepview://home`, `bookmarks`, `history`, `stats`, `settings`, `setup`, `blocked`, `router-down`.
`eepview://<page>?<query>` loads `src/ui/<page>.html?<query>` in the `internal` webview.

| Page | Parameters |
|---|---|
| `blocked` | `url`: the refused address |
| `router-down` | `url`: the page to load when the router is back; `state`: `RouterStatus.state`; `reason=paused` while the connection is paused |
| `history` | `q`: the search text from the address bar |

## Commands

Only `toolbar` and `internal` may call them (`src-tauri/capabilities/`). Arguments are camelCase in JS.

### Tabs

- `tab_new(url?: string) -> TabInfo`. Default `eepview://home`. The new tab becomes active.
- `tab_close(id)`. Closing the last tab opens a new home tab.
- `tab_select(id)`, `tab_move(id, index)`, `tab_list() -> TabInfo[]`.

### Navigation (active tab)

- `navigate(input) -> NavResult`.
  - `foo.i2p` becomes `http://foo.i2p/`.
  - An `http(s)` URL on a `.i2p` or `.b32.i2p` host loads.
  - `eepview://x` opens an internal page.
  - Text with no dot and no scheme searches history and bookmarks (`eepview://history?q=…`).
  - Anything else is refused with `{ok: false, reason: "not-i2p"}` and shows `eepview://blocked?url=…`.
- `go_back()`, `go_forward()`, `reload(hard?)`, `stop()`, `home()`. They work with page JavaScript off: they call the engine's native API.

### Find, zoom, JavaScript

- `find(query, forward, matchCase)` emits `find-result`. `find_close()`. Works with page JavaScript off.
- `zoom_in()`, `zoom_out()`, `zoom_reset()`: remembered per host; emit `tab-updated`. `TabInfo.zoom` is a factor (1.0 = 100 %).
- `site_js_set(host, on)`: rebuilds the tab webview at the same URL.

### Bookmarks

- `bookmarks_list() -> Bookmark[]`
- `bookmark_add({bookmark: {url, title, folder?}}) -> Bookmark`
- `bookmark_update({bookmark: Bookmark})`, `bookmark_remove(id)`, `bookmark_find(url) -> Bookmark | null`
- `bookmarks_export() -> string` (JSON), `bookmarks_import(json) -> number`
- `bookmarks_export_file() -> string`: writes `eepview-bookmarks-<YYYYMMDD>.json` to the Downloads folder and returns its path.
- First run seeds `stats.i2p`, `i2p-projekt.i2p`, `reg.i2p`, `notbob.i2p`.

### History

At most 10 000 entries. Nothing is recorded while `history.enabled` is false.

- `history_query({query: {q?, before?, limit?}}) -> HistoryEntry[]`, newest first, ordered by (`visited` desc, `id` desc). `before` is a cursor `{visited, id}` (a bare number of Unix ms also works).
- `history_remove(id)`, `history_clear(range: "hour" | "day" | "week" | "all")`
- `suggest(input) -> Suggestion[]`: at most 8, from bookmarks and history.

### Settings

`settings_get() -> Settings`, `settings_set({patch: Partial<Settings>}) -> Settings`.

### Router and connection

- `router_status() -> RouterStatus`
- `router_stats() -> RouterStats`. Every field may be `null`: an external router has no helper.
- `connection_pause()`: closes the gatekeeper, destroys every `tab-*` webview and shows `eepview://router-down?reason=paused`.
- `connection_resume()`: runs VERIFY again. The gatekeeper opens and the active tab reloads only when VERIFY passes. If it fails, everything stays closed.
- `router_control({action: "stop" | "start" | "restart"}) -> {ok: boolean, reason?: string}`. It answers `{ok: false, reason: "external"}` until eepview runs its own router (Phase 3).

### Window

- `chrome_set_height(px)`: the toolbar grows over the content while a popup (suggestions, router panel) is open. The content does not move. `0` goes back to the default. Clamped to 84–480.
- `platform() -> "macos" | "windows" | "linux"`

## Events

Rust sends them to `toolbar`, `internal` and `status`.

- `tabs-changed: TabInfo[]`: open, close, move, select.
- `tab-updated: TabInfo`: URL, title, loading, history, zoom.
- `find-result: {query, matches: number | null, active: number | null}`
- `router-status: RouterStatus`: every 5 s and on change.
- `bookmarks-changed`, `history-changed`, `settings-changed: Settings`
- `shortcut: {action}`: actions the UI handles (`focus-address`, `open-find`, …).
- `toast: {kind: "info" | "warn", text}`: refused downloads, new windows and in-page navigations.
- `link-hover: {text, blocked}`: the link under the mouse, already decoded and shortened to about 80 characters. `blocked: true` for a link that would be refused (`text` = `Blocked: <host>`). `text` is `Loading <host>…` while a page loads, and empty when there is nothing to show. It shows at once and hides after 150 ms.
- `fullscreen-changed: boolean`

## Types

```ts
type TabInfo = { id: number; url: string; title: string; kind: "internal" | "web";
  loading: boolean; canBack: boolean; canForward: boolean; active: boolean;
  zoom: number; jsOn: boolean; bookmarked: boolean };
type NavResult = { ok: boolean; reason?: "not-i2p" | "router-down" | "invalid" };
type Bookmark = { id: string; url: string; title: string; folder: string | null; created: number };
type HistoryEntry = { id: string; url: string; title: string; visited: number; visits: number };
type Suggestion = { url: string; title: string; source: "bookmark" | "history" };
type Settings = { homepage: string; theme: "system" | "light" | "dark"; jsDefault: boolean;
  history: { enabled: boolean }; keepCookies: boolean; zoomDefault: number };
type RouterStatus = { state: "verifying" | "ok" | "building" | "down" | "not-i2p" | "outproxy";
  proxy: string; version: string | null; detail: string | null;
  paused: boolean; managed: boolean };
type RouterStats = { version: string | null; uptimeMs: number | null; networkStatus: string | null;
  knownRouters: number | null; activePeers: number | null;
  tunnels: { in: number | null; out: number | null; participating: number | null };
  bandwidthBytesPerSecond: { in1s: number | null; out1s: number | null;
    in5m: number | null; out5m: number | null };
  tunnelBuildSuccessPercent: { exploratory: number | null; client: number | null };
  history: { t: number; in: number; out: number }[] };  // last 10 min, one sample per 5 s
```

`jsDefault` is `true`: JavaScript is on unless you turn it off for a site.

## Keyboard shortcuts

Cmd on macOS, Ctrl elsewhere. Rust handles them as menu accelerators.

New tab T, close tab W, reopen closed tab Shift+T, next and previous tab Ctrl+Tab and Ctrl+Shift+Tab, tab 1–8 and last 9, address bar L, find F, find next G, find previous Shift+G, reload R, hard reload Shift+R, back [ and Alt+Left, forward ] and Alt+Right, home Shift+H, bookmark D, bookmarks Shift+B, history Y (macOS) or H (others), zoom + − 0, stop Esc while loading, settings comma.

## Security rules

See [ADR 0001](adr-0001-no-leak-architecture.md).

- `tab-*` webviews get no IPC, use the gatekeeper as proxy, run incognito unless `keepCookies`, and have WebRTC off.
- No `tab-*` webview exists before VERIFY passes. When the router goes down, or you pause, every `tab-*` is destroyed.
- New-window requests open as a new tab through the same guard. Downloads are refused with a toast.

## Environment and command line

| Variable | Effect |
|---|---|
| `EEPVIEW_PROXY` | The router HTTP proxy, `127.0.0.1:<port>` (default `127.0.0.1:4444`). It must still pass VERIFY. |
| `EEPVIEW_START_URL` | Opens this URL in the first tab and skips the first-run setup. |
| `EEPVIEW_EXIT_AFTER` | Quits with exit code 0 after this many seconds. |
| `EEPVIEW_JS` | `off` turns page JavaScript off for every site. |
| `EEPVIEW_ROUTER_STATUS`, `EEPVIEW_ROUTER_STATUS_TOKEN` | The router helper address and the file with its token, for `router_stats`. |
| `EEPVIEW_LOG` | `1` prints load timings and router changes to stderr. |

They exist in the release binary, for the leak test. None of them can weaken a layer.

`eepview <url>…` opens each argument through the rules of `navigate`, one tab each.
