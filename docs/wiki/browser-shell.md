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
- Site icons: when a web tab finishes a page on a bookmarked or visited host, the shell asks that host for `/favicon.ico` through the gatekeeper, at most once a day. The icon is decoded and drawn again as a new PNG. See [Site icons](site-icons.md).
- Security: see [No-leak architecture](no-leak-architecture.md) and [ADR 0001](adr-0001-no-leak-architecture.md).

## Requirements (owner decisions)

- Only an `http(s)` URL on a `.i2p` host enters tab state or reaches an engine. `Core::load` is the one place a load is made, and it checks the rule again, so no other path can replay a clearnet URL.
- Address bar: blank input is ignored; the host is lower-cased and one trailing dot dropped; one word with no dot, no colon and no scheme searches; anything else that is not on `*.i2p` is `not-i2p`; unparseable input is `invalid`.
- `.i2p` URLs may carry an explicit port 1–65535 (http and https). The gatekeeper forwards them; `CONNECT` stays limited to `:80` and `:443`.
- Back and forward follow the standard per-tab session history: a new navigation clears the forward list, and back and forward never leave the tab's own history.
- `bookmark_add` refuses a non-I2P URL with `not-i2p`.
- Hand-edited store files are not trusted: a settings homepage that is not an I2P site or an internal page falls back to `eepview://home`; history keeps only I2P entries, newest first, at most 10 000.
- Gatekeeper: duplicate `Content-Length` headers are refused with 400; a request body cut short closes both sides at once.

## Toolbar popups

The toolbar has four popups: the address suggestions, the main menu, the router panel and the router hint (hovering the router dot). They show in their own webview, `popup`, over the page. The toolbar never grows for them.

### Requirements

