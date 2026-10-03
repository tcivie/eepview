# Roadmap

## Now (v0.1)

- Browser shell: tabs, navigation, bookmarks, history, find in page.
- Five-layer no-leak architecture. See [Decisions](decisions.md) (ADR 0001 lands with the browser shell).
- A permanent cross-OS leak test.
- Release pipeline.
- Coverage gate.

## Wanted: co-maintainer

eepview has one maintainer. The project wants a second active maintainer. This closes a real gap: nobody else can review, merge or release today. See [Access continuity](access-continuity.md) and [Governance](governance.md). Open an issue if you want to help.

## Next

- Startup state machine and router detection (Phase 2).
- Managed Java I2P install with in-network updates and rollback (Phase 3).
- Runtime scan of the downloaded I2P and JRE.

## Later: OS-level network layer (L6)

Goal: the web engine cannot reach anything but the local gatekeeper. The OS enforces this, even if every in-app layer fails. The browser always runs as a normal user. Admin rights are used once, at install only.

- Linux: run the web engine in a network namespace whose only route is the gatekeeper. Use one AppArmor profile (admin, once) for Ubuntu 24.04.
- Windows: an eepview-owned WebView2 Fixed Version runtime, plus a firewall rule (one UAC prompt) that blocks all outbound traffic except loopback. Spike S10 (branch spike/s10-windows-firewall): partial. The rules block every off-box TCP and UDP path from the engine. They do not block DNS, which goes through the Windows DNS service, so this layer adds to the engine proxy and does not replace it. Cost: about 300 MB to download, 670 MB on disk, and an update about every 4 weeks that eepview must ship, with one UAC prompt each time.
- macOS: a Network Extension content filter for the WebKit networking process. It needs an Apple Developer ID and an entitlement. It also brings notarized builds.

## Later: other

- Tauri 3 when it is stable.
- Drop the glib advisory when wry moves to gtk 0.19.
- CodeQL, Scorecard, secret scanning and badges when the repo is public. See [Going public](going-public.md).
- **Research: WebRTC inside I2P.** WebRTC is off today. The I2P HTTP proxy carries TCP only, and the web engines send WebRTC UDP straight to IP addresses, outside any proxy. I2P itself has datagrams (SAM, `streamr`), but a browser's WebRTC stack can address only IP:port endpoints, not I2P destinations. Study a bridge that maps WebRTC ICE candidates to I2P datagram destinations, and decide whether it can keep every packet inside I2P on all three engines. Sources: https://i2p.net/en/docs/api/datagrams, https://i2p.net/en/docs/api/i2ptunnel.
