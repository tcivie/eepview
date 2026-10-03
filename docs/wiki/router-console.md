<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# Router console

Status: in progress in [#54](https://github.com/tcivie/eepview/pull/54).

eepview shows router information. It does not change the router configuration. All router configuration goes through the router's own console pages. eepview finds the console of the router in use and gives quick links to it.

## How it works

1. **Detect.** When a page that shows the links opens, eepview reads the console port from the router configuration files, when it finds them. Then it sends one loopback `GET` to each candidate port. A port counts only when the answer looks like that router's console.
2. **Show.** The quick links show in the router panel, on the home page and in Settings. With no console, they are hidden and one line says "No router console found".
3. **Open.** A quick link opens the `console` view: a webview in its own window, `Router console`. It loads only the detected console origin, `http://127.0.0.1:<port>`.

The console view is not a web tab. It is built in `src-tauri/src/shell/console.rs` only. It never uses the gatekeeper, and the gatekeeper never sees it. An engine rule list confines it to the console origin (R19). The `tab-*` webviews keep all five layers and still cannot reach loopback. [ADR 0001](adr-0001-no-leak-architecture.md#router-console-exception) records this exception.

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
- **R6 On demand.** eepview never probes at start. Detection runs only on `console_detect()`. The UI calls it when the router panel opens, and when the home page or the Settings page loads while it is the active tab. While a console is known, eepview checks it again every 10 s (R21). When a trigger finds none, eepview retries (R20). A run that never shows these pages, such as the leak test with `EEPVIEW_START_URL`, opens no extra socket. When the result changes, eepview emits `console-changed`. When the console goes away (R21) or moves to another port, the console window closes.

### Pages

- **R7 Page map.** The quick links, in this order, map to these paths. A dash means the router has no such page, and the link is not shown.

  | Page key | Label | Java I2P | i2pd |
  |---|---|---|---|
  | `home` | Console | `/home` | `/` |
  | `tunnels` | Tunnels | `/tunnels` | `/?page=tunnels` |
  | `addressbook` | Address book | `/dns` | — |
  | `config` | Config | `/config` | `/?page=commands` |
  | `logs` | Logs | `/logs` | — |

  i2pd has no address book page and no log page in its web console. Its settings that change at run time are on the "Router commands" page.

### The console view

- **R8 One factory.** Only `src-tauri/src/shell/console.rs` builds the console webview. Its constructor takes a `&VerifiedConsole`. The webview label is exactly `console`, in a window labelled `console-window`. It is never a `tab-*` webview and never uses the content-webview factory.
- **R9 Origin.** The console view loads only URLs on the detected origin: scheme `http`, host `127.0.0.1`, the detected port.
- **R10 Navigation guard.** For each navigation in the console view:
  - same origin: it stays in the console view;
  - `http(s)://*.i2p`: the navigation is cancelled, and the URL opens in a new normal tab through the existing guard;
  - anything else: cancelled.

  A new-window request on the same origin loads in the console view. One for `*.i2p` opens a new tab. Anything else is dropped. The engine never opens a window by itself.
- **R11 Hardening.** The console view has no `proxy_url` (it is loopback), the engine rule list of R19, no IPC capability, WebRTC off in every frame (the same script as the tabs), downloads refused, and JavaScript on (the console needs it). It runs incognito, so nothing persists after exit.
- **R12 `console_open(page)`.** With no console: `{ok: false, reason: "no-console"}`. For a page the router does not have: `{ok: false, reason: "no-page"}`. For an unknown page key: an error. Otherwise it opens the console window, or reuses the open one, loads the page, focuses it and answers `{ok: true}`.
- **R13 `console_status()`.** It answers the current `ConsoleInfo`. It does not probe.

### Web tabs stay closed to loopback

- **R14 Tabs unchanged.** The five layers of the `tab-*` webviews do not change. A `tab-*` webview never receives a loopback URL. `proxy_url` stays only in `content.rs`. No capability names the `console` webview.

### UI

- **R15 Quick links.** The router panel and the home page show the links of R7 for the detected router, in that order. They are buttons that call `console_open`. No page holds an `http://127.0.0.1` link. With no console, no link shows and one line says "No router console found".
- **R16 No router configuration in eepview.** No eepview page changes router configuration. The Settings page has no bandwidth, share, relay (transit) or subscription control. Its Router section links to the console Config page instead. Pause and resume of the connection stay. The Router updates and Restore controls stay: they are managed-install controls (a disabled Phase 3 preview), not router configuration. The About list "Where eepview connects" names the console probe and the console view.
- **R17 Leak test.** The leak test passes unchanged.

### Router version

- **R18 Version, display only.** `ConsoleInfo.version` is the router version read from the probe page, or `null` when it is not found. It costs no extra request. Java I2P: the version in the console stylesheet link, `console.css?<version>`. i2pd: the first `:</b> <version><br>` value, where `<version>` is digits and dots (the label before it is translated). The router panel and the home page show it when `RouterStatus.version` is `null`. It changes nothing else.

### Confinement and retries

- **R19 Console rule list.** Before its first load, the console view gets an engine rule list: block every URL, then allow only URLs on the console origin (R9) and `about:`, `data:`, `blob:`. The view starts on `about:blank` and loads the page only after the list is attached. If it cannot be attached, nothing loads (fail closed). On Windows the same rule answers each `WebResourceRequested` with 403. Linux has no engine filter yet (the same limit as L3b for tabs); there the console view relies on R10 and the router's own pages.
- **R20 Retry after a miss.** When a `console_detect()` finds no console, eepview retries every 10 s for 2 minutes (12 retries), and stops at the first console found. A new trigger during the retries does not start a second retry loop. The UI also calls `console_detect()` when `router-status` turns `ok` (from any other state, not paused) while a page with the links shows: the home page or Settings as the active tab, or the open router panel.
- **R21 Re-check misses.** While a console is known, a re-check runs every 10 s. A re-check that finds a different console (another port or type) replaces it at once. A re-check that finds none counts a miss; the known console stays, with no event, until 3 misses in a row. The third miss clears it: `console-changed` with `found: false`, and the console window closes. A re-check that finds the same console resets the count.

## Interface

### Rust, `eepview_lib::net::console` (`src-tauri/src/net/console.rs`)

```rust
pub const JAVA_DEFAULT_PORT: u16 = 7657;
pub const I2PD_DEFAULT_PORT: u16 = 7070;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]                        // "java" | "i2pd"
pub enum ConsoleKind { Java, I2pd }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]       // "home" | "tunnels" | "addressbook" | "config" | "logs"
pub enum ConsolePage { Home, Tunnels, AddressBook, Config, Logs }
impl ConsolePage {
    pub const ALL: [ConsolePage; 5];                      // the R7 order
    pub fn parse(key: &str) -> Option<ConsolePage>;       // the page key
    pub fn key(self) -> &'static str;
}

pub fn page_path(kind: ConsoleKind, page: ConsolePage) -> Option<&'static str>;   // R7
pub fn probe_path(kind: ConsoleKind) -> &'static str;                            // R3

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
    pub fn url(&self, page: ConsolePage) -> Option<Url>;  // origin + page_path
    pub fn pages(&self) -> Vec<ConsolePage>;              // the pages this router has, R7 order
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
                         pub origin: Option<String>, pub pages: Vec<ConsolePage>,
                         pub version: Option<String> }
impl ConsoleInfo { pub fn none() -> ConsoleInfo; }  // found false, kind, origin and version null, no pages
```

Test helper, `crate::net::testing` (tests only): `FakeConsole::start(kind: ConsoleKind) -> FakeConsole` serves a page with that console's marker and the version `2.13.0` (Java: `console.css?2.13.0`; i2pd: `<b>Version:</b> 2.13.0<br>`) on a free loopback port. `FakeConsole::port(&self) -> u16`. `FakeConsole::verified(&self) -> VerifiedConsole`. `FakeConsole::requests(&self) -> Vec<String>` gives the request lines received.

### Rust, `eepview_lib::shell::console` (`src-tauri/src/shell/console.rs`)

```rust
pub const CONSOLE_LABEL: &str = "console";
pub const CONSOLE_WINDOW: &str = "console-window";

pub struct ConsoleWebview;
impl ConsoleWebview {
    /// Opens or reuses the console window and loads `page`.
    pub fn open<R: Runtime>(app: &AppHandle<R>, console: &VerifiedConsole, page: ConsolePage)
        -> tauri::Result<Webview<R>>;
}
/// Stores a detection result: emits `console-changed` when it changed, and closes the
/// console window when the console went away or moved.
pub fn set_console<R: Runtime>(app: &AppHandle<R>, console: Option<VerifiedConsole>);
pub const RETRY_EVERY: Duration = Duration::from_secs(10);   // R20, R21
pub const RETRY_FOR: Duration = Duration::from_secs(120);    // R20
/// Probes now (blocking), stores the result with `set_console`, and answers its info.
pub fn detect_now<R: Runtime>(app: &AppHandle<R>) -> ConsoleInfo;
/// The stored detection result.
pub fn current<R: Runtime>(app: &AppHandle<R>) -> Option<VerifiedConsole>;
/// Closes the console window, if open.
pub fn close<R: Runtime>(app: &AppHandle<R>);
```

### IPC (contract v1.4)

- `console_status() -> ConsoleInfo`: the stored result, no probe.
- `console_detect() -> ConsoleInfo`: probes now (R6), off the main thread, stores and answers the result.
- `console_open({page: ConsolePage}) -> {ok: boolean, reason?: "no-console" | "no-page"}`. The command takes the page key as a string and answers an error when `ConsolePage::parse` refuses it.
- Event `console-changed: ConsoleInfo`

```ts
type ConsolePage = "home" | "tunnels" | "addressbook" | "config" | "logs";
type ConsoleInfo = { found: boolean; kind: "java" | "i2pd" | null; origin: string | null;
  pages: ConsolePage[]; version: string | null };
```

### TypeScript, `src/ui/lib/console-links.ts` (pure)

```ts
export const NO_CONSOLE_TEXT = "No router console found";
export const CONSOLE_PAGE_ORDER: readonly ConsolePage[];        // R7 order
export const CONSOLE_PAGE_LABELS: Record<ConsolePage, string>;  // R7 labels
export interface ConsoleLinkView { page: ConsolePage; label: string }
export interface ConsoleLinksView {
  found: boolean;
  note: string | null;        // NO_CONSOLE_TEXT when not found, else null
  title: string | null;       // "Java I2P console" | "i2pd web console" | null
  links: ConsoleLinkView[];   // only the pages in info.pages, in R7 order; [] when not found
}
export function consoleLinks(info: ConsoleInfo | null | undefined): ConsoleLinksView;
/** R18: the version to show. statusVersion first; else the console version; else null. */
export function routerVersion(statusVersion: string | null, info: ConsoleInfo | null | undefined): string | null;
/** R20: true when next is ok, not paused, and prev was not ok (or not known). */
export function shouldRedetect(prev: RouterStatus | null, next: RouterStatus): boolean;
```

### Where the requirement tests live

- `src-tauri/src/net/console/tests.rs` (R1–R7, R9, R10, R18, R19, R21)
- `src-tauri/src/shell/console/tests.rs` (R6, R8, R10–R13, R20, R21; the mock runtime fixtures in `crate::shell::testing`)
- `src-tauri/tests/architecture.rs` (R4, R8, R11, R14)
- `src/ui/lib/console-links.test.ts` (R15, R18, R20)

## Limits

- A console behind a password (Java I2P console password, i2pd `http.auth`) is not detected.
- i2pd with a `webroot` other than `/`, or with `http.address` other than `127.0.0.1`, is not detected.
- Linux: the console view has no engine rule list yet (R19); see [No-leak architecture](no-leak-architecture.md).

## History

- 2026-10-03 — Router console detection, quick links and the console view — [#54](https://github.com/tcivie/eepview/pull/54)