1. Opening a popup never hides or moves the page. Only the popup's own rectangle covers the page. Everything outside that rectangle stays visible and takes clicks.
2. The toolbar is always exactly 84 px high, or 124 px while the find bar is open. No popup changes it. The content area starts right under the toolbar and fills the rest of the window.
3. A popup opens 4 px below its anchor (its button, or the address field). The suggestions line up with the left edge of the address field and take its width. The menu, the router panel and the hint line up with the right edge of their button.
4. A popup never leaves the window. It stays at least 8 px from the left and right window edges, and it is at most the window width minus 16 px wide.
5. A popup is never cut. It gets the height it needs, up to the window bottom minus 8 px. When its content is taller, the popup scrolls inside itself, so its last row (for example the router panel's action buttons) can always be reached.
6. One popup shows at a time. Opening a popup of another kind closes the open one first. Opening the same kind again (the suggestions while you type) updates it in place. The toolbar never asks for the hint while the menu or the router panel is open, so the hint never replaces them.
7. The router hint shows while the mouse is over the router dot and hides when the mouse leaves. It is small: a title and one short line, at most 280 px wide. It never resizes anything and never takes focus. It does not show while the menu or the router panel is open.
8. A click on the router dot opens the router panel. A second click on the dot closes it. Esc closes it, and focus goes back to the dot. A click anywhere outside the panel (on the toolbar or on the page) closes it. The toolbar reads the state at `pointerdown` on the dot: when the panel was open at that moment, the click closes it, however long the press lasts.
9. The menu button and the menu act the same as rule 8. A menu item that opens a page or a tab closes the menu; the zoom buttons keep it open and show the new zoom.
10. The suggestions show while the address field has focus and there are suggestions. The arrow keys move the selection, Enter opens it, and a click on a suggestion opens it. Esc, or leaving the address field, closes the list. A pick in the popup acts on `mousedown`, and the toolbar closes the list 150 ms after the address field loses focus, so the pick always lands first. An arrow key only moves the highlight (`popup-select`): it does not re-measure or re-place the popup.
11. A window resize closes the open popup.
12. A close or a size report for a popup that is no longer open does nothing. The toolbar and the popup page always close a popup by its id.
13. Popups use the palette tokens only and follow the light and dark themes. The webview is transparent, so nothing shows around the popup's card.
14. Security: `popup` is a bundled eepview page. It never loads a remote page, and it gets only the commands its popups need (listed below). `tab-*` webviews still get no IPC.
15. The popup page reports the natural size of its card: the size of its content with no width or height limit, not the size of the webview it sits in. So a list that grows while it is open gets its new height.
16. The `popup` webview stays above every other webview, also after a new tab webview is added.

### Public interface

Rust, in the `eepview_lib` crate (pure, no Tauri):

- `layout::TOOLBAR` (84), `layout::TOOLBAR_FIND` (124).
- `layout::toolbar_height(find_open: bool, requested: f64) -> f64`: 124 when `find_open` or when `requested` is 124 or more, else 84. Nothing else.
- `layout::split(width: f64, height: f64, toolbar: f64) -> (Rect, Rect)`: the toolbar strip (0, 0, width, toolbar) and the content area right under it, to the window bottom. Heights are never negative.
- `layout::popup(anchor: Rect, size: (f64, f64), window: (f64, f64), align: Align) -> Rect`: the popup rectangle for a card of natural `size` (width, height) under `anchor`, in a window of `window` (width, height). `Align::Start` lines up the left edges, `Align::End` the right edges. Rules 3 to 5.
- `popup::Kind`: `Suggestions`, `Menu`, `Router`, `Hint` (JSON `"suggestions"`, `"menu"`, `"router"`, `"hint"`). `Kind::align(self) -> layout::Align` (`Start` for the suggestions, `End` for the others). `Kind::takes_focus(self) -> bool` (true for the menu and the router panel).
- `popup::Closed { id: u64, kind: Kind }`.
- `popup::Popups` (`Default`): the one-popup state of rule 6 and rule 12.
  - `open(&mut self, kind: Kind, anchor: Rect) -> (u64, Option<Closed>)`: a new id (ids start at 1 and only go up). It returns the popup it closed, if one of another kind was open.
  - `size(&mut self, id: u64, size: (f64, f64), window: (f64, f64)) -> Option<Rect>`: where to place the open popup. `None` when `id` is not the open popup, or when the size is not finite and positive.
  - `close(&mut self, id: Option<u64>) -> Option<Closed>`: closes the open popup when `id` is it or is `None`. A stale id does nothing.
  - `current(&self) -> Option<Closed>`: the open popup.

TypeScript (pure):

- `src/ui/lib/chrome-height.ts`: `chromeHeight({ findOpen }) -> number`: 84, or 124 with the find bar.
- `src/ui/lib/popup-toggle.ts`: `PopupKind`, `TOGGLE_MS` (300), and `clickOpens(open: PopupKind | null, closedAt: number | undefined, kind: PopupKind, now: number) -> boolean`: a click on the button of `kind` opens it, unless that popup is open or closed less than 300 ms ago (the same click closed it, rule 8). The toolbar passes the state and the time of the `pointerdown` on the button.

IPC: see the [IPC contract](ipc-contract.md) (v1.4): `popup_open`, `popup_size`, `popup_close`, `popup-show`, `popup-closed` and `popup-select`.

The `popup` webview may call: `popup_size`, `popup_close`, `navigate`, `tab_new`, `tab_list`, `zoom_in`, `zoom_out`, `zoom_reset`, `router_status`, `router_stats`, `connection_pause`, `connection_resume`, `router_control`, `console_status`, `console_detect`, `console_open`, and listen to events.

## Router console

eepview shows router information but never changes the router configuration. It finds the console of the router in use (Java I2P or i2pd) on loopback and gives quick links to it: Console, Tunnels, Address book, Config and Logs. The links show in the router panel, on the home page and in Settings. A link opens the router's own page in a separate `Router console` window. With no console, one line says "No router console found". Details: [Router console](router-console.md).

## How to use / run locally

- `npm run tauri dev` with an I2P router on `127.0.0.1:4444`.
- `EEPVIEW_PROXY=127.0.0.1:<port>` picks another router proxy. `EEPVIEW_START_URL`, `EEPVIEW_EXIT_AFTER`, `EEPVIEW_JS=off` and `EEPVIEW_LOG=1` are listed in the [IPC contract](ipc-contract.md#environment-and-command-line).
- Tests: `cargo test --workspace` in `src-tauri`.

## Limits

- Find on Windows uses an app-injected script (`WebView2` has no native find with a count).
- The Linux engine filter (L3b) waits for a webkit2gtk binding; the page policy (L3a) holds there.
- HTTPS eepsites load only on Windows: TLS tunnels stay closed on macOS and Linux (ADR 0001).
- The first-run setup flow and router control are Phase 2 and Phase 3.

## History

- 2026-10-03 — Browser shell: tabs, navigation, bookmarks, history, find, gatekeeper, pause and resume — [#29](https://github.com/tcivie/eepview/pull/29)
- 2026-10-03 — Site icons in tabs, bookmarks and history — [#53](https://github.com/tcivie/eepview/pull/53)
- 2026-10-03 — Toolbar popups in their own `popup` webview; the toolbar stays 84 px — [#55](https://github.com/tcivie/eepview/pull/55)
- 2026-10-03 — Router console quick links and the console window — [#54](https://github.com/tcivie/eepview/pull/54)
