# Leak test

Status: shipped.

A permanent test on Linux, macOS and Windows. It proves that a page in eepview reaches only
`.i2p` hosts. It is the required check `leak-test (<os>)`. The spec is the "Leak test" section
of [ADR 0001](adr-0001-no-leak-architecture.md).

## What it does

`tests/leak/harness.py` (Python 3, standard library only) runs the real release binary:

- `EEPVIEW_PROXY` points at a fake I2P proxy. The proxy passes VERIFY (`GET http://proxy.i2p/`),
  serves `leaktest.i2p` and `frame.leaktest.i2p`, answers clearnet with `503 No Outproxy
  Configured`, and logs the host of every request.
- `EEPVIEW_START_URL=http://leaktest.i2p/` and `EEPVIEW_EXIT_AFTER=30`.
- Canaries: TCP on `127.0.0.1` and `::1` (one port), TCP and UDP on the LAN address, UDP on
  `127.0.0.1` for STUN.

The test page tries every vector from the ADR: clearnet `<img>`, prefetch, dns-prefetch and
preconnect, `fetch` to a clearnet IP, `127.0.0.1`, `localhost` and the LAN address, `ws://`,
WebRTC STUN in the page and in a cross-origin frame, `window.open`, a `target=_blank` click,
a form auto-submit, a 302 to clearnet and `location = "http://example.com/"`.

Two runs: `default` (JavaScript on, the one that counts) and `js-off` (a sanity check).

## Pass criteria

The exit code is 0 only when all of these hold:

- Every canary got zero hits.
- The fake proxy saw only hosts that end in `.i2p`.
- The page was served and rendered, and its script reported every vector. A page that does
  not load fails.
- Linux only: `strace` shows connects only to the gatekeeper's own listening port and the fake
  proxy, and no DNS (port 53, systemd-resolved or nscd sockets).

## Negative control

`harness.py --negative-control` leaks on purpose, with no eepview involved. It hits every
canary, asks the fake proxy for a clearnet host, loads no page, and feeds the strace parser a
clearnet connect and a DNS query. Every detector must report LEAK or FAIL. CI runs it first.

## Run it locally

```sh
npm run tauri -- build --no-bundle
python3 tests/leak/harness.py --binary src-tauri/target/release/eepview
python3 -m unittest discover -s tests/leak
```

## Results per OS

| OS | default (JS on) | js-off | Notes |
| --- | --- | --- | --- |
| ubuntu-24.04 | pending | pending | first CI run of the PR |
| macos-15 | pending | pending | first CI run of the PR |
| windows-2025 | pending | pending | first CI run of the PR |

## History

- Added in [#41](https://github.com/tcivie/eepview/pull/41).
