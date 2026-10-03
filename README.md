<!-- TODO: centered logo. Add it from assets/brand/ once the brand PR has merged. -->

<h1 align="center">eepview</h1>

<p align="center">A small browser that opens I2P sites and nothing else.</p>

<p align="center">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue"></a>
</p>

## What it is

eepview is a browser for I2P sites (`*.i2p`). It runs on the web view of your operating system, so it is small. The design goal: no clearnet traffic from the page view, because every page load goes through the I2P HTTP proxy. This is not built yet. See Status.

## Security model (design target, not built yet)

| Path | Planned guard |
|---|---|
| Page loads | The page view exists only after the I2P proxy is verified. All loads go through that proxy. |
| Outproxy | Not used. The managed router has an empty outproxy list, checked at every start. For an external router, eepview checks it. |
| Loopback | Forced through the proxy, which refuses it. A page script cannot reach local services. |
| Navigation | Only `http(s)://*.i2p` and `*.b32.i2p` can load. |
| WebRTC | Off. |
| JavaScript | Off by default. You can turn it on per site. |
| Consent | No network action starts before you agree to it. The installer and the Java update are the only clearnet paths. They run only after a click, to fixed hosts. |

## Features

- Tabs (in progress)
- Bookmarks (in progress)
- History (in progress)
- Find in page (in progress)
- Router stats (in progress)
- Light and dark theme (in progress)

## Screenshots

<!-- TODO: add real screenshots here when the UI is finished. -->

## Status

Pre-release. Phase 1 (app shell) is in progress. Do not use eepview for anything that needs anonymity yet.

## Build from source

Requirements:

- Rust 1.96 (`rust-toolchain.toml` pins it)
- Node.js 22 or later, and npm
- macOS 14 or later, Windows 10 or later with WebView2, or Linux with WebKitGTK 4.1

```sh
npm ci
npm run tauri dev
```

## Supply-chain checks

CI enforces these checks on every pull request:

- clippy with the pedantic lint group
- complexity limits
- biome
- cargo-deny (bans and sources)
- gitleaks
- zizmor
- actionlint
- Socket.dev

## Going public

CodeQL and Scorecard start when the repo becomes public. See [docs/going-public.md](docs/going-public.md).

## Credits

eepview uses the [I2P](https://geti2p.net) network. It is not affiliated with the I2P project.

## License

[MIT](LICENSE)
