<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# Router console

Status: shipped in [#54](https://github.com/tcivie/eepview/pull/54). The console tab and the one link (R7, R12, R15, R23–R30): in progress on `feat/console-tab`.

eepview shows router information. It does not change the router configuration. All router configuration goes through the router's own console pages. eepview finds the console of the router in use and gives one link to it, "I2P Router Console". The user reaches every other console page from the console itself.

## How it works

1. **Detect.** When a page that shows the links opens, eepview reads the console port from the router configuration files, when it finds them. Then it sends one loopback `GET` to each candidate port. A port counts only when the answer looks like that router's console.
2. **Show.** The link shows in the router panel, on the home page and in Settings. With no console, it is hidden and one line says "No router console found".
3. **Open.** The link opens the console home page in the console tab: a tab of its own kind in the tab strip. Its webview, `console`, sits in the content area like a web tab. It loads only the detected console origin, `http://127.0.0.1:<port>`.

The console tab is not a web tab, and its webview is not a `tab-*` webview. The webview is built in `src-tauri/src/shell/console.rs` only. It never uses the gatekeeper, and the gatekeeper never sees it. An engine rule list confines it to the console origin (R19). The `tab-*` webviews keep all five layers and still cannot reach loopback. [ADR 0001](adr-0001-no-leak-architecture.md#router-console-exception) records this exception.

## Requirements

### Detection

- **R1 Candidate ports.** The candidates are, in this order, with no duplicates:
  1. the Java I2P console ports read from the Java I2P configuration (R2);
  2. the i2pd web console port read from `i2pd.conf` (R2);
  3. the Java I2P default, `7657`;
  4. the i2pd default, `7070`.
- **R2 Configuration files.**
  - Java I2P: in the configuration folder, every file in `clients.config.d/` (by file name), then `clients.config`. The console is the client whose `clientApp.<n>.main` is `net.i2p.router.web.RouterConsoleRunner`. Its port is the first whitespace-separated token of `clientApp.<n>.args` that parses fully as a non-zero `u16` and does not follow `-s` (for example `7657 ::1,127.0.0.1 ./webapps/` gives `7657`). The token after `-s` is a TLS port and is skipped, so the TLS-only `-s 7667 ::1,127.0.0.1 ./webapps/` gives no port; `127.0.0.1` is not a `u16` token. A client with `clientApp.<n>.startOnLoad=false` gives no port.
  - i2pd: the `port` key in the `[http]` section of `i2pd.conf`. `enabled = false` in that section gives no port. Keys outside `[http]` are ignored. `#` starts a comment.
  - Folders. Java I2P: macOS `$HOME/Library/Application Support/i2p`; Linux `$HOME/.i2p` and `/var/lib/i2p/i2p-config`; Windows `%LOCALAPPDATA%\I2P` and `%APPDATA%\I2P`. i2pd: macOS `$HOME/Library/Application Support/i2pd/i2pd.conf`; Linux `$HOME/.i2pd/i2pd.conf` and `/etc/i2pd/i2pd.conf`; Windows `%APPDATA%\i2pd\i2pd.conf`. A path whose variable is not set is left out. A missing or unreadable file gives no port.
- **R3 Verification.** A candidate counts only when one `GET` to `http://127.0.0.1:<port><probe path>` answers `200` and the body has the marker of that console:

  | Router | Probe path | Marker (all must be in the body) |
  |---|---|---|
  | Java I2P | `/home` | `/themes/console/` and `console.css` |
  | i2pd | `/` | `?page=i2p_tunnels` |

  A closed port, another status, a body without the marker, or the console of the other router type does not count. The markers do not depend on the console language.
- **R4 Loopback only.** The probe connects only through `LoopbackAddr` to `127.0.0.1`, with a 5 s timeout. It sends one `GET <path> HTTP/1.0` in origin form, with `Host: 127.0.0.1:<port>`, so the answer is never chunked. It never follows a redirect. The probe code lives in `src-tauri/src/net/` (ADR 0001 rule 2).
- **R5 Result.** The first candidate that passes R3 is the console. If none passes, there is no console. Only the detector makes a `VerifiedConsole`.
- **R6 On demand.** eepview never probes at start. Detection runs only on `console_detect()`. The UI calls it when the router panel opens, and when the home page or the Settings page loads while it is the active tab. While a console is known, eepview checks it again every 10 s (R21). When a trigger finds none, eepview retries (R20). A run that never shows these pages, such as the leak test with `EEPVIEW_START_URL`, opens no extra socket. When the result changes, eepview emits `console-changed`. When the console goes away (R21) or moves to another port, the console tab closes (R30).

### The console page

- **R7 Home page.** The link opens the console home page. It is the probe page of R3:

  | Router | Home page |
  |---|---|
  | Java I2P | `/home` |
  | i2pd | `/` |

  eepview links to no other console page. The user reaches the tunnels, the address book, the configuration and the logs from the console itself.

### The console view

- **R8 One factory.** Only `src-tauri/src/shell/console.rs` builds the console webview. Its constructor takes a `&VerifiedConsole`. The webview label is exactly `console`. It is a child of the `main` window, like the tab webviews, and there is no other console window. It is never a `tab-*` webview, it is never in the tab-webview label map, and it never uses the content-webview factory.
- **R9 Origin.** The console view loads only URLs on the detected origin: scheme `http`, host `127.0.0.1`, the detected port.
- **R10 Navigation guard.** For each navigation in the console view:
  - same origin: it stays in the console view;
  - `http(s)://*.i2p`: the navigation is cancelled, and the URL opens in a new normal tab through the existing guard;
  - anything else: cancelled.

  A new-window request on the same origin loads in the console view. One for `*.i2p` opens a new tab. Anything else is dropped. The engine never opens a window by itself.
- **R11 Hardening.** The console view has no `proxy_url` (it is loopback), the engine rule list of R19, no IPC capability, WebRTC off in every frame (the same script as the tabs), downloads refused, and JavaScript on (the console needs it). It runs incognito, so nothing persists after exit.
- **R12 `console_open()`.** It takes no argument. With no console: `{ok: false, reason: "no-console"}`, and no tab and no webview are made. Otherwise it opens the console tab (R24), loads the console home page (R7) in it, and answers `{ok: true}`.
- **R13 `console_status()`.** It answers the current `ConsoleInfo`. It does not probe.

### Web tabs stay closed to loopback

- **R14 Tabs unchanged.** The five layers of the `tab-*` webviews do not change. A `tab-*` webview never receives a loopback URL. `proxy_url` stays only in `content.rs`. No capability names the `console` webview.

### UI

- **R15 One link.** The router panel, the home page and the Router section of Settings show one link, "I2P Router Console", for the detected router. It is a button that calls `console_open()`. No page holds an `http://127.0.0.1` link. With no console, the link is hidden and one line says "No router console found". The five per-page links of #54 (Console, Tunnels, Address book, Config, Logs) are gone.
- **R16 No router configuration in eepview.** No eepview page changes router configuration. The Settings page has no bandwidth, share, relay (transit) or subscription control. Its Router section has the one console link (R15) instead. Pause and resume of the connection stay. The Router updates and Restore controls stay: they are managed-install controls (a disabled Phase 3 preview), not router configuration. The About list "Where eepview connects" names the console probe and the console tab.
- **R17 Leak test.** The leak test passes unchanged.

### Router version

- **R18 Version, display only.** `ConsoleInfo.version` is the router version read from the probe page, or `null` when it is not found. It costs no extra request. Java I2P: the version in the console stylesheet link, `console.css?<version>`. i2pd: the first `:</b> <version><br>` value, where `<version>` is digits and dots (the label before it is translated). The router panel and the home page show it when `RouterStatus.version` is `null`. It changes nothing else.

### Confinement and retries

- **R19 Console rule list.** Before its first load, the console view gets an engine rule list: block every URL, then allow only URLs on the console origin (R9) and `about:`, `data:`, `blob:`. The view starts on `about:blank` and loads the page only after the list is attached. If it cannot be attached, nothing loads (fail closed). On Windows the same rule answers each `WebResourceRequested` with 403. Linux has no engine filter yet (the same limit as L3b for tabs); there the console view relies on R10 and the router's own pages.
- **R20 Retry after a miss.** When a `console_detect()` finds no console, eepview retries every 10 s for 2 minutes (12 retries), and stops at the first console found. A new trigger during the retries does not start a second retry loop. The UI also calls `console_detect()` when `router-status` turns `ok` (from any other state, not paused) while a page with the links shows: the home page or Settings as the active tab, or the open router panel.
- **R21 Re-check misses.** While a console is known, a re-check runs every 10 s. A re-check that finds a different console (another port or type) replaces it at once. A re-check that finds none counts a miss; the known console stays, with no event, until 3 misses in a row. The third miss clears it: `console-changed` with `found: false`, and the console tab closes (R30). A re-check that finds the same console resets the count.
- **R22 Stop.** `shell::console::stop(app)` ends every re-check and retry loop at its next tick (at most one tick, 10 s). After stop, no thread opens a connection to a console port. The shell calls it on `RunEvent::Exit`. A later `detect_now` starts the loops again.

### The console tab

- **R23 Tab kind.** The console tab is a tab of kind `console` in the tab strip (`TabInfo.kind`). While it is the active tab, the content area shows the `console` webview, at the place of a web tab. While another tab is active, the `console` webview is hidden. Its `TabInfo` has `kind: "console"`, `zoom: 1`, `jsOn: true` (R11), `bookmarked: false` and `icon: null`. The router state (verifying, down, paused) does not change what the console tab shows: the console is on loopback and does not go through the router proxy.
- **R24 One console tab.** There is at most one console tab. `console_open()` with a console tab open selects it and loads the home page in it. With none, it makes one right after the active tab and selects it. It never opens a second `console` webview.
- **R25 Tab state.** The tab title is the document title of the console page. Until the first title arrives, it is `Router console`. The tab URL is the URL the `console` webview shows (main frame), and `loading` follows its page loads. A console page never enters history, never gets a site icon and is never bookmarked. The bookmark star and the JavaScript toggle are disabled in a console tab. Find in page and zoom do nothing there.
- **R26 Address bar.** In a console tab the address bar shows the console URL and a `Router console` badge in place of the `I2P` badge. Typing in the address bar and pressing Enter navigates like in any tab: the input goes through the normal address rules and the tab guard. The console tab becomes a normal tab (`web` or `internal`) with a new back/forward list, and the `console` webview is destroyed. A console address typed by hand is refused like any other non-I2P address (`not-i2p`); only `console_open()` loads the console.
- **R27 Back, forward, reload, stop.** In a console tab, back, forward, reload, hard reload and stop act on the `console` webview. `canBack` and `canForward` follow the console pages loaded in that tab.
- **R28 Close.** Closing the console tab destroys the `console` webview. "Reopen closed tab" never brings a console tab back. eepview does not restore tabs at start, so a restart never shows a console tab.
- **R29 No console URL in a web tab.** The core never makes a `WebOp` (load, engine call, destroy) for the console tab. A console page URL never reaches a `tab-*` webview: a load of a non-I2P URL in a web tab shows the blocked page, as before. ADR 0001 and the leak test do not change.
- **R30 Console gone.** When the console goes away or moves (R6, R21), the console tab closes and the `console` webview is destroyed.

## Interface

### Rust, `eepview_lib::net::console` (`src-tauri/src/net/console.rs`)

```rust
pub const JAVA_DEFAULT_PORT: u16 = 7657;
pub const I2PD_DEFAULT_PORT: u16 = 7070;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]                        // "java" | "i2pd"
pub enum ConsoleKind { Java, I2pd }

pub fn probe_path(kind: ConsoleKind) -> &'static str;                            // R3, R7: also the home page

pub fn java_console_port(clients_config: &str) -> Option<u16>;                   // R2, one file's text
pub fn i2pd_console_port(i2pd_conf: &str) -> Option<u16>;                        // R2, one file's text
pub fn java_config_dirs(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf>;   // R2, this OS
pub fn i2pd_config_files(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf>;  // R2, this OS
pub fn java_ports_in(dir: &Path) -> Vec<u16>;    // clients.config.d/* by name, then clients.config
pub fn i2pd_port_in(file: &Path) -> Option<u16>;
pub fn candidates(env: &dyn Fn(&str) -> Option<String>) -> Vec<(ConsoleKind, u16)>;  // R1

pub fn judge(kind: ConsoleKind, answer: &crate::net::verify::Answer) -> bool;    // R3
pub fn probe(kind: ConsoleKind, port: u16) -> Option<VerifiedConsole>;           // R3, R4
pub fn detect(candidates: &[(ConsoleKind, u16)]) -> Option<VerifiedConsole>;     // R5
pub fn detect_here() -> Option<VerifiedConsole>;  // detect(&candidates(<process environment>))

/// A console that passed R3. Private fields: only `probe` and `detect` make one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedConsole { /* private */ }
impl VerifiedConsole {
    pub fn kind(&self) -> ConsoleKind;
    pub fn port(&self) -> u16;
    pub fn origin(&self) -> String;                       // "http://127.0.0.1:<port>"
    pub fn home(&self) -> Url;                            // R7: origin + probe_path
    pub fn route(&self, url: &Url) -> ConsoleNav;         // R10
    pub fn version(&self) -> Option<&str>;               // R18
    pub fn info(&self) -> ConsoleInfo;
    pub fn rule_list(&self) -> serde_json::Value;         // R19, WKContentRuleList JSON
    pub fn engine_allows(&self, url: &str) -> bool;       // R19, the same rule for one URL
}

pub const RECHECK_MISSES: u32 = 3;                                                // R21
/// R21: the known console and miss count after one re-check that found `found`.
pub fn after_recheck(known: Option<&VerifiedConsole>, misses: u32,
                     found: Option<VerifiedConsole>) -> (Option<VerifiedConsole>, u32);

pub fn console_version(kind: ConsoleKind, body: &str) -> Option<String>;          // R18

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleNav { Stay, OpenTab, Cancel }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]   // camelCase on the wire
pub struct ConsoleInfo { pub found: bool, pub kind: Option<ConsoleKind>,
                         pub origin: Option<String>, pub version: Option<String> }
impl ConsoleInfo { pub fn none() -> ConsoleInfo; }  // found false, kind, origin and version null
```

Test helper, `crate::net::testing` (tests only): `FakeConsole::start(kind: ConsoleKind) -> FakeConsole` serves a page with that console's marker and the version `2.13.0` (Java: `console.css?2.13.0`; i2pd: `<b>Version:</b> 2.13.0<br>`) on a free loopback port. `FakeConsole::port(&self) -> u16`. `FakeConsole::verified(&self) -> VerifiedConsole`. `FakeConsole::requests(&self) -> Vec<String>` gives the request lines received.

### Rust, `eepview_lib::core` (pure; the console tab)

```rust
// crate::tabs::Tab gets one field:
pub console: bool;                    // R23: this is the console tab

// crate::core
pub enum View { Internal(String), Web(u32), Console(u32) }   // Console: the console tab id
pub enum Effect { /* as before */ Console(ConsoleOp) }        // R27, R28: act on the `console` webview
#[derive(Debug, Clone, PartialEq)]
pub enum ConsoleOp {
    Engine(EngineOp),   // Back, Forward, Reload, HardReload, Stop (never Find, FindClear, Zoom)
    Close,              // destroy the `console` webview
}
impl Core {
    /// The id of the console tab, if one is open.
    pub fn console_tab(&self) -> Option<u32>;
    /// R24: selects the console tab, or makes one right after the active tab and selects
    /// it, showing `url`. The shell has already started the load. No `WebOp`.
    pub fn console_open(&mut self, url: &str) -> Vec<Effect>;
    /// R25: the `console` webview started (`console_started`) or finished
    /// (`console_finished`) a main-frame load of `url`. Updates url, loading and the
    /// back/forward flags of the console tab. No history.
    pub fn console_started(&mut self, url: &str) -> Vec<Effect>;
    pub fn console_finished(&mut self, url: &str) -> Vec<Effect>;
    /// R25: the document title of the console page changed.
    pub fn console_title(&mut self, title: &str) -> Vec<Effect>;
    /// R30: the console went away: closes the console tab, if open (with `ConsoleOp::Close`).
    pub fn console_gone(&mut self) -> Vec<Effect>;
}
```

The other commands keep their signatures and branch on the console tab: `view()` (R23), `tab_infos()` (`kind: "console"`, R23), `navigate()` (R26), `step()`, `reload()`, `stop()` (R27), `tab_close()`, `tab_reopen()` (R28), `find()`, `zoom()`, `bookmark_toggle()` (R25).

### Rust, `eepview_lib::shell::console` (`src-tauri/src/shell/console.rs`)

```rust
pub const CONSOLE_LABEL: &str = "console";

pub struct ConsoleWebview;
impl ConsoleWebview {
    /// R8, R24: builds the `console` webview in the main window (hidden, at the content
    /// rect), or reuses it, loads the home page of `console` (after the rule list, R19),
    /// and opens or selects the console tab (`Core::console_open`).
    pub fn open<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole)
        -> tauri::Result<Webview<R>>;
}
/// Stores a detection result: emits `console-changed` when it changed, and closes the
/// console tab when the console went away or moved (R30).
pub fn set_console<R: Runtime>(app: &AppHandle<R>, console: Option<VerifiedConsole>);
pub const RETRY_EVERY: Duration = Duration::from_secs(10);   // R20, R21
pub const RETRY_FOR: Duration = Duration::from_secs(120);    // R20
/// Probes now (blocking), stores the result with `set_console`, and answers its info.
pub fn detect_now<R: Runtime>(app: &AppHandle<R>) -> ConsoleInfo;
/// The stored detection result.
pub fn current<R: Runtime>(app: &AppHandle<R>) -> Option<VerifiedConsole>;
/// R10: the navigation guard of the console view; its `on_navigation` callback is a
/// one-line call of this. True for the console origin (and the first `about:blank`). An
/// `http(s)://*.i2p` URL answers false and opens a new normal tab through the tab guard.
/// Anything else answers false.
pub fn navigation<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole, url: &Url) -> bool;
/// R10: the new-window handler of the console view; its `on_new_window` callback is a
/// one-line call of this. Always `Deny`: the console origin loads in the console view, an
/// `http(s)://*.i2p` URL opens a new normal tab, anything else does nothing.
pub fn new_window<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole, url: &Url)
    -> NewWindowResponse<R>;
