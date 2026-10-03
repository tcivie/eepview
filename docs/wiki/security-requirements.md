# Security requirements

Status: in progress. The browser shell PR ships the checks that enforce these promises.

This page says what eepview promises about security, and what it does not.

## What eepview promises

1. **No traffic outside I2P.** A page loads only through the I2P HTTP proxy of a router that eepview verified. Nothing from the web engine goes to the clearnet, to a DNS server or to a local port. This includes WebRTC and UDP.
2. **Consent before any network action.** eepview does no network action that the user did not start. There is no telemetry, no background request, no update check, and no remote font or asset in a bundled page. A change to the router settings needs a click.
3. **Fail closed.** If a layer is missing, broken or unsure, the request is blocked. The browser opens no page until the router proxy is verified. An address that is not an I2P site shows the blocked page.
4. **No IPC for web content.** A page from the network never reaches the internal commands of the app. Only the bundled toolbar and internal pages have IPC. Pages from `.i2p` sites have none.
5. **No extra HTTP client.** The project adds no new HTTP client crate. Network code lives in one module.

How eepview does this: see [Assurance case](assurance-case.md) and [No-leak architecture](no-leak-architecture.md).

## What eepview does not promise

- **No anonymity beyond I2P.** eepview does not make I2P stronger. An I2P flaw is an I2P flaw.
- **No protection from a hostile site inside I2P.** A site can run its own scripts, track you inside the page and try to fingerprint you. The JavaScript toggle and the content policy limit this, but they do not remove it.
- **No protection from a compromised computer.** Malware, a rogue browser extension on the host, or a hostile administrator is out of scope.
- **No fingerprint match across systems.** Each operating system uses its own web engine. The engines differ. This is a documented limit.
- **No bug-free web engine.** eepview uses the WebView of the operating system. A bug in it is out of scope. See [SECURITY.md](https://github.com/tcivie/eepview/blob/main/SECURITY.md).
- **No signed installers yet.** Windows installers and the macOS `.dmg` are not signed. See [Release pipeline](release-pipeline.md).
- **No OS-level network block yet.** An OS layer that enforces the rules even if every in-app layer fails is planned. See the [roadmap](roadmap.md).

## History

- 2026-10-03 — Add the governance pages — [#30](https://github.com/tcivie/eepview/pull/30).
