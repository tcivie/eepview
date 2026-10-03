# Testing policy

Status: shipped.

## Rules

- A new feature needs tests in the same pull request.
- A bug fix needs a regression test. The test must fail before the fix and pass after it. Say in the PR how you saw it fail.
- Every pure module has unit tests. In TypeScript, a module in `src/ui/lib` or `src/ui/shared` needs a sibling `.test.ts` file. `scripts/check-tested.sh` fails if it is missing.
- Coverage only goes up. The ratchet values are in `.github/workflows/ci.yml` and `package.json`. Raise them when coverage rises. Never lower them. See [Coverage](coverage.md).
- A change to the no-leak design needs a test too. The architecture test and the leak test must stay green. See [Leak test](leak-test.md).
- Add no coverage exclusion and no skipped test.

## Tests check requirements, not code

Tests validate the requirements of the product owner (PO). They do not validate the code.

- The agent that writes the tests is not the agent that writes the code.
- The test writer gets only the requirement and the public interface. The requirement is the issue, the ADR, a `docs/wiki` page or the IPC contract. The public interface is the function signatures, the IPC commands, the events and the UI.
- The test writer does not read the implementation. It does not care how the code works.
- Each test names the requirement it checks. Use a link to the doc, or write the requirement text in the test name or in a comment.
- These tests are not allowed: a test that copies constants from the code, a test that asserts private state, a test that calls internal helpers, and a test that snapshots whatever the code prints today.
- When the code and the test disagree, the requirement decides. Never edit a test to match the code, unless the PO approves a change to the requirement first.

The flow:

1. Write down the requirement.
2. A blind test writer writes the tests from the requirement and the public interface.
3. The tests fail.
4. The implementer makes them pass.
5. Refactor with the tests green.

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