/// R25: a main-frame page load of the console view; its `on_page_load` callback is a
/// one-line call of this. Only a URL on the console origin reaches the core
/// (`Core::console_started` / `Core::console_finished`); any other URL is ignored.
pub fn page_load<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole,
                             event: PageLoadEvent, url: &Url);
/// R25: the document title changed; its `on_document_title_changed` callback is a
/// one-line call of this (`Core::console_title`).
pub fn title_changed<R: Runtime>(app: &AppHandle<R>, title: &str);
/// R27, R28: carries out a `ConsoleOp` on the `console` webview (nothing when none exists).
pub fn run<R: Runtime>(app: &AppHandle<R>, op: &ConsoleOp);
/// R22: ends the re-check and retry loops at their next tick.
pub fn stop<R: Runtime>(app: &AppHandle<R>);
/// R30: closes the console tab (if open) and destroys the `console` webview (if any).
pub fn close<R: Runtime>(app: &AppHandle<R>);
```

`shell::view::sync` shows the `console` webview for `View::Console`, at the content rect, and hides it for every other view (R23).

### IPC (contract v1.6)

- `console_status() -> ConsoleInfo`: the stored result, no probe.
- `console_detect() -> ConsoleInfo`: probes now (R6), off the main thread, stores and answers the result.
- `console_open() -> {ok: boolean, reason?: "no-console"}` (R12). No argument.
- `TabInfo.kind`: `"internal" | "web" | "console"` (R23).
- Event `console-changed: ConsoleInfo`

```ts
type ConsoleInfo = { found: boolean; kind: "java" | "i2pd" | null; origin: string | null;
  version: string | null };
