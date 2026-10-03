# Assurance case

Status: in progress. Controls that live in the browser shell are marked "ships with the browser shell PR" until that PR merges.

This page argues that eepview keeps its promises in [Security requirements](security-requirements.md). It names the attackers, the trust boundaries, the design rules and the weaknesses that we counter.

The five layers come from ADR 0001 (no-leak architecture). The ADR page lands with the browser shell PR as `adr-0001-no-leak-architecture.md`. See [No-leak architecture](no-leak-architecture.md).

## Claim

No packet from the web engine leaves except through the I2P HTTP proxy. A page cannot reach the app. The user starts every network action.

## The five layers and the gatekeeper

| Layer | What it is |
| --- | --- |
| L1 | The gatekeeper. eepview's own HTTP proxy on `127.0.0.1` at a random port. It forwards a request to the verified router only when the host is an `.i2p` name. It answers everything else itself, with no upstream connection. |
| L2 | The engine proxy setting. Every content webview uses the gatekeeper as its proxy. On Windows the same proxy is also in the browser arguments. |
| L3 | The request rules. L3a is a content security policy that the gatekeeper adds to every response. L3b is an engine-level request filter, attached before the first load. |
| L4 | The navigation guard. It checks every navigation and every new-window request. Only an I2P URL passes. |
| L5 | WebRTC removed in every frame, and turned off in the engine settings where the engine allows it. |

One host check, `is_i2p_host`, is the only predicate. L1, L3 and L4 all call it.

## Threat model

| Attacker | How | What we want to protect |
| --- | --- | --- |
| Hostile eepsite | Serves a page that loads a clearnet or loopback URL (`<img>`, `<iframe>`, `fetch`, `WebSocket`), redirects off I2P, opens a window, uses WebRTC, or runs script that tries to call the app. | The user's IP address. Local services. The app commands. |
| Local non-I2P proxy | Another program listens on the port where eepview expects the router proxy, or the configured proxy is not an I2P router. | The traffic. The promise that the proxy is an I2P router. |
| Malicious update | A tampered installer, a poisoned dependency, or a changed workflow. | The code the user runs. |
| Malicious local file | A crafted bookmark import or a changed store file. | The stores and the app state. |

Out of scope: a compromised computer, a flaw inside I2P, and a flaw in the OS web engine. See [Security requirements](security-requirements.md).

## Trust boundaries

```text
                +-----------------------------------------------+
  UNTRUSTED     |  I2P network and every .i2p site              |
                +----------------------+------------------------+
                                       |
                +----------------------v------------------------+
  SEMI-TRUSTED  |  Local I2P router (HTTP proxy)                |
                |  verified before use                          |
                +----------------------^------------------------+
                                       |  B3: only .i2p hosts
 ============================ eepview process ==================
                +----------------------+------------------------+
  TRUSTED CODE  |  Gatekeeper (L1) on 127.0.0.1                 |
                +----------------------^------------------------+
                                       |  B2: proxy + rules (L2, L3)
                +----------------------+------------------------+
  UNTRUSTED     |  Content webviews tab-*   (no IPC)            |
                |  L4 navigation guard, L5 no WebRTC            |
                +-----------------------------------------------+
                                       X  B1: no IPC to web content
                +-----------------------------------------------+
  TRUSTED       |  Rust core, stores, toolbar and internal      |
                |  pages (bundled, IPC allowed)                 |
                +-----------------------------------------------+
                                       |  B4: files in the app config dir
                +----------------------v------------------------+
  LOCAL DATA    |  JSON stores (bookmarks, history, settings)   |
                +-----------------------------------------------+
```

- B1: a web page has no IPC. The capabilities name only the `toolbar` and `internal` webviews.
- B2: the web engine can reach only the gatekeeper, and the rules block the rest.
- B3: the gatekeeper forwards only `.i2p` hosts, to a router that eepview verified.
- B4: the core reads and writes only fixed files in the app config directory.

## Secure design principles

| Principle | How eepview applies it |
| --- | --- |
| Economy of mechanism | One host predicate, `is_i2p_host`, used by L1, L3 and L4. One factory builds every remote webview. The gatekeeper handles one request per connection and has no general proxy code. Pure modules hold the rules, so unit tests cover them. The architecture test fails when a rule is broken: a webview built outside the factory, a socket outside `net/`, `unsafe` outside the platform bridge, or IPC for a `tab-*` webview. |
| Fail-safe defaults | The default answer is "block". The rule set is "block everything, then allow `.i2p`". No content webview exists before the router is verified. A webview starts on `about:blank` and loads the page only after the engine filter is on. `unsafe_code` is denied. A broken store file means a fresh start, not a crash with partial data. |
| Complete mediation | Every request from a web page goes through the gatekeeper (L1, L2). Every response gets the policy (L3a). Every navigation and every new window goes through the guard (L4). No page path skips these. The factory is the only place that builds a remote webview, so none is missed. |
| Least privilege | Web content has no IPC. The capability files name each command for each webview. The status bubble can only listen for one event. The browser runs as a normal user. A future OS layer (L6) is planned to block everything but loopback. |
| Allowlist input validation | The gatekeeper allows `.i2p` host names and refuses the rest. The address bar parses input into a typed target, and refuses `javascript:`, `data:`, `file:` and similar schemes. The loopback address type can hold only a loopback address. Stores read typed JSON with a version field. |

## Common weaknesses we counter

| CWE | Where it could appear | Counter |
| --- | --- | --- |
| CWE-918 SSRF | A page makes the app fetch an internal URL. | The gatekeeper allows only `.i2p` hosts, and answers all other hosts itself. A page cannot make the core issue a request. The core builds no HTTP client for page input. L3 blocks loopback requests in the engine (macOS spike S1). |
| CWE-601 Open redirect | A site redirects the tab to a clearnet URL. | The L4 guard checks every navigation, including a redirect target. A refused target shows the blocked page. |
| CWE-79 XSS into the chrome | A page title, a URL or a history entry runs script in the toolbar or an internal page. | The toolbar and internal pages are bundled. They write page-controlled text with `textContent`, not as HTML. Web content runs in a separate webview with no IPC, so script there cannot call a command. |
| CWE-22 Path traversal in stores | A name from input picks a file outside the store. | Store file names are fixed in code (`bookmarks.json`, `history.json`, `sites.json`, and similar). Ids are generated by the app. No page input becomes a path. Writes use a temp file and a rename. |
| CWE-94 / CWE-77 IPC injection | A page calls an app command, or an event carries a command. | Web content has no IPC. Only two bundled webviews may call commands, and each command is named in a capability file. Command arguments are typed, so a malformed call fails to parse. |

## Evidence

- Unit tests for each pure module. See [Testing policy](testing-policy.md) and [Coverage](coverage.md).
- The architecture test (ADR 0001) and the leak test. See [Leak test](leak-test.md). Both ship with the browser shell and the leak harness PRs.
- Static analysis in CI: clippy pedantic, CodeQL, cargo-deny, zizmor and gitleaks. See [Coding standards](coding-standards.md) and [CI and quality gates](ci-and-quality-gates.md).
- The [security review](security-review.md).

## Limits

- This case rests on the browser shell and the leak harness. Until they merge, the layers exist as a design and as work on branches.
- No third party has audited eepview.
- The OS-level layer (L6) is not built. If the in-app layers have a bug, no second wall stops the leak.

## History

- 2026-10-03 — Add the assurance case — see CHANGELOG.
