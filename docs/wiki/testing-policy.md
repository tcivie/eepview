# Testing policy

Status: shipped.

## Rules

- A new feature needs tests in the same pull request.
- A bug fix needs a regression test. The test must fail before the fix and pass after it. Say in the PR how you saw it fail.
- Every pure module has unit tests. In TypeScript, a module in `src/ui/lib` or `src/ui/shared` needs a sibling `.test.ts` file. `scripts/check-tested.sh` fails if it is missing.
- Coverage only goes up. The ratchet values are in `.github/workflows/ci.yml` and `package.json`. Raise them when coverage rises. Never lower them. See [Coverage](coverage.md).
- A change to the no-leak design needs a test too. The architecture test and the leak test must stay green. See [Leak test](leak-test.md).
- Add no coverage exclusion and no skipped test.

## How to run all suites

```sh
npm ci
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm test
npm run test:coverage
```

- `cargo test` runs the Rust unit tests and the integration tests.
- `npm test` runs the TypeScript unit tests with the Node test runner.
- `npm run test:coverage` checks the TypeScript coverage thresholds.
- For Rust coverage, run `cargo llvm-cov --all-targets --summary-only` in `src-tauri`.
- The full set of checks also runs in CI on Linux, macOS and Windows.

## Limits

- Today the Rust coverage floor is 0, because the first Rust tests arrive with the browser shell. Raise it in that PR.
- DOM glue outside `src/ui/lib` and `src/ui/shared` is not covered.

## History

- 2026-10-03 — Add the testing policy — [#30](https://github.com/tcivie/eepview/pull/30).