type ConsoleOpenResult = { ok: boolean; reason?: "no-console" };
```

### TypeScript, `src/ui/lib/console-links.ts` (pure)

```ts
export const NO_CONSOLE_TEXT = "No router console found";
export const CONSOLE_LINK_TEXT = "I2P Router Console";           // R15
export interface ConsoleLinkView {
  found: boolean;
  note: string | null;        // NO_CONSOLE_TEXT when not found, else null
  title: string | null;       // "Java I2P console" | "i2pd web console" | null
  label: string | null;       // CONSOLE_LINK_TEXT when found, else null
}
export function consoleLink(info: ConsoleInfo | null | undefined): ConsoleLinkView;
/** R18: the version to show. statusVersion first; else the console version; else null. */
export function routerVersion(statusVersion: string | null, info: ConsoleInfo | null | undefined): string | null;
/** R20: true when next is ok, not paused, and prev was not ok (or not known). */
export function shouldRedetect(prev: RouterStatus | null, next: RouterStatus): boolean;
```

### TypeScript, `src/ui/lib/address.ts` (pure)

```ts
export interface AddressBadge { text: string; title: string; kind: "i2p" | "console" }
/** R26: the badge before the address: "I2P" for a web tab, "Router console" for the
 *  console tab, none (null) for an internal page or no tab. */
