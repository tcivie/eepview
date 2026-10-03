# Coverage

Status: shipped.

Unit-test coverage gate for Rust and TypeScript.

## How it works

- The `coverage` job in `.github/workflows/ci.yml` runs on ubuntu-24.04 only.
- Rust: `cargo llvm-cov --all-targets --summary-only`, then `cargo llvm-cov report --fail-under-lines <N>`. Every file counts. There are no exclusions.
- TypeScript: `npm run test:coverage` first runs `scripts/check-tested.sh`. Node counts only files that a test loads, so the script fails when a module in `src/ui/lib` or `src/ui/shared` has no sibling `.test.ts`. Then it runs the Node built-in test runner with coverage thresholds. It adds no npm package.
- The job is not a required check yet. The owner adds it to the ruleset after it passes on main.

## Ratchet values

- Rust lines: 0. Today there are no Rust tests, so coverage is 0%. Raise the number when coverage rises. Never lower it.
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

- The Rust gate checks lines only, not branches or functions.
- Test files are excluded from the TypeScript report only.
- DOM glue outside `src/ui/lib` and `src/ui/shared` is not loaded by any test, so TypeScript coverage does not count it.
- The Rust floor is 0 because there are no Rust tests yet. The gate checks nothing until the first test lands. Raise it in that PR.

## History

- 2026-10-03 — Add the coverage gate for Rust and TypeScript — [#13](https://github.com/tcivie/eepview/pull/13)
- 2026-10-03 — State the test rule; remove UI tests that lock in the implementation — [#49](https://github.com/tcivie/eepview/pull/49)
