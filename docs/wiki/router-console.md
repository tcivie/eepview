<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# Router console

Status: shipped in [#54](https://github.com/tcivie/eepview/pull/54). The console tab and the one link (R7, R12, R15, R23–R30): in progress in [#76](https://github.com/tcivie/eepview/pull/76). Router statistics from the console (R31–R46): in progress in [#78](https://github.com/tcivie/eepview/pull/78).

eepview shows router information. It does not change the router configuration. All router configuration goes through the router's own console pages. eepview finds the console of the router in use and gives one link to it, "I2P Router Console". The user reaches every other console page from the console itself. When the router has no eepview helper, eepview also reads the router statistics from that console (R31–R46).

## How it works

1. **Detect.** When a page that shows the links opens, eepview reads the console port from the router configuration files, when it finds them. Then it sends one loopback `GET` to each candidate port. A port counts only when the answer looks like that router's console.
2. **Show.** The link shows in the router panel, on the home page and in Settings. With no console, it is hidden and one line says "No router console found".
3. **Open.** The link opens the console home page in the console tab: a tab of its own kind in the tab strip. Its webview, `console`, sits in the content area like a web tab. It loads only the detected console origin, `http://127.0.0.1:<port>`.
4. **Read the statistics.** Without a router helper, `router_stats()` reads the figures of the router panel and the Network page from the detected console: one read-only loopback `GET` per refresh, while those figures show (R31–R46).

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

- **R19 Console rule list.** Before its first load, the console view gets an engine rule list: block every URL, then allow only URLs on the console origin (R9) and `about:`, `data:`, `blob:`. The view starts on `about:blank` and loads the page only after the list is attached. If it cannot be attached, nothing loads (fail closed): the console tab closes, its `console` webview is destroyed, and a warning toast says "The router console could not be opened safely" (`Core::console_failed`). On Windows the same rule answers each `WebResourceRequested` with 403. Linux has no engine filter yet (the same limit as L3b for tabs); there the console view relies on R10 and the router's own pages.
- **R20 Retry after a miss.** When a `console_detect()` finds no console, eepview retries every 10 s for 2 minutes (12 retries), and stops at the first console found. A new trigger during the retries does not start a second retry loop. The UI also calls `console_detect()` when `router-status` turns `ok` (from any other state, not paused) while a page with the links shows: the home page or Settings as the active tab, or the open router panel.
- **R21 Re-check misses.** While a console is known, a re-check runs every 10 s. A re-check that finds a different console (another port or type) replaces it at once. A re-check that finds none counts a miss; the known console stays, with no event, until 3 misses in a row. The third miss clears it: `console-changed` with `found: false`, and the console tab closes (R30). A re-check that finds the same console resets the count.
- **R22 Stop.** `shell::console::stop(app)` ends every re-check and retry loop at its next tick (at most one tick, 10 s). After stop, no thread opens a connection to a console port. The shell calls it on `RunEvent::Exit`. A later `detect_now` starts the loops again.

### The console tab

- **R23 Tab kind.** The console tab is a tab of kind `console` in the tab strip (`TabInfo.kind`). While it is the active tab, the content area shows the `console` webview, at the place of a web tab. While another tab is active, the `console` webview is hidden. Its `TabInfo` has `kind: "console"`, `zoom: 1`, `jsOn: true` (R11), `bookmarked: false` and `icon: null`. The router state (verifying, down, paused) does not change what the console tab shows: the console is on loopback and does not go through the router proxy.
- **R24 One console tab.** There is at most one console tab. `console_open()` with a console tab open selects it and loads the home page in it. With none, it makes one right after the active tab and selects it. It never opens a second `console` webview. One field of the core, `console_tab`, names the console tab; a tab has no console flag.
- **R25 Tab state.** The tab title is the document title of the console page. Until the first title arrives, it is `Router console`. The tab URL is the URL the `console` webview shows (main frame), and `loading` follows its page loads. A console page never enters history, never gets a site icon and is never bookmarked. The bookmark star and the JavaScript toggle are disabled in a console tab. Find in page and zoom do nothing there: the find shortcut and `find()` leave the find bar closed and make no engine call.
- **R26 Address bar.** In a console tab the address bar shows the console URL and a `Router console` badge in place of the `I2P` badge. Typing in the address bar and pressing Enter navigates like in any tab: the input goes through the normal address rules and the tab guard. When the input is allowed (an I2P address, an internal page or a search), the console tab becomes a normal tab (`web` or `internal`) with a new back/forward list, and the `console` webview is destroyed. When the input is refused (`not-i2p`, `invalid`), for example an edited console address, the answer is the refusal and the console tab stays as it is: no blocked page, no effect on the `console` webview. A console address typed by hand never loads; only `console_open()` loads the console.
- **R27 Back, forward, reload, stop.** In a console tab, back, forward, reload, hard reload and stop act on the `console` webview. `canBack` and `canForward` follow the console pages loaded in that tab.
- **R28 Close.** Closing the console tab destroys the `console` webview. "Reopen closed tab" never brings a console tab back. eepview does not restore tabs at start, so a restart never shows a console tab.
- **R29 No console URL in a web tab.** The core never makes a `WebOp` (load, engine call, destroy) for the console tab. A console page URL never reaches a `tab-*` webview: a load of a non-I2P URL in a web tab shows the blocked page, as before. ADR 0001 and the leak test do not change.
- **R30 Console gone.** When the console goes away or moves (R6, R21), the console tab closes and the `console` webview is destroyed.

### Router statistics from the console

The router panel and the Network page (`eepview://stats`) show the router statistics. Until now they came only from the router helper (`EEPVIEW_ROUTER_STATUS`), which an external router does not have. Java I2P keeps I2PControl off by default, and eepview never turns it on: that is a router configuration change. So, without the helper, eepview reads the same figures from the console that detection already verified. It only reads. It never changes the router.

- **R31 Source order.** `router_stats()` takes the statistics from the first source that answers:
  1. the router helper, when `EEPVIEW_ROUTER_STATUS` and its token are set, and it answers `200` with JSON;
  2. the console stored by detection (`shell::console::current`), when one is stored and it answers `200` (R32, R33);
  3. else no source: every field is `null`.

  When the helper answers, eepview does not ask the console. `router_stats()` never runs detection: with no stored console it does not probe. No page shows which source gave the figures.
- **R32 One read-only request.** The console request is one loopback `GET <path> HTTP/1.0` in origin form on the stored console origin (`127.0.0.1:<port>`), through `LoopbackAddr`, with the headers of R4 (`Host`, `User-Agent: eepview`, `Accept: text/html`, `Connection: close`) and nothing else: no cookie, no body. It never sends a `POST`, never follows a redirect, and never goes through a webview. The path is fixed per router type:

  | Router | Path |
  |---|---|
  | Java I2P | `/xhr1.jsp?requestURI=/summaryframe` |
  | i2pd | `/` |

  The path never carries a `lang`, `action` or `consoleNonce` parameter. (A Java I2P console saves `?lang=` in the router configuration.) The request code lives in `src-tauri/src/net/` (ADR 0001 rule 2). The Java path is the sidebar data that the console's own script loads every 15 s, with the section list of the sidebar page (`/summaryframe`). It has the least markup of the pages that carry the figures.
- **R33 Bounds.** The whole request, connect and reads together, takes at most 3 s. It reads at most 256 KiB of the page. A peer that sends slowly cannot hold a call longer than 3 s. A closed port, a timeout before any answer, a status other than `200`, or an answer that is not HTTP counts as "the console does not answer": R31 moves on. A body cut by the size cap or by the timeout is still parsed; R35 makes every cut value `null`.
- **R34 Cadence.** Rust starts no thread and no timer for the console statistics: each `router_stats()` call makes at most one console request. The UI calls `router_stats()` every 5 s, and only while the figures show:
  - the router panel: while it is open (unchanged);
  - the Network page: while `document.visibilityState` is `"visible"`. It stops while the page is hidden, and refreshes at once when it shows again.

  The Network page calls `console_detect()` once when it loads (a new trigger for R6). The router panel and the Network page load the figures again when that `console_detect()` answers, so a console found at that moment fills the figures at once. After `RunEvent::Exit`, no page calls `router_stats()`, so no console request runs. After `shell::console::stop` (R22), `router_stats()` asks no console either, until the next detection. A call that starts after the stop sends no request; a request already sent ends within 3 s (R33).
- **R35 Fail closed, per field.** Each field is parsed alone. A field that does not match its rule exactly is `null`, and the UI shows "—". eepview never shows a guessed or a derived number.
  - An integer is one or more ASCII digits and fits in `u64`. A decimal is ASCII digits, then optionally `.` and one or more digits. No sign, no group separator, no `,` as the decimal mark, no other digit set. A value that does not match is `null`.
  - A value must end with the terminator its rule names. A value at the end of the body (cut by R33) is `null`.
  - A unit conversion is exact on the decimal digits, then rounds to the nearest integer, a half rounds up. For example, `53.91` KBps is 53 910 B/s, and `12.34` KiB/s is 12 636 B/s (12 636.16).
  - When an anchor (a Java I2P table `id`, or an i2pd label) is in the body more than once, the fields it gives are `null`.
  - A missing section gives `null` for its fields only. The other fields still parse.
- **R36 Java I2P figures.** The parser reads tables by their `id` and their rows by position. It never reads a row label or a `title`, so it works in every console language. A row is a `<tr…>…</tr>` element in the table. Its value is the text of the last `<td…>` cell, up to `</td>`. When a table has another number of rows than the rule names, every field of that table is `null`.

  | Field | Table and row | Value form | Result |
  |---|---|---|---|
  | `uptimeMs`, `uptimeResolutionMs` | `sb_general` or `sb_shortgeneral` with 2 rows: row 1. Else `sb_advancedgeneral` with 4 rows: row 1, with 5 rows: row 2. The first of the three ids in this order wins. | `N&nbsp;<unit>` | R37 |
  | `bandwidthBytesPerSecond.in1s`, `.out1s` | `sb_bandwidth`, any row count from 2 to 4: row 0 | `A / B&nbsp;KBps` or `A / B&nbsp;MBps` | `A` and `B` (decimals) × 1 000 (`K`) or × 1 000 000 (`M`) |
  | `bandwidthBytesPerSecond.in5m`, `.out5m` | `sb_bandwidth` with 4 rows: row 1. With 2 or 3 rows: `null` (the router is younger than 6 minutes). | as above | as above |
  | `activePeers` | `sb_peers` with 5 rows, or `sb_peersadvanced` with 6 rows: row 0 | `A / B` | `A` (peers with a connection now) |
  | `floodfills` | the same table: row 3 | `N` | `N` |
  | `knownRouters` | the same table: row 4 | `N` | `N` |
  | `tunnels.exploratory` | `sb_tunnels` with 4 rows: row 0 | `N` | `N` (inbound + outbound) |
  | `tunnels.client` | `sb_tunnels` with 4 rows: row 1 | `N` | `N` (inbound + outbound) |
  | `tunnels.participating` | `sb_tunnels` with 4 rows: row 2 | `N` | `N` |
  | `networkStatus` | the class after `sb_netstatus` in the first `<span class="sb_netstatus <class>">` | `running`, `firewalled`, `testing`, `hidden`, `warn`, `error`, `clockskew`, `vmcomm` | `OK`, `FIREWALLED`, `TESTING`, `HIDDEN`, `WARN`, `ERROR`, `CLOCK_SKEW`, `VMCOMM`; another class gives `null` |

  `A / B` is `A`, a space, `/`, a space, `B`. The rows Fast and High capacity (rows 1 and 2 of `sb_peers`) have no field and are not read. Always `null` from Java I2P: `version` (R18 shows the console version), `tunnels.in`, `tunnels.out` (the sidebar gives only in + out together), and every `tunnelBuildSuccessPercent` field: the sidebar shows no build success figure, and eepview does not compute one from other statistics.
- **R37 Java I2P uptime.** The console rounds the uptime down to one unit. The value is `N&nbsp;<unit>` with `N` an integer. Only the English units parse:

  | Unit | `uptimeMs` | `uptimeResolutionMs` |
  |---|---|---|
  | `ms` | N | 1 |
  | `sec` | N × 1 000 | 1 000 |
  | `min` | N × 60 000 | 60 000 |
  | `hour`, `hours` | N × 3 600 000 | 3 600 000 |
  | `day`, `days` | N × 86 400 000 | 86 400 000 |

  Any other unit, such as `years` or a translated unit, gives `null` for both fields.
- **R38 i2pd figures.** This rule comes from the i2pd source, `daemon/HTTPServer.cpp` (`ShowStatus`, `ShowUptime`, `ShowNetworkStatus`), of i2pd 2.5x. No i2pd router was available to capture a real page. i2pd translates its labels and has no table ids, so the parser reads the English page only: when the body has no `<html lang="en"`, every field is `null`. A label is the exact text `<b><label>:</b> ` (a space after it). Lines may end with `\r\n` or `\n`.

  | Field | Label | Value form, then terminator | Result |
  |---|---|---|---|
  | `uptimeMs`, `uptimeResolutionMs` | `Uptime` | `D day(s), H hour(s), M minute(s), S second(s)`, then `<br>`. Each part is `<integer> <unit>`, singular or plural. Days, hours and minutes are optional; seconds is always there; the order is fixed; parts are joined by `, `. | the total in ms; resolution 1 000 |
  | `networkStatus` | `Network status` (not `Network status v6`) | exactly one of `OK`, `Firewalled`, `Unknown`, `Proxy`, `Mesh`, `Stan`, then `<br>` | the word in upper case. A ` (Testing)` or ` - <error>` suffix gives `null`. |
  | `tunnelBuildSuccessPercent.total` | `Tunnel creation success rate` | `N%`, then `<br>` | `N` |
  | `bandwidthBytesPerSecond.in1s` | `Received` | `<amount> (X KiB/s)`, then `<br>` | `X` (decimal) × 1 024 |
  | `bandwidthBytesPerSecond.out1s` | `Sent` | the same | `X` × 1 024 |
  | `knownRouters` | `Routers` | `N`, then `&nbsp;` | `N` |
  | `floodfills` | `Floodfills` | `N`, then `&nbsp;` | `N` |
  | `tunnels.participating` | `Transit Tunnels` | `N`, then `<br>` | `N` |

  Always `null` from i2pd: `version` (R18), `activePeers` (the main page has no peer count), `tunnels.in`, `tunnels.out`, `tunnels.exploratory`, `tunnels.client` (the `Client Tunnels` figure of i2pd is the sum of every inbound and every outbound tunnel of the router, exploratory tunnels included, so it is not the client count of the contract; eepview does not read it, and adds no contract field for it), `bandwidthBytesPerSecond.in5m` and `.out5m`, `tunnelBuildSuccessPercent.exploratory` and `.client`. The label `Total tunnel creation success rate` is a different label and is not read.
- **R39 New `RouterStats` fields (contract v1.7).** `uptimeResolutionMs`, `floodfills`, `tunnels.client`, `tunnels.exploratory` and `tunnelBuildSuccessPercent.total`. See [IPC contract](ipc-contract.md). `bandwidthBytesPerSecond.in1s` and `.out1s` hold the shortest window that the source gives, which the UI shows as "now": 1 s from the helper, and the current rate that the console gives from Java I2P and i2pd. (The Java I2P row is labeled "3 sec", but the console only shows the router's current rate there, so it is not a 3 s average.) From the helper: `uptimeResolutionMs` is `1` when `uptimeMs` is set, else `null`. `tunnels.client` is `clientInbound + clientOutbound` and `tunnels.exploratory` is `exploratoryInbound + exploratoryOutbound`, each `null` when a part is missing. `floodfills` and `tunnelBuildSuccessPercent.total` are `null`.
- **R40 Bandwidth history from the console.** When `router_stats()` answers from the console, it adds that answer's bandwidth sample to the 10-minute history, as the watcher does for the helper. It adds no sample when the newest sample is less than 4 s old, so the panel and the Network page together still add one sample per 5 s. The `history` of the answer includes the new sample. With the console, the history grows only while a page shows the figures.
- **R41 The UI reads the contract shape.** `router_stats()` answers the `RouterStats` of the [IPC contract](ipc-contract.md). `src/ui/contract.ts` uses that same shape. The router panel and the Network page turn it into their view with one pure function, `statsView` (below):

  | View field | From `RouterStats` |
  |---|---|
  | `uptimeSeconds` | `floor(uptimeMs / 1000)` |
  | `uptimeResolutionSeconds` | `max(1, floor(uptimeResolutionMs / 1000))`; `null` when `uptimeResolutionMs` is `null` |
  | `routerVersion` | `version` |
  | `networkStatus`, `activePeers`, `knownRouters`, `floodfills` | the same field |
  | `bandwidthInBps`, `bandwidthOutBps` | `bandwidthBytesPerSecond.in1s`, `.out1s` |
  | `inboundTunnels`, `outboundTunnels`, `participatingTunnels` | `tunnels.in`, `.out`, `.participating` |
  | `clientTunnels`, `exploratoryTunnels` | `tunnels.client`, `.exploratory` |
  | `buildSuccessRate` | `tunnelBuildSuccessPercent.total / 100`; when `total` is `null`, `.exploratory / 100`; else `null` |
  | `history` | `{ stepSeconds: 5, inBps, outBps }` from the last contiguous run of `history` (below), oldest first; `null` when `history` is empty |
  | `routerKind`, `javaVersion` | `null` |

  `buildSuccessRate` never uses `tunnelBuildSuccessPercent.client`: the build success of client tunnels alone is not the build success of the router, and the panel would show it as the router's rate. A helper that gives only `client` shows "—". **The history run.** `history[].t` is the sample time in ms. `statsView` keeps only the last contiguous run of samples: it starts from the newest sample and goes back while the gap to the previous sample is at most 2 × `stepSeconds` (10 000 ms, with `stepSeconds` 5). The first larger gap ends the run, and the samples before it are dropped. `inBps` and `outBps` hold the `in` and `out` of the kept samples, oldest first. `statsView` never adds, repeats or interpolates a sample. A single sample is a run. A missing or `null` input gives a view with every field `null`.
- **R42 Uptime to its resolution.** The UI never shows a unit smaller than the uptime resolution. `formatUptime(seconds, resolutionSeconds)` keeps today's form (`<d> d <h> h`, `<h> h <m> min`, `<m> min`) and drops each unit smaller than `resolutionSeconds`: with a resolution of 3 600 s, 28 800 s is `8 h`, not `8 h 0 min`; with 86 400 s, 172 800 s is `2 d`. A `null` or missing resolution, or one below 60 s, gives today's output. The router panel and the Network page pass `uptimeResolutionSeconds`.
- **R43 Tunnels line in the router panel.** When `inboundTunnels` and `outboundTunnels` are both `null` and `clientTunnels` or `exploratoryTunnels` is not, the line is `<client> client · <exploratory> exploratory · <participating> participating`, each count as `formatCount` gives it ("—" when `null`). Else it stays `<in> in · <out> out · <participating> participating`. i2pd gives neither `tunnels.client` nor `tunnels.exploratory` (R38), so its line is `— in · — out · <participating> participating`.
- **R44 Where eepview connects.** The About list row of the console names the statistics too: "Router console check, router statistics and the console tab".
- **R45 No console request after stop.** After `shell::console::stop` (R22), `current_stats` does not query the console: it sends no request and takes no figures from the console, until `detect_now` runs again. A `router_stats()` call that starts after the stop and before the next `detect_now` answers from the helper when the helper answers (R31), else with every field `null`, and adds no console sample to the history (R40).
- **R46 One deadline for the whole request.** The 3 s of R33 cover the connect and every read of the request together, not each read alone. A peer that sends bytes slower than that gets figures from what arrived within 3 s: a value cut by the deadline is `null` (R35), and when no complete HTTP answer head arrived within 3 s the console does not answer (R33). A call never lasts longer than 3 s plus the time to close the socket.

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
// crate::core::Core holds one private field, `console_tab: Option<u32>` (R23, R24): the id of
// the console tab. It is the only place that says which tab is the console tab. `Tab` has
// no console flag, so two console tabs cannot exist.

// crate::core
pub enum View { Internal(String), Web(u32), Console(u32) }   // Console: the console tab id, read from `console_tab`
pub enum Effect { /* as before */ Console(ConsoleOp) }        // R27, R28: act on the `console` webview
#[derive(Debug, Clone, PartialEq)]
pub enum ConsoleOp {
    Back,        // R27: one step back in the console view
    Forward,     // R27: one step forward
    Reload,      // R27
    HardReload,  // R27: reload, bypassing the cache
    Stop,        // R27
    Close,       // R28, R30: destroy the `console` webview
}
// Only the operations the console tab needs. There is no variant for Find, FindClear or
// Zoom, so R25 and R27 hold by construction.
impl Core {
    /// The id of the console tab, if one is open.
    pub fn console_tab(&self) -> Option<u32>;
    /// R24: selects the console tab, or makes one right after the active tab and selects
    /// it, showing `url`. The shell calls it before it starts the load, so the first page
    /// event finds the tab. No `WebOp`.
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
    /// R19: the rule list could not be attached: `console_gone` plus a warning toast.
    pub fn console_failed(&mut self) -> Vec<Effect>;
}
```

The other commands keep their signatures and branch on the console tab: `view()` (R23), `tab_infos()` (`kind: "console"`, R23), `navigate()` (R26), `step()`, `reload()`, `stop()` (R27), `tab_close()`, `tab_reopen()` (R28), `find()`, `zoom()`, `bookmark_toggle()`, `shortcut(Find)` (R25). Every path that builds a lazy web tab (`tab_select()`, `tab_cycle()`, `tab_number()`, the tab shown after `tab_close()`, `router_changed()`, `resume()`) skips the console tab, so it never turns into a blocked page.

### Rust, `eepview_lib::shell::console` (`src-tauri/src/shell/console.rs`)

```rust
pub const CONSOLE_LABEL: &str = "console";

pub struct ConsoleWebview;
impl ConsoleWebview {
    /// R8, R24: builds the `console` webview in the main window (hidden, at the content
    /// rect) and puts the chrome (`toolbar`, `status`, `popup`) above it again
    /// (`view::raise_chrome`), or reuses the live one. Then it opens or selects the console
    /// tab (`Core::console_open`), and only then loads the home page of `console` (after
    /// the rule list, R19).
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

### Rust, router statistics (R31–R40)

In `eepview_lib::net::console` (`src-tauri/src/net/console.rs`):

```rust
use crate::net::stats::RouterStats;

pub const STATS_TIMEOUT: Duration = Duration::from_secs(3);   // R33, the whole request
pub const STATS_MAX_ANSWER: u64 = 256 * 1024;                  // R33, bytes

/// R32: "/xhr1.jsp?requestURI=/summaryframe" (Java I2P) or "/" (i2pd).
pub fn stats_path(kind: ConsoleKind) -> &'static str;
/// R35–R37: the Java I2P sidebar body (any status, any size) to statistics.
pub fn parse_java_summary(body: &str) -> RouterStats;
/// R35, R38: the i2pd main page body to statistics.
pub fn parse_i2pd_main(body: &str) -> RouterStats;
/// The parser of `kind`.
pub fn parse_console_stats(kind: ConsoleKind, body: &str) -> RouterStats;
/// R32, R33: one GET of `stats_path` on the console origin, parsed, with the request code of
/// `probe` and its own bounds. `None` when the console does not answer `200` (R33).
/// `history` is always empty here.
pub fn fetch_stats(console: &VerifiedConsole) -> Option<RouterStats>;
```

In `eepview_lib::net::stats` (`src-tauri/src/net/stats.rs`):

```rust
// R39: new fields. Serialized as in the IPC contract (camelCase).
pub struct RouterStats { /* existing fields */, pub uptime_resolution_ms: Option<u64>,
                         pub floodfills: Option<u64> }
pub struct Tunnels { /* in, out, participating */, pub client: Option<u64>,
                     pub exploratory: Option<u64> }
pub struct BuildSuccess { /* exploratory, client */, pub total: Option<u64> }

/// R31: which source gave the figures. Never shown in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsSource { Helper, Console, None }
/// R31: the helper answer, or `None` when the helper does not answer `200` with JSON.
pub fn try_fetch(addr: LoopbackAddr, token: &str) -> Option<RouterStats>;
/// R31: the helper stats when `Some`; else calls `console` once and takes its stats when
/// `Some`; else all-null stats. `console` is not called when `helper` is `Some`.
pub fn pick(helper: Option<RouterStats>, console: impl FnOnce() -> Option<RouterStats>)
    -> (RouterStats, StatsSource);
pub const MIN_SAMPLE_GAP_MS: u64 = 4_000;                       // R40
impl History {
    /// R40: like `record`, but adds no sample when the newest one is less than
    /// MIN_SAMPLE_GAP_MS older than `now`.
    pub fn record_spaced(&mut self, now: u64, stats: &RouterStats);
}
```

`from_helper` maps the new fields as R39 says.

In `eepview_lib::shell::commands` (`src-tauri/src/shell/commands.rs`):

```rust
/// R31, R40: the `router_stats()` answer. Asks the helper (when given), else the stored
/// console (`shell::console::current`, no probe); records a console sample (R40); fills
/// `history`. The `router_stats` command runs this off the main thread.
pub fn current_stats<R: Runtime>(app: &AppHandle<R>, helper: Option<(LoopbackAddr, String)>)
    -> RouterStats;
```

Test helper, `crate::net::testing` (tests only), in addition to `FakeConsole::start`: `FakeConsole::serving(kind: ConsoleKind, path: &str, status: u16, body: &str) -> FakeConsole` answers a request for exactly `path` (the request target, query included) with `status` and `body`, and every other request as `start` does, so `verified()` still passes the probe. `requests()` lists every request line it received.

Fixtures: `src-tauri/tests/fixtures/console/java-2.13.0-xhr1-summaryframe.txt` is a real answer body of `/xhr1.jsp?requestURI=/summaryframe` from Java I2P 2.13.0 (English, not advanced; the router ident, the console nonce and a tunnel hash are replaced). `src-tauri/tests/fixtures/console/i2pd-2.58.0-main-synthetic.txt` is an i2pd main page written from the i2pd source (R38), not captured; its addresses and ident are fake. The lines end with `\n`; i2pd sends `\r\n`, which R38 also accepts. The values that R36–R38 give for them:

| Field | Java fixture | i2pd fixture |
|---|---|---|
| `uptimeMs` / `uptimeResolutionMs` | 28 800 000 / 3 600 000 (`8&nbsp;hours`) | 93 784 000 / 1 000 (`1 day, 2 hours, 3 minutes, 4 seconds`) |
| `networkStatus` | `OK` | `OK` |
| `bandwidthBytesPerSecond` in1s / out1s / in5m / out5m | 53 910 / 37 370 / 37 830 / 33 060 | 12 636 / 5 806 / `null` / `null` |
| `activePeers` / `knownRouters` / `floodfills` | 1 678 / 4 905 / 1 570 | `null` / 3 021 / 812 |
| `tunnels` in / out / participating / client / exploratory | `null` / `null` / 398 / 2 / 11 | `null` / `null` / 157 / `null` / `null` |
| `tunnelBuildSuccessPercent` exploratory / client / total | `null` / `null` / `null` | `null` / `null` / 42 |
| `version` | `null` | `null` |

### IPC (contract v1.7)

- `console_status() -> ConsoleInfo`: the stored result, no probe.
- `console_detect() -> ConsoleInfo`: probes now (R6), off the main thread, stores and answers the result.
- `console_open() -> {ok: boolean, reason?: "no-console"}` (R12). No argument.
- `TabInfo.kind`: `"internal" | "web" | "console"` (R23).
- Event `console-changed: ConsoleInfo`
- `router_stats() -> RouterStats` (contract v1.7): R31–R40. The new fields are in the [IPC contract](ipc-contract.md).

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

### TypeScript, `src/ui/lib/router-stats.ts` (pure, new)

```ts
import type { RouterStats } from "../contract.ts";   // the IPC contract shape (R41)
export const HISTORY_STEP_SECONDS = 5;
export interface StatsView {
  networkStatus: string | null;
  uptimeSeconds: number | null;
  uptimeResolutionSeconds: number | null;
  routerKind: string | null;
  routerVersion: string | null;
  javaVersion: string | null;
  bandwidthInBps: number | null;
  bandwidthOutBps: number | null;
  history: { stepSeconds: number; inBps: number[]; outBps: number[] } | null;
  clientTunnels: number | null;
  exploratoryTunnels: number | null;
  inboundTunnels: number | null;
  outboundTunnels: number | null;
  activePeers: number | null;
  participatingTunnels: number | null;
  buildSuccessRate: number | null;   // 0..1
  knownRouters: number | null;
  floodfills: number | null;
}
/** R41. */
export function statsView(stats: RouterStats | null | undefined): StatsView;
```

Changed, in `src/ui/lib/stats-view.ts` and `src/ui/lib/router-panel.ts`:

```ts
/** R42: a unit smaller than resolutionSeconds is not shown. */
export function formatUptime(seconds: number | null, resolutionSeconds?: number | null): string;
// StatsLike and PanelStatsLike gain optional fields:
//   uptimeResolutionSeconds?: number | null   (statsText and panelText pass it on, R42)
//   clientTunnels?: number | null; exploratoryTunnels?: number | null   (PanelStatsLike, R43)
```

`src/ui/contract.ts` `RouterStats` becomes the contract v1.7 shape. The dev mock (`src/ui/mock.ts`) answers that shape.

### Where the requirement tests live

- `src-tauri/src/net/console/tests.rs` (R1–R7, R9, R10, R18, R19, R21)
- `src-tauri/src/shell/console/tests.rs` (R6, R8, R10–R13, R20–R30 in the shell; the mock runtime fixtures in `crate::shell::testing`)
- `src-tauri/src/core/console_tab/tests.rs` (R23–R29 in the core)
- `src-tauri/tests/architecture.rs` (R4, R8, R11, R14)
- `src/ui/lib/console-links.test.ts` (R15, R18, R20)
- `src/ui/lib/address.test.ts` (R26)
- `src-tauri/src/net/console/stats_from_console.rs` (R32, R33, R35–R38, R46, with the fixtures; declared from `console.rs` as `#[cfg(test)] mod stats_from_console;`)
- `src-tauri/src/net/stats/requirement_tests.rs` (declared from `stats.rs` as `#[cfg(test)] mod requirement_tests;`: R31 `pick`, R39, R40 `record_spaced`)
- `src-tauri/src/shell/commands/tests.rs` (R31, R34 one request per call, R40 and R45 through `current_stats`)
- `src/ui/lib/router-stats.test.ts` (R41, with the history run), `src/ui/lib/stats-view.test.ts` (R42), `src/ui/lib/router-panel.test.ts` (R42, R43)
- `src/ui/popup-router.test.ts` and `src/ui/popup-page.test.ts`: their `router_stats` stubs move to the contract v1.7 shape (R41)
- `src/ui/lib/console-links.test.ts` (R44)

## Limits

- A console behind a password (Java I2P console password, i2pd `http.auth`) is not detected.
- i2pd with a `webroot` other than `/`, or with `http.address` other than `127.0.0.1`, is not detected.
- Linux: the console view has no engine rule list yet (R19); see [No-leak architecture](no-leak-architecture.md).
- Find in page and zoom do nothing in the console tab (R25).
- Statistics from a Java I2P console: no build success rate, and no inbound and outbound split of the tunnels. The uptime has the coarse unit of the console ("8 hours"). The uptime needs an English console. The other figures work in every console language, but they need a router JVM that writes numbers with `.` as the decimal mark and no group separator: Java I2P formats them with the JVM default locale, so on a `de_DE` or `fr_FR` system the bandwidth (`53,91`) and a long uptime (`1,095 days`) are "—". A future console that reorders the rows of a sidebar table without changing their count would give wrong figures; the fixture tests pin the 2.13.0 layout.
- Statistics from i2pd: English console only (R30). Written from the i2pd source, not tested against a live i2pd.
- A sidebar section that the user removed in the Java I2P console settings gives "—" for its figures.

## History

- 2026-10-03 — Router console detection, quick links and the console view — [#54](https://github.com/tcivie/eepview/pull/54)
- 2026-10-03 — The console opens in a console tab; one "I2P Router Console" link replaces the five page links — [#76](https://github.com/tcivie/eepview/pull/76)
- 2026-10-03 — Router statistics from the console, UI contract shape — [#78](https://github.com/tcivie/eepview/pull/78)
