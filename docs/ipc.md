# eepview browser shell — IPC contract v1

The Rust core and the UI build against this file.
Change it only by message to the PO.

## Window layout (one OS window, Tauri `unstable` multi-webview)

| Webview label | Loads | IPC | Notes |
|---|---|---|---|
| `toolbar` | `ui/toolbar.html` (bundled) | yes | Top strip, 84 px: tab strip row + nav row. Find bar slides in under it (toolbar grows to 124 px while open). |
| `internal` | `ui/<page>.html` (bundled) | yes | Shown when the active tab is an internal page. |
| `tab-<n>` | remote `http://*.i2p/` | NO | One per web tab. Proxy = verified I2P proxy. Only the active one is visible. |

Internal pages: `eepview://home`, `eepview://bookmarks`, `eepview://history`, `eepview://stats`,
`eepview://settings`, `eepview://setup`, `eepview://blocked`, `eepview://router-down`.
A tab holds either an internal page or a web page. The address bar shows `eepview://…` for internal.

## Commands (`invoke`), allowed from `toolbar` and `internal` only

Tabs:
- `tab_new(url?: string) -> TabInfo` — default url `eepview://home`. Becomes active.
- `tab_close(id: number)` — closing the last tab opens a new home tab.
- `tab_select(id: number)`
- `tab_move(id: number, index: number)`
- `tab_list() -> TabInfo[]`

Navigation (act on the active tab):
- `navigate(input: string) -> NavResult` — input from the address bar. Rules: `foo.i2p` → `http://foo.i2p/`;
  `*.i2p` / `*.b32.i2p` URL (http or https) → load; `eepview://x` → internal page; anything else →
  if it has no dot and no scheme, treat as a search in history+bookmarks and open `eepview://history?q=…`;
  otherwise refuse: `{ok:false, reason:"not-i2p"}` and show `eepview://blocked?url=…`.
- `go_back()`, `go_forward()`, `reload(hard?: boolean)`, `stop()` — must work with page JS OFF.
- `home()` — opens the configured homepage in the active tab.

Find in page (active web tab, must work with page JS OFF):
- `find(query: string, forward: boolean, matchCase: boolean)` → emits `find-result`
- `find_close()`

Zoom (active tab, per-host remembered): `zoom_in()`, `zoom_out()`, `zoom_reset()` → emits `tab-updated`.

Site JS toggle: `site_js_set(host: string, on: boolean)` — recreates the tab webview at the same URL.

Bookmarks (JSON file in app config dir, schema version field):
- `bookmarks_list() -> Bookmark[]`, `bookmark_add({url,title,folder?}) -> Bookmark`,
  `bookmark_update(Bookmark)`, `bookmark_remove(id: string)`, `bookmark_find(url) -> Bookmark|null`,
  `bookmarks_export() -> string(JSON)`, `bookmarks_import(json: string) -> number`.
- Seed on first run: `stats.i2p`, `i2p-projekt.i2p`, `reg.i2p`, `notbob.i2p`.

History (JSON-lines or JSON file in app data dir, capped at 10 000 entries, skipped entirely when
setting `history.enabled` is false):
- `history_query({q?: string, before?: number, limit?: number}) -> HistoryEntry[]` (newest first)
- `history_remove(id: string)`, `history_clear(range: "hour"|"day"|"week"|"all")`
- `suggest(input: string) -> Suggestion[]` — max 8, from bookmarks + history, prefix/substring on url+title,
  ranked by visit count and recency.

Settings: `settings_get() -> Settings`, `settings_set(patch: Partial<Settings>) -> Settings`.
Settings: `{ homepage: string, theme: "system"|"light"|"dark", jsDefault: boolean,
history: {enabled: boolean}, keepCookies: boolean, zoomDefault: number }`.

Router: `router_status() -> RouterStatus`.

## Events (`listen`), Rust → toolbar + internal

