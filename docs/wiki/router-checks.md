<!--
SPDX-FileCopyrightText: 2026 The eepview contributors
SPDX-License-Identifier: MIT
-->

# Router checks

Status: in progress.

The router card on the Home page shows the four checks that eepview runs on the router. Each check shows what the backend found, and nothing else. A check turns green only when the backend reports that it passed. No timer and no CSS animation in the page changes a check.

Before this page, the card showed a path "You → Hop → Hop → Hop → Site". Its dots turned green when the router state was `ok`, but nothing checked a hop. The path is gone.

## The four checks

| # | `id` | Name on the card | What eepview checks |
|---|---|---|---|
| 1 | `proxy-i2p` | Proxy is an I2P router | VERIFY: `GET http://proxy.i2p/` through the router proxy answers `200` with "I2P HTTP proxy OK", and the gatekeeper runs ([ADR 0001](adr-0001-no-leak-architecture.md), rule 5). |
| 2 | `version` | Router version is supported | The router version is at least the minimum for its type: Java I2P 2.4.0, i2pd 2.50.0. |
| 3 | `no-outproxy` | No outproxy | The HTTP proxy tunnel of the router, on the port of the router proxy, lists no outproxy. eepview reads the router's own tunnel configuration file. |
| 4 | `tunnels` | Network up, client tunnel built | The router network status is `OK`, and the router has at least one client tunnel. |

Only check 1 opens and closes the gate. Checks 2 to 4 show a fact. They never close the gate and never change `RouterStatus.state`. The gatekeeper forwards only `.i2p` hosts (ADR 0001, layer L1), so a router outproxy cannot carry a request from eepview. An old router or a router with no tunnel yet cannot make eepview leak: pages only fail to load.

The minimum versions are the first releases of each router with the network database hardening of late 2023: Java I2P 2.4.0 (December 2023) and i2pd 2.50.0 (January 2024).

## Requirements

Each requirement is a test target. The tests check these rules, not the code.

### The state of a check

- **V1 Shape.** `RouterStatus.checks` always has exactly four entries, in the order of the table: `proxy-i2p`, `version`, `no-outproxy`, `tunnels`. Each entry is `{id, state, detail, passedAt}`.
- **V2 States.**
  - `pending`: the check has not started. It waits for check 1, or for the gate.
  - `running`: the check runs now.
  - `passed`: the last run passed.
  - `failed`: the last run failed.
  - `not-checked`: eepview cannot check this for this router. `detail` says why. A check that eepview cannot run is never `passed`.
- **V3 Detail.** A `failed` or `not-checked` check always has a `detail` that is not empty: the short reason. A `passed` check may have a `detail`: the fact that it checked. A `pending` or `running` check has `detail` `null`.
- **V4 Pass time.** `passedAt` is the Unix time in ms when the check turned `passed`. It stays the same while the check stays `passed`, round after round. In every other state it is `null`. A check that leaves `passed` and passes again gets the time of the new pass.

### Check 1: the proxy is an I2P router

- **V5 Start.** At start, all four checks are `pending`. When a VERIFY round starts and check 1 is `pending`, check 1 turns `running`. A check 1 that is `passed` or `failed` keeps that state while the round runs, so the card does not flicker every 5 s.
- **V6 Result.** After the round, `RouterStatus.state` `ok` makes check 1 `passed`. Any other state makes it `failed`, and its `detail` is the `RouterStatus.detail` of that round: the reason VERIFY gave, such as a refused connection or a wrong self-test page. When that `detail` is `null`, the check `detail` is the router state.
- **V7 Gate closed.** While check 1 is not `passed`, checks 2 to 4 are `pending`, with `detail` and `passedAt` `null`. So when the router stops or a VERIFY round fails, check 1 shows `failed` with its reason, and checks 2 to 4 go back to `pending`.
- **V8 Pause and resume.** `connection_pause()` makes all four checks `pending`. While the connection is paused, they stay `pending`. `connection_resume()` makes all four `pending`. The next VERIFY round then runs as V5 and V6.

### Checks 2 to 4

