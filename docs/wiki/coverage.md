# Coverage

Status: shipped.

Unit-test coverage gate for Rust and TypeScript.

## How it works

- The coverage jobs are in `.github/workflows/heavy.yml`, the heavy lane. They moved there from `ci.yml`, so a pull request into main does not wait for them. They run on pull requests into a `release/**` branch, every night on main, and by hand. They run on ubuntu-24.04 only. See [CI and quality gates](ci-and-quality-gates.md).
- Lines and branches are two jobs. They run in parallel. Both run the tests under `cargo llvm-cov nextest`, one process per test. See [Testing policy](testing-policy.md).
- Rust lines (stable toolchain), job `coverage (rust lines + typescript)`: `cargo llvm-cov nextest --workspace --locked --summary-only --fail-under-lines <N>`. The gate is on the run itself, because `cargo llvm-cov report` has no `--workspace` and would gate the root crate only. Every file of both crates counts. There are no exclusions.
- Rust branches (pinned nightly, named in `scripts/nightly-toolchain.txt`), job `coverage (rust branches)`: the same tests with `--branch --json`. cargo-llvm-cov has no `--fail-under-branches`, so `jq` prints `totals.branches.percent` and fails below the floor.
- TypeScript: `npm run test:coverage` first runs `scripts/check-tested.sh`, the traceability check. Then it runs the Node built-in test runner with coverage thresholds. It adds no npm package. The lines job runs it, and so does the `typescript (tsc + tests + coverage)` job of the fast lane when a pull request changes the UI files.
- The runner does not change the number. On macOS, nextest measured 91.76% lines, and plain `cargo test` measured 91.70%.
- Traceability: every numbered requirement under "Requirements" in [Browser UI](browser-ui.md), and every IPC contract section the UI depends on, must be named by at least one test title. The script fails when a requirement has no test.
  - Tag a test title with `[browser-ui N]` for requirement N on the Browser UI page.
  - Tag it with `[ipc-contract <anchor>]` for an IPC contract section: `navigation-active-tab`, `window-layout`, `window`, `history-1` (History under Commands), `events`, `keyboard-shortcuts`. The list is `IPC_SECTIONS` in the script.
  - A test file no longer has to sit beside each module. That rule locked in the code layout.
- On a release branch, the `heavy-gate` check needs both coverage jobs to pass. A nightly failure opens the issue "Nightly heavy suite failed".

## Ratchet values

- Rust lines: 93. The stable run on ubuntu-24.04 measured 93.96% after [#29](https://github.com/tcivie/eepview/pull/29).
- Rust branches: 82. The nightly run on ubuntu-24.04 measured 82.23% after #29.
- Each floor is the measured value of its own run, rounded down. Raise it when coverage rises. Never lower it. The floors did not change when the jobs moved to `heavy.yml`.
- TypeScript: lines 80, branches 70, functions 80. [#49](https://github.com/tcivie/eepview/pull/49) removed the tests that lock in the implementation. Coverage is below the floor until the requirement tests land in that PR.

## How to use / run locally

- Rust: `cargo install cargo-llvm-cov --locked`, `rustup component add llvm-tools-preview`, then in `src-tauri`:
  - Install the runner once: `brew install cargo-nextest` or `cargo install cargo-nextest --locked`.
  - lines: `cargo llvm-cov nextest --workspace --summary-only`
  - branches: `rustup toolchain install "$(cat scripts/nightly-toolchain.txt)" --component llvm-tools-preview`, then `cargo "+$(cat scripts/nightly-toolchain.txt)" llvm-cov nextest --workspace --branch --summary-only`
  - Run `npm run build` first. CI measures on Linux; macOS and Windows compile other platform code, so their numbers differ.
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
- CI builds and runs the instrumented tests twice: on stable for lines, on the nightly for branches. The line gate stays on the toolchain that ships the app. The two jobs run in parallel.
- The fast lane does not run the Rust coverage gate. A coverage drop shows on the nightly run or on a pull request into a release branch.
- Test files are excluded from the TypeScript report only.
- Node counts only the files that a test loads. A module that no requirement test reaches does not count against the floor. Such a module is dead code: inline it or delete it.

## History

- 2026-10-03 — Add the coverage gate for Rust and TypeScript — [#13](https://github.com/tcivie/eepview/pull/13)
- 2026-10-03 — State the test rule; remove UI tests that lock in the implementation — [#49](https://github.com/tcivie/eepview/pull/49)
- 2026-10-03 — Raise the Rust gate to the measured floor: lines 93, branches 82 — [#58](https://github.com/tcivie/eepview/pull/58)
- 2026-10-09 — Fast lane for pull requests, heavy lane for release branches — [#89](https://github.com/tcivie/eepview/pull/89)