- `tabs-changed: TabInfo[]` — on open, close, move, select.
- `tab-updated: TabInfo` — on url, title, loading, history, zoom change.
- `find-result: {query, matches: number|null, active: number|null}` — null when the engine does not report a count.
- `router-status: RouterStatus` — every 5 s and on change.
- `bookmarks-changed`, `history-changed`, `settings-changed: Settings`.
- `shortcut: {action: string}` — for actions the UI must handle (`focus-address`, `open-find`).

## Types

```ts
type TabInfo = { id: number; url: string; title: string; kind: "internal"|"web";
  loading: boolean; canBack: boolean; canForward: boolean; active: boolean;
  zoom: number; jsOn: boolean; bookmarked: boolean };
type NavResult = { ok: boolean; reason?: "not-i2p"|"router-down"|"invalid" };
type Bookmark = { id: string; url: string; title: string; folder: string|null; created: number };
type HistoryEntry = { id: string; url: string; title: string; visited: number; visits: number };
type Suggestion = { url: string; title: string; source: "bookmark"|"history" };
type RouterStatus = { state: "verifying"|"ok"|"building"|"down"|"not-i2p"|"outproxy";
  proxy: string; version: string|null; detail: string|null };
```

## Keyboard shortcuts (Cmd on macOS, Ctrl elsewhere) — handled in Rust (menu accelerators)

New tab T · close tab W · reopen closed tab Shift+T · next/prev tab Ctrl+Tab / Ctrl+Shift+Tab ·
tab 1–8, last 9 · address bar L · find F · find next G / Enter · find prev Shift+G / Shift+Enter ·
reload R · hard reload Shift+R · back [ and Alt+Left · forward ] and Alt+Right · home Shift+H ·
bookmark this page D · bookmarks Shift+B · history Y (macOS) / H (others) · zoom + / − / 0 ·
stop Esc (when loading) · settings , (comma).

## Security rules (non-negotiable)

- `tab-*` webviews: no IPC capability, `proxy_url` = verified proxy, `incognito(true)` unless
  `keepCookies`, JS per `jsDefault` + per-host override, WebRTC off (spike S3), loopback forced
  through the proxy (spike S1).
- `on_navigation` on `tab-*`: allow only `http(s)://<host>.i2p[...]`. Everything else is cancelled; a
  top-level attempt shows `eepview://blocked?url=…` in that tab.
- New-window requests (`target=_blank`, `window.open`) open as a new tab through the same guard.
- Downloads: refused for now; emit a toast event `toast: {kind:"info", text}`.
- No `tab-*` webview exists before `router_status().state == "ok"`. When the router goes down, every
  `tab-*` is destroyed and the tabs show `eepview://router-down` until it comes back.

## Wire details (v1.1, additive)

These notes pin down what the v1 text leaves open. They add to v1 and change nothing in it.

### Argument names

`invoke` arguments are camelCase in JS. Commands that take one object take it under a named key:

| Command | Arguments |
|---|---|
| `bookmark_add` | `{ bookmark: { url, title, folder? } }` |
| `bookmark_update` | `{ bookmark: Bookmark }` |
| `history_query` | `{ query: { q?, before?, limit? } }` (`before` and `visited` are Unix milliseconds) |
| `settings_set` | `{ patch: Partial<Settings> }` |
| `find` | `{ query, forward, matchCase }` |

### Internal page files

`eepview://<page>?<query>` loads the bundled file `src/ui/<page>.html?<query>` in the `internal`
webview. The page reads its parameters from `location.search`:

| Page | Parameters |
|---|---|
| `blocked` | `url` — the refused address |
| `router-down` | `url` — the page the tab will load when the router is back; `state` — the `RouterStatus.state` |
| `history` | `q` — the search text from the address bar |

### Extra command: `toolbar_set_height(px: number)`

The `toolbar` webview cannot paint over a `tab-*` webview. To show the address-bar suggestion
list, the toolbar asks for more height, and asks for 0 to go back to the default (84 px, or
124 px with the find bar open). The value is clamped to 84–480. Optional: a UI that never
calls it keeps the v1 behaviour.

### Event `toast`

`toast: { kind: "info" | "warn", text: string }` — for refused downloads, refused new windows and
refused navigations inside a page.

### Command line

`eepview <url>…` opens each argument through the same rules as `navigate`, one tab each.