export function addressBadge(tab: Pick<TabInfo, "kind" | "url"> | null | undefined): AddressBadge | null;
```

The `I2P` badge keeps its title, "Opened over I2P". The console badge title is "The router's own console on this computer".

### Where the requirement tests live

- `src-tauri/src/net/console/tests.rs` (R1–R7, R9, R10, R18, R19, R21)
- `src-tauri/src/shell/console/tests.rs` (R6, R8, R10–R13, R20–R30 in the shell; the mock runtime fixtures in `crate::shell::testing`)
- `src-tauri/src/core/console_tab/tests.rs` (R23–R29 in the core)
- `src-tauri/tests/architecture.rs` (R4, R8, R11, R14)
- `src/ui/lib/console-links.test.ts` (R15, R18, R20)
- `src/ui/lib/address.test.ts` (R26)

## Limits

- A console behind a password (Java I2P console password, i2pd `http.auth`) is not detected.
- i2pd with a `webroot` other than `/`, or with `http.address` other than `127.0.0.1`, is not detected.
- Linux: the console view has no engine rule list yet (R19); see [No-leak architecture](no-leak-architecture.md).
- Find in page and zoom do nothing in the console tab (R25).

## History

- 2026-10-03 — Router console detection, quick links and the console view — [#54](https://github.com/tcivie/eepview/pull/54)
