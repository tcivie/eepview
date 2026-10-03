# Robustness tests

Status: in progress.

Property tests that look for panics and wrong behavior in the pure logic of the browser shell. No GUI is needed.

## How it works

- `proptest` makes random input. Each test names the requirement it checks in a `Req:` comment. The sources are the [ADR](adr-0001-no-leak-architecture.md), the [IPC contract](ipc-contract.md) and the [Browser shell](browser-shell.md) page.
- `src-tauri/tests/robust_nav.rs`: the host predicate, the address bar, the engine rules, and the planner that the gatekeeper uses. One test checks that the L1, L3 and L4 layers agree on every host.
- `src-tauri/tests/robust_gatekeeper.rs`: the gatekeeper with real loopback sockets and a fake router, in its own test process so its sockets cannot disturb the gatekeeper unit tests. Random and malformed request heads, CONNECT lines, huge heads, partial reads, slow and half-closed clients, and eight concurrent slow images.
- `src-tauri/tests/robust_core.rs`: random command sequences on the state machine, back and forward, and a soak test of 1 000 tabs.
- `src-tauri/tests/robust_stores.rs`: random bookmark and history commands, corrupt files, the 10 000-entry history cap, import and export.

## How to use / run locally

- `cargo test --manifest-path src-tauri/Cargo.toml --test robust_nav --test robust_core --test robust_stores --test robust_gatekeeper`
- A failing property prints the smallest input that fails.

## Limits

- The shell glue (webviews, the main loop) is not covered. Check by hand on each OS: open a second content tab, toggle site JavaScript, and close and reopen a tab. The window must stay responsive.
- The gatekeeper tests use a fake router, not a real I2P router.

## History

- 2026-10-03 — Add the robustness tests — [#52](https://github.com/tcivie/eepview/pull/52)
