# Leak test

Status: shipped.

A permanent test on Linux, macOS and Windows. It proves that a page in eepview reaches only
`.i2p` hosts. It runs in both CI lanes, as the checks `leak-test (ubuntu-24.04, debug)` and
`leak-test (<os>, release)`. The spec is the "Leak test" section of
[ADR 0001](adr-0001-no-leak-architecture.md).

## When it runs

| Lane | Check | Build | OS | When |
| --- | --- | --- | --- | --- |
| Fast lane (`ci.yml`) | `leak-test (ubuntu-24.04, debug)` and `leak harness (ruff + unit tests)` | debug | Linux | A pull request changes a no-leak layer file, the engine settings or the harness. Every push to main. |
| Heavy lane (`heavy.yml`) | `leak-test (<os>, release)` and `leak harness (ruff + unit tests)` | release | ubuntu-24.04, macos-15, windows-2025 | A pull request into a release branch, every night on main, and by hand. |

- The list of files that mark the leak area is in [CI and quality gates](ci-and-quality-gates.md).
- A pull request into main gets the Linux debug run only, and only when it marks the leak area. The macOS and Windows runs happen in the heavy lane.
- On a release branch, the `heavy-gate` check needs the release run on all three OS.
- Both lanes call one script, `scripts/leak-run.sh debug|release`. It builds the app, checks the hardening on a Linux release build, and runs the harness. The script is shared because a reusable workflow and a local composite action do not pass both linters. See the Limits of [CI and quality gates](ci-and-quality-gates.md).
- The `leak harness (ruff + unit tests)` job runs `ruff check` and `ruff format --check` on `tests/leak`, then the unit tests of the harness.

## What it does

`scripts/leak-run.sh` builds the app in the form that users run. Then it runs
`tests/leak/harness.py` (Python 3, standard library only) against it:

- macOS runs the binary inside `eepview.app`. The bundle's `Info.plist` turns on App Transport
  Security (ATS). A bare binary has no `Info.plist`, so ATS never applies to it, and a run of the
  bare binary missed a bug where no `http://` page loaded in the installed app ([#85](https://github.com/tcivie/eepview/pull/85)).
- Windows runs `eepview.exe`. The installer copies this same file, and WebView2 gets its proxy
  from the code, not from a file that the installer writes.
- Linux runs the bare binary. The AppImage carries its own WebKitGTK libraries, which this run
  does not cover yet ([#86](https://github.com/tcivie/eepview/issues/86)).

The run itself:

- `EEPVIEW_PROXY` points at a fake I2P proxy. The proxy passes VERIFY (`GET http://proxy.i2p/`),
  serves `leaktest.i2p` and `frame.leaktest.i2p`, answers clearnet with `503 No Outproxy
  Configured`, and logs the host of every request.
- `EEPVIEW_START_URL=http://leaktest.i2p/` and `EEPVIEW_EXIT_AFTER=30`.
- `EEPVIEW_LOG=1`: the app prints a test trace on stderr. It has one line for each gatekeeper
  request head, answer, refusal and failed connection, and one line for each first load of a
  tab webview. The harness keeps it in `app.<run>.log`. CI uploads it, when the job fails, in the
  `leak-results-debug-<OS>` or `leak-results-release-<OS>` artifact. The trace never goes into
  the diagnostics log or a bug report.
- The fake proxy listens with a backlog of 128. The page sends a burst of parallel requests.
  With the default backlog of 5, macOS drops connects, and the gatekeeper answers 502 after its
  500 ms connect limit.
- Canaries: TCP on `127.0.0.1` and `::1` (one port), TCP and UDP on the LAN address, UDP on
  `127.0.0.1` for STUN.

The test page tries every vector from the ADR: clearnet `<img>`, prefetch, dns-prefetch and
preconnect, `fetch` to a clearnet IP, `127.0.0.1`, `localhost` and the LAN address, `ws://`,
WebRTC STUN in the page and in a cross-origin frame, `window.open`, a `target=_blank` click,
a form auto-submit, a 302 to clearnet and `location = "http://example.com/"`.

Two runs: `default` (JavaScript on, the one that counts) and `js-off` (a sanity check).

On a Linux release build, `scripts/leak-run.sh` also runs `scripts/check-hardening.sh` on the binary. The job fails when PIE, full RELRO or NX is missing.

## Pass criteria

The exit code is 0 only when all of these hold:

- Every canary got zero hits.
- The fake proxy saw only hosts that end in `.i2p`.
- The page was served and rendered, and its script reported every vector. A page that does
  not load fails.
- JavaScript on: the cross-origin frame reported that all its probes finished
  (`frame_wait=done`) before the page navigated away. The page pings the frame until the frame
  answers, and both sides accept only the other's origin and window. The page waits at most
  `EEPVIEW_EXIT_AFTER` minus 15 s (15 s for the default 30), and always at least 4 s, so the
  vectors it does not await get time before the first navigation.
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
# macOS: run the binary inside the .app, as CI does (its Info.plist turns on ATS)
npm run tauri -- build --bundles app
python3 tests/leak/harness.py --binary src-tauri/target/release/bundle/macos/eepview.app/Contents/MacOS/eepview
python3 -m unittest discover -s tests/leak
```

To run the same steps as CI, call the script. Set `RUNNER_OS` to `Linux`, `macOS` or `Windows`:

```sh
RUNNER_OS=macOS ./scripts/leak-run.sh release
```

## Results per OS

| OS | default (JS on) | js-off | Notes |
| --- | --- | --- | --- |
| ubuntu-24.04 | pass | pass | run on the browser shell head with this harness; 0 canary hits, only .i2p hosts upstream |
| macos-15 | pass | pass | run on the browser shell head with this harness; 0 canary hits, only .i2p hosts upstream |
| windows-2025 | pass | pass | run on the browser shell head with this harness; 0 canary hits, only .i2p hosts upstream |

## History

- Added in [#41](https://github.com/tcivie/eepview/pull/41).
- The frame done signal, the trace and the fake proxy backlog in [#71](https://github.com/tcivie/eepview/pull/71).
- macOS runs the binary inside the `.app` bundle in [#85](https://github.com/tcivie/eepview/pull/85). The bare binary has no `Info.plist`, so App Transport Security never applied to it, and the test missed a bug where no `http://` page loaded in the installed app.
- The test runs in two lanes in [#89](https://github.com/tcivie/eepview/pull/89): a debug build on Linux in the fast lane, and a release build on all three OS in the heavy lane. `scripts/leak-run.sh` holds the shared steps.
