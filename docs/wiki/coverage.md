# Coverage

Status: shipped.

Unit-test coverage gate for Rust and TypeScript.

## How it works

- The `coverage` job in `.github/workflows/ci.yml` runs on ubuntu-24.04 only.
- Rust: `cargo llvm-cov --all-targets --summary-only`, then `cargo llvm-cov report --fail-under-lines <N>`. Every file counts. There are no exclusions.
- TypeScript: `npm run test:coverage` runs the Node built-in test runner with coverage thresholds. It adds no npm package.
- The job is not a required check yet. The owner adds it to the ruleset after it passes on main.

## Ratchet values

- Rust lines: 0. Today there are no Rust tests, so coverage is 0%. Raise the number when coverage rises. Never lower it.
- TypeScript: lines 80, branches 70, functions 80. There is no `src/**/*.test.ts` file yet, so the run passes with zero tests.

## How to use / run locally

- Rust: `cargo install cargo-llvm-cov --locked`, `rustup component add llvm-tools-preview`, then `cargo llvm-cov --all-targets --summary-only` in `src-tauri`. Run `npm run build` first.
- TypeScript: `npm run test` and `npm run test:coverage`. Node 22.18 or later is needed.

## Limits

- The Rust gate checks lines only, not branches or functions.
- Test files are excluded from the TypeScript report only.

## History

- 2026-10-03 — Add the coverage gate for Rust and TypeScript — [#13](https://github.com/tcivie/eepview/pull/13)
