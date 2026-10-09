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
- Gatekeeper: a bare CR in the request line or a header, and a bare LF in a header value, get 400 and nothing goes upstream (RFC 9112), also inside a `CONNECT` tunnel; duplicate `Content-Length` headers are refused with 400; a request body cut short closes both sides at once.

## Requirements (UX batch 1)

Found in QA of the first shell build. Fixed in [#57](https://github.com/tcivie/eepview/pull/57).

1. After a page loads from the address bar, keyboard scrolling (Page Down, Space, the arrow keys) works on the page with no extra click. Keyboard focus moves to the page when the first commit of that navigation arrives, once, whatever address it commits: a redirect target counts. A later commit (a link, a script, a second redirect) does not move focus. Pressing Cmd/Ctrl+L before the commit cancels the move, so typing in the address bar is never taken away. A navigation from the address bar to an internal page moves focus to that page at once.
2. While a web page loads, the content area shows the theme surface colour (dark in dark mode), never white. A new web tab starts with its background set to the `--color-surface` token of the saved theme, before its first paint, and the window behind the tabs has the same colour. "System" follows the operating system. The Settings page saves the theme choice in the settings store, so the shell knows it, and a theme change repaints the window. When the first page of a tab has finished, the tab goes back to the engine default background, so a page that sets no background of its own stays readable in dark mode.
3. A page load that failed is not saved in history. That covers the I2P "Website Unreachable" page, any HTTP 5xx answer that passes the gatekeeper (the router proxy's and an eepsite's own), and a gatekeeper refusal for an I2P address. The gatekeeper matches an answer to a page by the exact URL, not by tab. The tab stops loading and shows the page. A history entry that already exists for the address stays as it is: its title, visit count and visit time do not change.
4. Cmd/Ctrl+T opens a new tab, puts keyboard focus in the address bar and selects its text. The "New tab" button and the menu item do the same. A tab that a page opens (`target=_blank`) or that opens with an address does not put focus in the address bar.
5. In Settings, "Where eepview connects" lists the router proxy that eepview really uses, the same value the Home page shows, including a value set with `EEPVIEW_PROXY`. Both pages show a dash until the proxy is known.
6. The Router card on the Home page never shows "Unknown". It shows `I2P <version>` when the router reports its version, and "Version not reported by the router" when it does not. The version comes from the router statistics when a router helper is set up.
7. After you close the last tab, Cmd/Ctrl+Shift+T opens that closed tab again, with the page it showed. The fresh Home tab that replaced it may stay or go.

## macOS bundle: App Transport Security

- **B1.** The macOS `.app` loads `http://` eepsites. App Transport Security (ATS) applies to every app with an `Info.plist`, and by default it refuses each `http://` load of a `WKWebView`. `src-tauri/Info.plist` (Tauri merges it into the bundle) sets one key: `NSAppTransportSecurity` > `NSAllowsArbitraryLoadsInWebContent`. It covers web view content only. The file sets no other key, and the ATS exemption is no wider (`src-tauri/tests/app_transport.rs`).
- ADR 0001 does not change. Every content request still goes to the gatekeeper on loopback, and the gatekeeper forwards only `.i2p` hosts. The Rust code uses plain sockets, which ATS never covers, so the key changes nothing for the gatekeeper, VERIFY or the icon fetch.
- The macOS leak test runs the binary inside the `.app` bundle, as users do. A bare binary has no `Info.plist`, so ATS never applies to it.
- The release job checks the key in the `.app` of each dmg (`scripts/verify-dmg-signature.sh`), so a bundler change that drops it fails the release.

## Failed loads

A tab never spins forever. Before this, a load that the engine failed on macOS left the tab loading with a blank page: wry reports no failed navigation there.

- **F1.** When the engine fails the main-frame load of a web tab, before the first commit or after it, the tab stops loading and shows `eepview://load-failed?url=<address>&reason=<reason>&code=<code>`. On macOS the platform bridge reports `webView:didFailProvisionalNavigation:withError:` and `webView:didFailNavigation:withError:` (`eepview_platform::on_load_failed`). Windows (`NavigationCompleted`) and Linux (`load-changed` after `load-failed`) already report a failed load as finished and show the engine's own error page, so the bridge reports nothing there.
- **F2.** A cancelled load is not a failure, and the tab stays as it is: `NSURLErrorDomain` -999 (a new load replaced it, or Stop) `WebKitErrorDomain` 102 (the navigation guard or a download policy stopped it), and `WebKitErrorDomain` 204 (the engine shows the file itself, such as a media file).
- **F3.** `reason` is `blocked` when the system or the engine filter stopped the request before it left the computer (`NSURLErrorDomain` -1022 App Transport Security, `WebKitErrorDomain` 104 content rule list). It is `unreachable` when the connection to the gatekeeper failed (`NSURLErrorDomain` -1001 timed out, -1003 host not found, -1004 cannot connect, -1005 connection lost, -1006 DNS failed, -1009 offline). Any other error is `engine`.
- **F4.** `code` is the error domain, a space and the error number, for example `NSURLErrorDomain -1022`.
- **F5.** `url` is the address the engine failed, when it is an allowed I2P address. Otherwise the page has no `url`: it shows no address, and Try again is off. A non-I2P address never reaches the page. The tab's own address is no fallback: during Back, Forward or Reload it is still the page the user was leaving.
- **F6.** A failed load makes no history entry. The error page takes the place of the failed load in the tab's back/forward list: a failure after the commit replaces the entry of that address, so Back goes to the page before the failed one. A failure for a tab that is not loading a web page (it shows an internal page, or the load already ended) changes nothing. The failure also clears the "Loading …" status bubble.
- **F7.** The page says that the page did not load, shows the address, a sentence for the reason and the code, and has **Try again**, **Go back**, and the "Report this problem" link of kind `load-failed`. Try again is the `reload` command: on the error page, Reload loads its address again in place of the error page, so no error entry stays in the list. Go back is the `go_back` command, so it steps the tab's own list.
- **F8.** A failure from a webview that no longer belongs to the tab (the tab built a new webview, for example for another JavaScript choice) is ignored.
- **F9.** When the engine filter (L3b) of a new tab webview cannot be attached, that webview never loads a page. The tab stops loading and shows the load-failed page with `reason` `blocked` and `code` `eepview engine-filter`, for the newest address that waited for the filter. The next load in the tab, Try again too, builds a new webview, which tries the filter again. See [ADR 0001](adr-0001-no-leak-architecture.md) (L3b).

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

eepview shows router information but never changes the router configuration. It finds the console of the router in use (Java I2P or i2pd) on loopback and gives one link to it, "I2P Router Console". The link shows in the router panel, on the home page and in Settings. It opens the console home page in the console tab. The console tab is a tab of its own kind in the tab strip, and it shows the `console` webview in the content area. There is at most one console tab. Back, forward, reload and stop work in it. The address bar shows the console URL with a `Router console` badge. With no console, one line says "No router console found". Details: [Router console](router-console.md).

## How to use / run locally

- `npm run tauri dev` with an I2P router on `127.0.0.1:4444`.
- `EEPVIEW_PROXY=127.0.0.1:<port>` picks another router proxy. `EEPVIEW_START_URL`, `EEPVIEW_EXIT_AFTER`, and `EEPVIEW_JS=off` are listed in the [IPC contract](ipc-contract.md#environment-and-command-line).
- Tests: `cargo test --workspace` in `src-tauri`.

## Limits

- Find on Windows needs a `WebView2` runtime with the native find API (`ICoreWebView2Find`). On an older runtime, find finds nothing and shows no count. No find runs a script in the page's own JavaScript world: there a page could read every query and fake the count.
- The Linux engine filter (L3b) waits for a webkit2gtk binding; the page policy (L3a) holds there.
- On macOS the engine draws no background of its own once a colour is set, so a page with no background shows the window colour until the tab returns to the engine default (UX batch 1, item 2).
- HTTPS eepsites load only on Windows: TLS tunnels stay closed on macOS and Linux (ADR 0001).
- The first-run setup flow and router control are Phase 2 and Phase 3.

## History

- 2026-10-03 — UX batch 1: focus after address-bar navigation, theme background, failed pages kept out of history, new tab focus, Settings proxy, Home router version — [#57](https://github.com/tcivie/eepview/pull/57)
- 2026-10-03 — Browser shell: tabs, navigation, bookmarks, history, find, gatekeeper, pause and resume — [#29](https://github.com/tcivie/eepview/pull/29)
- 2026-10-03 — Site icons in tabs, bookmarks and history — [#53](https://github.com/tcivie/eepview/pull/53)
- 2026-10-03 — Toolbar popups in their own `popup` webview; the toolbar stays 84 px — [#55](https://github.com/tcivie/eepview/pull/55)
- 2026-10-03 — Router console quick links and the console window — [#54](https://github.com/tcivie/eepview/pull/54)
- 2026-10-03 — The address bar refuses a dot host with a port instead of panicking; the gatekeeper refuses a bare CR or LF in a head — [#69](https://github.com/tcivie/eepview/pull/69)
- 2026-10-03 — The router console opens in a console tab, with one console link — [#76](https://github.com/tcivie/eepview/pull/76)
- 2026-10-06 — Root cause of "no eepsite loads, the tab spins forever" on macOS: ATS refused every `http://` load in the `.app` bundle before WebKit opened a socket (`NSURLErrorDomain -1022`). wry does not report a failed provisional load, so the tab never stopped loading. The leak test ran the bare binary, which has no `Info.plist`, so CI missed it. Fix: `NSAllowsArbitraryLoadsInWebContent` in `src-tauri/Info.plist` (B1), and the macOS leak test runs the `.app` — [#85](https://github.com/tcivie/eepview/pull/85)
- 2026-10-06 — Failed loads (F1 to F8): a load that the engine fails ends the tab load and shows `eepview://load-failed`. Root cause of the endless spinner: wry 0.57 passes no failed navigation to Tauri on macOS; a delegate relay in `eepview-platform` now reports it — [#87](https://github.com/tcivie/eepview/pull/87)
- 2026-10-09 — F9: a tab whose engine filter failed never loads a page and shows the load-failed page. Find on Windows uses the native `WebView2` find API, not a page script — [#92](https://github.com/tcivie/eepview/pull/92)
