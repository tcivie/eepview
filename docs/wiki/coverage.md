# Coverage

Status: shipped.

Unit-test coverage gate for Rust and TypeScript.

## How it works

- The `coverage` job in `.github/workflows/ci.yml` runs on ubuntu-24.04 only.
- Rust lines: `cargo llvm-cov --workspace --all-targets --summary-only`, then `cargo llvm-cov report --fail-under-lines <N>`. Every file counts. There are no exclusions.
- Rust branches: the pinned `nightly-2026-10-01` runs the same tests with `--branch`. cargo-llvm-cov has no `--fail-under-branches`, so `jq` reads `totals.branches.percent` from the JSON summary and fails below the floor.
- TypeScript: `npm run test:coverage` first runs `scripts/check-tested.sh`, the traceability check. Then it runs the Node built-in test runner with coverage thresholds. It adds no npm package.
- Traceability: every numbered requirement under "Requirements" in [Browser UI](browser-ui.md), and every IPC contract section the UI depends on, must be named by at least one test title. The script fails when a requirement has no test.
  - Tag a test title with `[browser-ui N]` for requirement N on the Browser UI page.
  - Tag it with `[ipc-contract <anchor>]` for an IPC contract section: `navigation-active-tab`, `window-layout`, `window`, `history-1` (History under Commands), `events`, `keyboard-shortcuts`. The list is `IPC_SECTIONS` in the script.
  - A test file no longer has to sit beside each module. That rule locked in the code layout.
- The job is not a required check yet. The owner adds it to the ruleset after it passes on main.

## Ratchet values

- Rust: lines 91, branches 82. CI on ubuntu-24.04 measured 91.64% lines and 82.23% branches on the nightly run after [#29](https://github.com/tcivie/eepview/pull/29). The floor is that value rounded down. Raise it when coverage rises. Never lower it.
- TypeScript: lines 80, branches 70, functions 80. [#49](https://github.com/tcivie/eepview/pull/49) removed the tests that lock in the implementation. Coverage is below the floor until the requirement tests land in that PR.

## How to use / run locally

- Rust: `cargo install cargo-llvm-cov --locked`, `rustup component add llvm-tools-preview`, then `cargo llvm-cov --all-targets --summary-only` in `src-tauri`. Run `npm run build` first.
- TypeScript: `npm run test` and `npm run test:coverage`. Node 22.18 or later is needed.

## Test rule

- A test checks a requirement, not the code. The requirement must be written in `docs/wiki/` or in an issue. ADR 0001 and the IPC contract land in `docs/wiki/` with the browser shell PR ([#29](https://github.com/tcivie/eepview/pull/29)).
- A test uses the public interface: an exported function whose output the requirement defines, an IPC command, an event or the UI.
- A test does not copy a constant from the code, check private state, snapshot today's output, repeat the algorithm, or exist only for coverage. Such tests are removed.
- The person who writes a requirement test does not read the code under test.
- The architecture test (lands with [#29](https://github.com/tcivie/eepview/pull/29)) and the leak test (lands with [#41](https://github.com/tcivie/eepview/pull/41)) enforce ADR 0001. They stay.

## Limits

- The Rust gate checks lines and branches, not functions or regions.
- Branch coverage needs a nightly compiler, so the branch number comes from one dated nightly. A nightly bump can move it a little.
- Test files are excluded from the TypeScript report only.
- Node counts only the files that a test loads. A module that no requirement test reaches does not count against the floor. Such a module is dead code: inline it or delete it.

## History

- 2026-10-03 — Add the coverage gate for Rust and TypeScript — [#13](https://github.com/tcivie/eepview/pull/13)
- 2026-10-03 — State the test rule; remove UI tests that lock in the implementation — [#49](https://github.com/tcivie/eepview/pull/49)
- 2026-10-03 — Raise the Rust gate to the measured floor: lines 91, branches 82 — [#58](https://github.com/tcivie/eepview/pull/58)