- **V9 When they run.** When check 1 turns `passed` and the connection is not paused, checks 2 to 4 turn `running`. In the same round, eepview runs them (V10 to V13). It runs them again in every VERIFY round (every 5 s) while the gate is open.
- **V10 Facts.** eepview reads these facts once per round:
  - The router type: the type of the console that detection stored ([Router console](router-console.md), R5): Java I2P or i2pd. With no stored console, the type is not known.
  - The router version: `RouterStatus.version` (from the router helper) when it is set, else the version of the stored console (R18).
  - The router statistics: from the router helper when it answers, else from the stored console, with the request of R32 and R33. With neither, there are none. This read adds no sample to the bandwidth history (R40 does not change).
  - The outproxy configuration (V12).
- **V11 Check 2, the version.**
  - Router type not known: `not-checked`. The detail says that no router console was found, so the router type is not known.
  - Version `null`: `not-checked`. The detail says that the router did not report its version.
  - The version text is one to three decimal integers joined by `.`, then optionally `-` and any text, which is ignored. A missing part is `0`. So `2.10.0-3` is 2.10.0 and `2.7` is 2.7.0. Any other text: `not-checked`, and the detail quotes the text.
  - The version compares part by part, as numbers: 2.10.0 is newer than 2.9.0.
  - At least the minimum of its type (Java I2P 2.4.0, i2pd 2.50.0): `passed`. The detail names the version and the minimum.
  - Older: `failed`. The detail names the version and the minimum.
- **V12 Check 3, no outproxy.**
  - The proxy port is the port of `RouterStatus.proxy` (`127.0.0.1:4444` gives 4444).
  - **Java I2P.** In each Java I2P configuration folder of R2, in that order: every file in `i2ptunnel.config.d/`, by file name, then `i2ptunnel.config`. A file is a Java properties file (`key=value`; `#` and `!` start a comment line). In `i2ptunnel.config`, the keys of tunnel `<n>` are `tunnel.<n>.<key>`. A file in `i2ptunnel.config.d/` holds one tunnel, and its keys are either `<key>` or `tunnel.<n>.<key>`. A tunnel is the HTTP proxy when `type` is `httpclient` and `listenPort` is the proxy port. Its outproxies are the values of `proxyList` and of `option.i2ptunnel.httpclient.SSLOutproxies`, split on `,`, `;` and white space. Empty parts and repeats are dropped. The order is kept.
  - **i2pd.** Each `i2pd.conf` of R2, in that order. The keys of the `[httpproxy]` section count. `#` starts a comment. The section is the HTTP proxy when `enabled` is not `false` and `port` (default `4444`) is the proxy port. Its outproxies are the value of `outproxy`, split on `,`, each part trimmed, empty parts dropped.
  - **Which files.** Router type Java I2P: the Java I2P files only. i2pd: the i2pd files only. Type not known: both. Within one type, the first HTTP proxy found wins. When the type is not known and both types have an HTTP proxy on the port: `not-checked`, and the detail says that both a Java I2P and an i2pd configuration use that port.
  - **Result.** No HTTP proxy found on the port: `not-checked`. The detail says that no router configuration with an HTTP proxy on that port was found. An HTTP proxy with no outproxy: `passed`. The detail names the file. An HTTP proxy with outproxies: `failed`. The detail names each outproxy.
  - eepview only reads these files. A missing or unreadable file counts as no HTTP proxy. A detail names a file by its name only, never by its folder.
  - A router that eepview manages (Phase 3) gets a configuration from eepview with no outproxy. This same rule reads it.
- **V13 Check 4, network and tunnels.**
  - No statistics (V10): `not-checked`. The detail says that the router statistics are not available.
  - `networkStatus` set and not `OK`: `failed`. The detail names the status, for example `FIREWALLED`.
  - `tunnels.client` is `0`: `failed`. The detail says that no client tunnel is built yet.
  - `networkStatus` is `OK` and `tunnels.client` is 1 or more: `passed`. The detail names the client tunnel count.
  - Else, one of the two figures is `null`: `not-checked`. The detail names the figure that the router does not report. i2pd never reports `tunnels.client` (R38).
  - A failure wins over a missing figure: `networkStatus` `TESTING` with `tunnels.client` `null` is `failed`.
- **V14 Events.** `router-status` carries `checks`. eepview emits it on every VERIFY round, as before, and also when check 1 turns `running` (V5) and when checks 2 to 4 change their `state`, `detail` or `passedAt`. A round that changes nothing in checks 2 to 4 emits nothing for them.

