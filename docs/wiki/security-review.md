# Security review

Status: shipped. Review 1 of a yearly cycle.

## Review record

- Date: 2026-10-03.
- Reviewer: the maintainer ([@tcivie](https://github.com/tcivie)), with the automated tools: clippy pedantic, CodeQL, cargo-deny, zizmor, gitleaks, Socket and dependency review. No outside party took part.
- Scope: the no-leak design (the five layers of ADR 0001), the IPC contract and capabilities, the gatekeeper, the JSON stores, and the GitHub workflows.
- Method: read the design and the code, run the spikes, run the automated tools. The case is in [Assurance case](assurance-case.md).

## Findings so far

| Finding | Where found | Status |
| --- | --- | --- |
| **macOS loopback leak.** `WKWebView` does not send requests to `127.0.0.1` through the proxy. An `<img>`, `<iframe>`, `fetch` or `WebSocket` to a local port went out directly. | Spike S1 | Fixed by design. The gatekeeper adds a content security policy to every response. An engine-level content rule list blocks the rest before the first load. Both ship with the browser shell PR. |
| **The `wry` WebView2 proxy trap.** On Windows, `wry` drops the `proxy_url` setting when custom browser arguments are set. Custom arguments without a proxy would mean direct connections. | Browser shell work | Fixed by design. The Windows arguments always carry `--proxy-server` and the loopback rule. Ships with the browser shell PR. |
| **Lint exclusion in `release.yml`.** The calls of the reusable workflows carry `zizmor: ignore[self-repository]` comments. They go against the no-exclusions rule. | This review | Open. Remove the comments or find a form that both actionlint and zizmor accept. |
| **No DNS block on Windows.** The firewall rules from spike S10 block every off-box TCP and UDP path from the engine, but not DNS. | Spike S10 | Open. The engine proxy stays the main control. See the [roadmap](roadmap.md). |

## Not reviewed

- The managed I2P install and in-network updates (Phase 3). They are not built.
- The OS-level network layer (L6). It is not built.

## Next review due

2027-10-03. Repeat the review sooner if a new network path, a new IPC command or a new dependency with network code is added.

## History

- 2026-10-03 — First security review recorded — see CHANGELOG.
