# Roadmap

## Now (v0.1)

- Browser shell: tabs, navigation, bookmarks, history, find in page.
- Five-layer no-leak architecture. See [ADR 0001](docs/adr/0001-no-leak-architecture.md).
- A permanent cross-OS leak test.
- Release pipeline.
- Coverage gate.

## Next

- Startup state machine and router detection (Phase 2).
- Managed Java I2P install with in-network updates and rollback (Phase 3).
- Runtime scan of the downloaded I2P and JRE.

## Later: OS-level network layer (L6)

Goal: the web engine cannot reach anything but the local gatekeeper. The OS enforces this, even if every in-app layer fails. The browser always runs as a normal user. Admin rights are used once, at install only.

- Linux: run the web engine in a network namespace whose only route is the gatekeeper. Use one AppArmor profile (admin, once) for Ubuntu 24.04.
- Windows: an eepview-owned WebView2 Fixed Version runtime, plus a firewall rule (one UAC prompt) that blocks all outbound traffic except loopback. Under evaluation in spike S10.
- macOS: a Network Extension content filter for the WebKit networking process. It needs an Apple Developer ID and an entitlement. It also brings notarized builds.

## Later: other

- Tauri 3 when it is stable.
- Drop the glib advisory when wry moves to gtk 0.19.
- CodeQL, Scorecard, secret scanning and badges when the repo is public. See [docs/going-public.md](docs/going-public.md).
