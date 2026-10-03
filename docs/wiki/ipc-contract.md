<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# IPC contract

Version 1.4. The Rust shell (`src-tauri/`) and the UI (`src/ui/`) build against this page.
Change it in a PR that changes both sides, or keep the old form working as a shim.

## History

- v1: first contract.
- v1.1: argument names, `chrome_set_height`, `platform`, `router_stats`, `bookmarks_export_file`, history cursors, `link-hover`, `fullscreen-changed`, `toast`.
- v1.3: `chrome_insets`, `chrome-insets-changed`, `window_fullscreen`, `status-size`, `status-side`; the bubble shows after 100 ms and hides at once.
- v1.4: `TabInfo.icon`, `Bookmark.icon`, `HistoryEntry.icon`, `icons-changed`. See [Site icons](site-icons.md). [#53](https://github.com/tcivie/eepview/pull/53)
- v1.2: `connection_pause`, `connection_resume`, `router_control`, `RouterStatus.paused` and `.managed`, `RouterStats.history`. The `outproxy` state is gone: VERIFY no longer asks for a clearnet host.
- v1.3: `report_preview`, `report_open`, `diag_crash_status`, `diag_crash_dismiss`, `diag_logs_delete` (internal webview only); the `eepview://report` page; `EEPVIEW_LOG` is gone. See [Diagnostics and bug reports](diagnostics-and-bug-reports.md). Added in [#56](https://github.com/tcivie/eepview/pull/56).
- Shipped in [#29](https://github.com/tcivie/eepview/pull/29).
- v1.4: the `popup` webview, `popup_open`, `popup_size`, `popup_close`, `popup-show`, `popup-closed`, `popup-select`. `chrome_set_height` reports the find bar only; the toolbar never grows for a popup — [#55](https://github.com/tcivie/eepview/pull/55).

## Window layout

One OS window with several webviews (Tauri `unstable` multi-webview).

| Webview label | Loads | IPC | Notes |
|---|---|---|---|
| `toolbar` | `src/ui/toolbar.html` (bundled) | yes | Top strip, 84 px: tab row (44 px) and nav row. 124 px with the find bar open. It sits above the content in z-order. |
| `internal` | `src/ui/<page>.html` (bundled) | yes | Shown when the active tab is an internal page. |
| `status` | `src/ui/status.html` (bundled) | events only | The link-hover bubble, bottom left, over the content. |
| `popup` | `src/ui/popup.html` (bundled) | popup commands | The toolbar popups (suggestions, menu, router panel, router hint). Transparent, hidden until a popup opens, then sized and placed to the popup's own rectangle, on top of every other webview. |
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
  - Blank or whitespace-only input is ignored.
  - `foo.i2p` becomes `http://foo.i2p/`. The host is lower-cased and one trailing dot is dropped.
  - An `http(s)` URL on a `.i2p` or `.b32.i2p` host loads, with or without an explicit port 1–65535.
  - `eepview://x` opens an internal page.
  - One word with no dot, no colon and no scheme searches history and bookmarks (`eepview://history?q=…`).
  - Anything else that parses is refused with `{ok: false, reason: "not-i2p"}` and shows `eepview://blocked?url=…` (`localhost:8080` is `not-i2p`). Input that does not parse is `invalid`.
- `go_back()`, `go_forward()`, `reload(hard?)`, `stop()`, `home()`. They work with page JavaScript off: they call the engine's native API.

### Find, zoom, JavaScript

- `find(query, forward, matchCase)` emits `find-result`. `find_close()`. Works with page JavaScript off.
- `zoom_in()`, `zoom_out()`, `zoom_reset()`: remembered per host; emit `tab-updated`. `TabInfo.zoom` is a factor (1.0 = 100 %).
- `site_js_set(host, on)`: rebuilds the tab webview at the same URL.

### Bookmarks

- `bookmarks_list() -> Bookmark[]`
- `bookmark_add({bookmark: {url, title, folder?}}) -> Bookmark`. A non-I2P URL is refused with the error `not-i2p`.
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

- `chrome_set_height(px)`: the toolbar reports its find bar: 124 or more while it shows, 84 when it is hidden. The toolbar is 124 px when the core has the find bar open (`find`, the Find shortcut) or the last report is 124 or more, else 84 px. It never takes another height.
- `popup_open({kind, anchor: {x, y, width, height}, data?}) -> number` (`toolbar` only): opens a popup under `anchor` (window points) and returns its id. `kind` is `"suggestions" | "menu" | "router" | "hint"`. `data` goes to the page unchanged: `{items: Suggestion[], index: number}` for the suggestions, `{title, text}` for the hint. The shell sends `popup-show` to `popup`; the popup shows once the page reports its size. Opening another kind closes the open one; the same kind updates in place.
- `popup_size({id, width, height})` (`popup` only): the natural size of the popup's card. The shell clamps it to the window (8 px margin), places the `popup` webview 4 px under the anchor and shows it, above every other webview (`popup` is raised last when a tab webview is added). The menu and the router panel take the focus. The size is the content size of the card with no width or height limit (`scrollWidth`/`scrollHeight` of an unconstrained card), never the size of the webview.
- `popup_close({id, refocus?})` (`toolbar` and `popup`): closes the popup `id`. A stale id does nothing. Only the shell closes any popup (on a window resize). `refocus: true` gives the focus back to the toolbar. The shell hides `popup` and sends `popup-closed`.
- `platform() -> "macos" | "windows" | "linux"`
- `window_fullscreen() -> boolean`
- `chrome_insets() -> {left: number}`: the space the tab strip leaves on the left for the macOS window buttons. The shell centers the buttons on the tab row (y = 22), measures their frames, and answers their right edge plus their left margin, so the gap after the buttons equals the margin before them. 0 in full screen, and 0 on Windows and Linux (native title bar; the UI picks its own margin).

### Diagnostics and reports

Only the `internal` webview may call these (`capabilities/report.json`). `toolbar`, `status` and `tab-*` cannot.

- `report_preview({kind, description, includeLog}) -> string`: exactly the text that goes out, scrubbed.
- `report_open({kind, description, includeLog}) -> {file: string, trimmed: boolean}`: saves the report in Downloads, shows it in the file manager, and opens the prefilled GitHub issue in the system browser. `file` is the file name, no folder.
- `diag_crash_status() -> boolean`: the last run crashed and the banner was not dismissed.
- `diag_crash_dismiss()`
- `diag_logs_delete()`: deletes the diagnostics log and the crash marker.

## Events

Rust sends them to `toolbar`, `internal`, `status` and `popup`.

- `tabs-changed: TabInfo[]`: open, close, move, select.
- `tab-updated: TabInfo`: URL, title, loading, history, zoom.
- `find-result: {query, matches: number | null, active: number | null}`
- `router-status: RouterStatus`: every 5 s and on change.
- `bookmarks-changed`, `history-changed`, `settings-changed: Settings`
- `shortcut: {action}`: actions the UI handles (`focus-address`, `open-find`, …).
- `toast: {kind: "info" | "warn", text}`: refused downloads, new windows and in-page navigations.
- `link-hover: {text, blocked}`: the link under the mouse, already decoded and shortened to about 80 characters. `blocked: true` for a link that would be refused (`text` = `Blocked: <host>`). `text` is `Loading <host>…` while a page loads, and empty when there is nothing to show. It shows after 100 ms of hovering (at once when the bubble is already up) and hides at once.
- `fullscreen-changed: boolean`
- `status-side: "left" | "right"` (to `status` only): the corner the bubble sits in. It moves to the bottom right while the mouse is over the bottom-left spot.
- `chrome-insets-changed: {left: number}`: on full screen enter and exit, and when the scale factor or the button frames change.
- `icons-changed: null`: a site icon was stored or deleted. `tabs-changed` follows. Read the lists again for the new `icon` values.

- `popup-show: {id, kind, anchorWidth, data}` (to `popup` only): render this popup, measure it, call `popup_size`.
- `popup-closed: {id, kind, refocus}` (to `toolbar` and `popup`): the popup closed (Esc, a click outside, a resize, or another popup).

### Events a page sends

- `popup-select: {index}` (from `toolbar`, to `popup`): the arrow keys moved the highlight of the open suggestions. The page moves it; no new id, no new size.
- `status-size: {width, height}` (from `status` only): the natural size of the pill. The shell fits the transparent `status` webview to it, at most half the content width; longer text ends in an ellipsis. The webview never sits under the mouse.

## Types

```ts
type TabInfo = { id: number; url: string; title: string; kind: "internal" | "web";
  loading: boolean; canBack: boolean; canForward: boolean; active: boolean;
  zoom: number; jsOn: boolean; bookmarked: boolean;
  icon: string | null };  // 32 px site icon, data:image/png;base64,…; null for internal tabs
type NavResult = { ok: boolean; reason?: "not-i2p" | "router-down" | "invalid" };
type Bookmark = { id: string; url: string; title: string; folder: string | null; created: number;
  icon: string | null };  // 64 px site icon
type HistoryEntry = { id: string; url: string; title: string; visited: number; visits: number;
  icon: string | null };  // 32 px site icon
type Suggestion = { url: string; title: string; source: "bookmark" | "history" };
type Settings = { homepage: string; theme: "system" | "light" | "dark"; jsDefault: boolean;
  history: { enabled: boolean }; keepCookies: boolean; zoomDefault: number };
type RouterStatus = { state: "verifying" | "ok" | "building" | "down" | "not-i2p";
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

`icon` is a `data:image/png;base64,` URL that eepview drew itself, or `null`. `bookmark_update` and `bookmarks_import` ignore an `icon` they receive, and the stores never keep one. See [Site icons](site-icons.md).

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

They exist in the release binary, for the leak test. None of them can weaken a layer.

`eepview <url>…` opens each argument through the rules of `navigate`, one tab each.