### The card on the Home page

- **V15 Four rows.** The card shows the four checks as an ordered list (`ol#router-checks`, label "Router checks"), in the order of V1. Each row (`li.check`, `data-check="<id>"`, `data-state="<state>"`) shows the name of the check (`.check-name`), its state as text (`.check-state`), and its detail (`.check-detail`) when it has one.
- **V16 State text.** The state is always written as text, not only as a color:

  | `state` | Text |
  |---|---|
  | `pending` | Waiting |
  | `running` | Checking |
  | `passed` | Passed at `HH:MM:SS` (24-hour local time of `passedAt`, two digits each) |
  | `failed` | Failed |
  | `not-checked` | Not checked |

  A `passed` check with `passedAt` `null` shows "Passed".
- **V17 Only real state.** A row looks passed (the accent mark) only while its check is `passed`. The page changes a row only when a `router-status` event or the `router_status()` answer brings new `checks`. No timer, no CSS animation and no CSS transition changes a row. A `RouterStatus` with no `checks` shows four rows "Waiting".
- **V18 Announcements.** A polite live region (`#router-checks-live`, `role="status"`) says "<name>: <state text>." for each check whose `state` changed, in check order, one sentence each. It says nothing on the first render, and nothing when only a `detail` or a `passedAt` changed.
- **V19 The rest of the card.** The state chip and line, Router, Proxy, "Network details" and the "Router console" section ([Router console](router-console.md), R15) do not change.
- **V20 Look.** The rows use the shared component `.checks` ([UI components](ui-components.md)): palette tokens only, a mark per state that is `aria-hidden`, and the state text next to it. `failed` uses the danger color, `passed` the accent color, `not-checked` and `pending` the muted color.

## Interface

### IPC (contract v1.9)

```ts
type CheckId = "proxy-i2p" | "version" | "no-outproxy" | "tunnels";
type CheckState = "pending" | "running" | "passed" | "failed" | "not-checked";
type VerifyCheck = { id: CheckId; state: CheckState; detail: string | null;
  passedAt: number | null };  // Unix ms; null unless state is "passed"
type RouterStatus = { /* v1.8 fields */ checks: VerifyCheck[] };  // V1: always 4, in order
```

### Rust, `eepview_lib::types`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]       // "proxy-i2p" | "version" | "no-outproxy" | "tunnels"
pub enum CheckId { ProxyI2p, Version, NoOutproxy, Tunnels }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]       // "pending" | "running" | "passed" | "failed" | "not-checked"
pub enum CheckState { Pending, Running, Passed, Failed, NotChecked }

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyCheck { pub id: CheckId, pub state: CheckState,
                         pub detail: Option<String>, pub passed_at: Option<u64> }

/// The `router_status()` answer and the `router-status` payload: every `RouterStatus`
/// field, then `checks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouterReport { #[serde(flatten)] pub status: RouterStatus, pub checks: Vec<VerifyCheck> }
```

`RouterStatus` keeps its fields. The checks live next to it in the core.

### Rust, `eepview_lib::core::checks` (pure)

```rust
pub const JAVA_MIN_VERSION: (u64, u64, u64) = (2, 4, 0);
pub const I2PD_MIN_VERSION: (u64, u64, u64) = (2, 50, 0);

/// One run of a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome { Passed(Option<String>), Failed(String), NotChecked(String) }

/// V11: `1.2.3`, `1.2`, `1`, each with an optional `-<text>` suffix. `None` for other text.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)>;
pub fn version_outcome(kind: Option<ConsoleKind>, version: Option<&str>) -> Outcome;   // V11
pub fn outproxy_outcome(finding: &OutproxyFinding) -> Outcome;                         // V12
pub fn tunnels_outcome(stats: Option<&RouterStats>) -> Outcome;                         // V13

/// The facts of one round (V10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFacts { pub kind: Option<ConsoleKind>, pub version: Option<String>,
                        pub outproxy: OutproxyFinding, pub stats: Option<RouterStats> }
