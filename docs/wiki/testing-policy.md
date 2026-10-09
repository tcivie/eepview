# Testing policy

Status: shipped.

## Rules

- A new feature needs tests in the same pull request.
- A bug fix needs a regression test. The test must fail before the fix and pass after it. Say in the PR how you saw it fail.
- Every pure module has unit tests. In TypeScript, a module in `src/ui/lib` or `src/ui/shared` needs a sibling `.test.ts` file. `scripts/check-tested.sh` fails if it is missing.
- Coverage only goes up. The ratchet values are in `.github/workflows/heavy.yml` and `package.json`. Raise them when coverage rises. Never lower them. See [Coverage](coverage.md).
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
cargo nextest run --manifest-path src-tauri/Cargo.toml --locked
npm test
npm run test:coverage
```

- `cargo nextest run` runs the Rust unit tests and the integration tests. Install it with `brew install cargo-nextest` or `cargo install cargo-nextest --locked`.
- `npm test` runs the TypeScript unit tests with the Node test runner.
- `npm run test:coverage` checks the TypeScript coverage thresholds.
- For Rust coverage, run `cargo llvm-cov nextest --workspace --summary-only` in `src-tauri`.
- The Rust tests run in CI on Linux for each change to the Rust or UI files. They also run on macOS and on Windows when a change touches the code of that OS. The coverage jobs and the release-build leak test run in the heavy lane. See [CI and quality gates](ci-and-quality-gates.md).

## The test runner

Rust tests run under cargo-nextest, in CI and on your machine. It runs each test in its own process.

- The reason is time. Some `net::console` tests wait on each other for more than 60 s. In the single process of `cargo test`, the lib tests took 218 s. Under nextest, 1,596 tests took 33 s on 8 cores.
- The config is `src-tauri/.config/nextest.toml`.
- A test that runs for 2 minutes (30 s, four times) is stopped and reported as hung.
- The `ci` profile does not stop at the first failure. It runs every test and prints the output of each failure when it happens and again at the end. CI uses it with `--profile ci`.
- Tests that bind the default router console ports (7657 and 7070) belong to the `console-ports` test group. The group runs one test at a time, so none of them finds a port taken by another.
- The crates have no doc tests, so nextest runs every test.
- The coverage is the same under both runners. See [Coverage](coverage.md).

## Limits

- Today the Rust coverage floor is 0, because the first Rust tests arrive with the browser shell. Raise it in that PR.
- DOM glue outside `src/ui/lib` and `src/ui/shared` is not covered.

## History

- 2026-10-03 — Add the testing policy — [#30](https://github.com/tcivie/eepview/pull/30).
- 2026-10-09 — Fast lane for pull requests, heavy lane for release branches — [#89](https://github.com/tcivie/eepview/pull/89).