```

`ConsoleKind` is `eepview_lib::net::console::ConsoleKind`. `RouterStats` is `eepview_lib::net::stats::RouterStats` (`Default` gives every field `None`).

### Rust, `eepview_lib::core::Core`

```rust
impl Core {
    pub fn checks(&self) -> &[VerifyCheck];                                   // V1
    pub fn router_report(&self) -> RouterReport;                              // V1, V14
    /// V5: a VERIFY round starts. Emits `Event::Router` only when check 1 changed.
    pub fn verify_started(&mut self) -> Vec<Effect>;
    /// The result of a VERIFY round at `now` (Unix ms): `router_changed(status)`, then V6, V7, V9.
    pub fn router_checked(&mut self, now: u64, status: RouterStatus) -> Vec<Effect>;
    /// V9 to V14: checks 2 to 4 from the facts of a round at `now`. Does nothing while the
    /// gate is closed or the connection is paused. Emits `Event::Router` only on a change.
    pub fn checks_seen(&mut self, now: u64, facts: &CheckFacts) -> Vec<Effect>;
}
```

`Core::new`, `router_changed`, `pause` and `resume` keep their signatures. `router_changed(status)` alone does not change the checks. `pause()` and `resume()` do (V8).

### Rust, `eepview_lib::net::outproxy` (`src-tauri/src/net/outproxy.rs`)

```rust
/// What the router configuration says about the HTTP proxy on one port (V12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutproxyFinding {
    /// The HTTP proxy is in `file` (file name only) and lists no outproxy.
    Clear { file: String },
    /// The HTTP proxy is in `file` and lists these outproxies.
    Listed { file: String, outproxies: Vec<String> },
    /// eepview could not tell: the reason.
    Unknown(String),
}

/// The outproxies of the `httpclient` tunnel on `port` in one Java I2P tunnel file
/// (`i2ptunnel.config` or one file of `i2ptunnel.config.d/`). `None`: no such tunnel.
pub fn java_outproxies(config: &str, port: u16) -> Option<Vec<String>>;
/// The outproxies of the `[httpproxy]` section of one `i2pd.conf` when it serves `port`.
pub fn i2pd_outproxies(i2pd_conf: &str, port: u16) -> Option<Vec<String>>;
/// V12 over the files of this OS (the folders of `net::console::java_config_dirs` and the
/// files of `net::console::i2pd_config_files`, with the same `env`).
pub fn find_outproxy(kind: Option<ConsoleKind>, port: u16,
                     env: &dyn Fn(&str) -> Option<String>) -> OutproxyFinding;
```

### TypeScript, `src/ui/lib/router-checks.ts` (pure)

```ts
export const CHECK_NAMES: Record<CheckId, string>;   // the names of the table above
export interface CheckView { id: CheckId; name: string; state: CheckState; stateText: string;
  detail: string | null }
/** V15–V17: the four rows, in order. A missing or short list gives "Waiting" rows. */
export function checkViews(checks: VerifyCheck[] | null | undefined): CheckView[];
/** V16: `HH:MM:SS`, 24-hour local time. */
export function clockTime(ms: number): string;
/** V18: the announcement for a change, "" when there is nothing to say. */
export function checkAnnouncement(before: VerifyCheck[] | null | undefined,
  after: VerifyCheck[] | null | undefined): string;
```

`CheckId`, `CheckState` and `VerifyCheck` are in `src/ui/contract.ts`. The dev mock (`src/ui/mock.ts`) answers `router_status()` with four checks that follow its mock router state, and `?checks=mixed` previews a failed and a not-checked check.

### Where the requirement tests live

- `src-tauri/tests/router_checks.rs` (V1–V14, the core and the outcomes)
- `src-tauri/tests/router_checks_outproxy.rs` (V12, the configuration files)
- `src/ui/lib/router-checks.test.ts` (V15–V18)
- `src/ui/home-checks.test.ts` (V15–V19, the Home page on the stand-in shell)
- `src/ui/mock-contract.test.ts` (V1, the dev mock)

## Limits

- Checks 2 to 4 need the router console for the router type and, without a router helper, for the statistics. Detection runs when the Home page loads as the active tab ([Router console](router-console.md), R6). Until then, checks 2 and 4 are `not-checked`.
- Java I2P ships with an outproxy in its HTTP proxy tunnel. So on a default Java I2P router, check 3 fails. eepview still never uses it (ADR 0001, layer L1).
- eepview cannot see an outproxy set on the i2pd command line.

## History

- 2026-10-06 — The router card shows the four real checks instead of the decorative hop path.
