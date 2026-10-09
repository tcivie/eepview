# CI and quality gates

Every change to main passes the fast lane: lint, complexity limits, security scans and tests. A change to a release branch also passes the heavy lane.

## How it works

CI has two lanes. The fast lane runs on every pull request and gives a result in minutes. The heavy lane holds the suites that take longer. It runs on release branches and every night.

### The fast lane

- `.github/workflows/ci.yml` is the fast lane. It runs on every pull request, on every push to main, and by hand.
- On a pull request, the `changes` job runs `scripts/ci-changes.sh`. The script compares the head of the pull request with the merge base and marks the areas that the change touches. A job runs only when its area is marked.
- On a push or a manual run, every area is marked.
- A change to `ci.yml`, `scripts/ci-changes.sh` or `scripts/ci-apt.sh` marks every area, so a change to the pipeline tests the whole pipeline.
- A new push to a pull request cancels the older run of that pull request.

| Area | Files that mark it | Jobs that run |
| --- | --- | --- |
| `rust_src` | `src-tauri/**`, `rust-toolchain.toml` | `rustfmt + clippy (ubuntu-24.04)`. It checks the fuzz crate too. |
| `rust` | The `rust_src` files, and the UI files: `src/**`, `index.html`, `package.json`, `package-lock.json`, `vite.config.ts`, `tsconfig*.json` | `rust tests (ubuntu-24.04)`, under cargo-nextest. The UI files mark it because the Rust tests read some UI files, and the app embeds the built UI. |
| `web` | The UI files, and `scripts/check-tested.sh` | `typescript (tsc + tests + coverage)` and `codeql (javascript-typescript)` |
| `macos` | A changed Rust file that holds macOS code. `src-tauri/*macos*`. `src-tauri/Info.plist`. The Cargo manifests and the lock file. `src-tauri/build.rs`. `src-tauri/tauri.conf.json`. `rust-toolchain.toml`. | `clippy + rust tests (macos-15)` |
| `windows` | The same rule for Windows code, and `src-tauri/*windows*`. The manifests, the lock file, `build.rs`, `tauri.conf.json` and `rust-toolchain.toml` mark it too. `Info.plist` does not. | `clippy + rust tests (windows-2025)` |
| `deps` | The Cargo manifests, `src-tauri/Cargo.lock`, `deny.toml`, `package.json`, `package-lock.json` | `cargo-deny (bans, sources)` |
| `workflows` | `.github/**` | `zizmor (workflow security)` |
| `leak` | The no-leak layer files under `src-tauri`: `src/net/gatekeeper*`, `src/net/rules.rs`, `src/net/host.rs`, `src/net/loopback.rs`, `src/nav.rs`, `src/shell/content.rs`, `src/shell/engine.rs`, `src/shell/webrtc.rs`, `crates/eepview-platform/**`, `capabilities/**`, `tauri.conf.json` and `Info.plist`. Also `tests/leak/**`, `scripts/leak-run.sh`, the manifests and the lock file, `build.rs` and `rust-toolchain.toml`. | `leak-test (ubuntu-24.04, debug)` and `leak harness (ruff + unit tests)` |

- The Linux jobs never compile code for macOS or Windows. So `ci-changes.sh` reads each changed Rust file at both ends of the change. A file that holds macOS code or Windows code marks that OS. A block that the change removes counts too.
- These jobs run on every pull request, whatever the change:
  - `repo checks`: biome, taplo, shellcheck, palette-check, style-check, the tests of `ci-changes.sh`, lizard complexity, reuse and actionlint.
  - `gitleaks (full history)`.
  - `codeql (actions)`. It runs on every change, so every change keeps a static analysis result for the Scorecard SAST check.
  - `dco`. It checks that every commit has a `Signed-off-by` line that matches its author. Dependabot commits are skipped.
  - `dependency-review`. It fails a pull request that adds a dependency with a high severity advisory.
  - `score-ratchet (pr)`. See [Score ratchet](score-ratchet.md).
- `dco`, `dependency-review` and `score-ratchet (pr)` run on pull requests only. The other jobs of this list also run on a push to main.
- The `gate` job needs every other job of `ci.yml`. It runs with `always()`. It passes only when each job passed or was skipped. A skipped job means that its area did not change. A failed or cancelled job fails the gate. `always()` is there because a skipped gate would count as a pass.
- Three settings make the lane fast:
  - `ci.yml` sets `CARGO_PROFILE_DEV_DEBUG=line-tables-only`. Panics keep `file:line`, and the build and the cache are smaller.
  - `rust-cache` saves only from main. A pull request reads the cache of main and does not evict it.
  - `scripts/ci-apt.sh` starts the WebKitGTK `apt` install in the background. A later step waits for it. So the install runs at the same time as the toolchain, cache and npm steps.
- The Rust jobs run the tests under cargo-nextest. See [Testing policy](testing-policy.md).

### The heavy lane

- `.github/workflows/heavy.yml` is the heavy lane. It runs on pull requests into `release/**`, every night at 02:23 UTC on main, and by hand on any branch: `gh workflow run heavy.yml --ref <branch>`. A manual run needs the workflow on main.
- The jobs:
  - `coverage (rust lines + typescript)`. See [Coverage](coverage.md).
  - `coverage (rust branches)`. It uses the pinned nightly toolchain.
  - `codeql (rust)`.
  - `leak-test (<os>, release)` on ubuntu-24.04, macos-15 and windows-2025. See [Leak test](leak-test.md).
  - `leak harness (ruff + unit tests)`.
- The `heavy-gate` job needs these jobs. It passes only when every one of them passed. A skipped or cancelled job fails it.
- When the nightly run fails, `heavy-gate` opens the issue "Nightly heavy suite failed". When that issue is open, it adds a comment to it.

### Release branches

- Cut a release branch from main: `git push origin <sha>:refs/heads/release/v0.1`.
- A ruleset on `refs/heads/release/**` blocks deletion and force pushes. It requires a pull request (squash), and it requires the checks `gate` and `heavy-gate`.
- A change reaches a release branch only through a pull request. That pull request runs both lanes.
- No workflow runs on a push to a release branch. The zizmor cache-poisoning audit flags a cache in a workflow that runs on such a push, and `release.yml` builds without caches.
- To put `heavy-gate` on the cut commit itself, run `heavy.yml` on the release branch by hand.
- A tag `v*` on a release branch starts the release. See [Release pipeline](release-pipeline.md).

### The other workflows

- `.github/workflows/scorecard.yml` runs OpenSSF Scorecard, publishes the result and uploads the SARIF to code scanning.
- `.github/workflows/fuzz.yml` runs ClusterFuzzLite on the cargo-fuzz targets every night and on demand. It does not run on pull requests. It is not a required check. See [Fuzzing](fuzzing.md).
- `.github/workflows/score-ratchet.yml` keeps the `published` job of the [Score ratchet](score-ratchet.md).
- `.github/workflows/docs-check.yml` runs the `docs-check` job. It stays its own workflow, because it must run again when the title of the pull request changes. The job (`scripts/docs-check.sh`) fails a `feat` PR that changes no page under `docs/wiki/`, and any PR that edits `CHANGELOG.md`. It also checks that every wiki page is indexed. `scripts/docs-check.test.sh` tests the check.
- The old workflows `lint.yml`, `security.yml`, `codeql.yml`, `dco.yml`, `dependency-review.yml` and `leak.yml` are gone. Their jobs moved into `ci.yml` or `heavy.yml`.

### Rules

- Required checks on main: `gate`, `docs-check`, `Socket Security: Project Report` and `Socket Security: Pull Request Alerts`. A repository ruleset blocks direct pushes to main. A pull request needs these checks green.
- Required checks on a release branch: `gate` and `heavy-gate`.
- rustfmt and clippy run with `pedantic` and `-D warnings`. The `reuse` tool and its build backend are pinned by hash in `scripts/requirements-reuse.txt` and `scripts/requirements-reuse-build.txt`.
- zizmor runs with the pedantic persona in CI and in lefthook. Every write permission and every non-default read permission has a comment that says why. CI pins the zizmor version (1.30.1).
- Complexity limits: cognitive and cyclomatic complexity 10 or less, 40 lines per function, 5 parameters, nesting 3.
- No lint exclusions exist. The code is fixed instead.
- Socket reviews every dependency change.

## How to use / run locally

- Install the hooks once: `lefthook install`.
- Rust: `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings`.
- Rust tests: `cargo nextest run --manifest-path src-tauri/Cargo.toml`.
- Web: `npx biome ci .` and `npm run typecheck`.
- Complexity: `./scripts/complexity.sh`.
- Licenses: `reuse lint`.
- Sign-off: use `git commit -s`.
- See the areas that a branch marks: `./scripts/ci-changes.sh origin/main HEAD`. Run `./scripts/ci-changes.test.sh` to test the script.
- Run the heavy lane on a branch: `gh workflow run heavy.yml --ref <branch>`.

## Limits

- The Scorecard badge shows "invalid repo path" until the first Scorecard run is published.
- The Rust jobs need the WebKitGTK packages on Linux.
- A pull request into main does not run the Rust coverage gate. The nightly run and every pull request into a release branch run it.
- The fast lane does not build the app in release mode. The heavy lane does, in the leak test.
- There is no reusable workflow and no local composite action. zizmor 1.30.1 (pedantic) asks for the new `$/` self-repository syntax for `uses: ./...`, and actionlint 1.7.12 rejects that syntax. So the leak steps that the two lanes share are a script, `scripts/leak-run.sh`.

## History

- 2026-10-03 — Repo bootstrap: skeleton, linters, security scans, CI — [#1](https://github.com/tcivie/eepview/pull/1)
- 2026-10-03 — Remove the warnings from the CI logs — [#3](https://github.com/tcivie/eepview/pull/3)
- 2026-10-03 — Remove every clippy lint exclusion — [#5](https://github.com/tcivie/eepview/pull/5)
- 2026-10-03 — Remove CodeQL and Scorecard until the repo is public — [#9](https://github.com/tcivie/eepview/pull/9)
- 2026-10-03 — Cut dependencies flagged by Socket — [#10](https://github.com/tcivie/eepview/pull/10)
- 2026-10-03 — Hash-pinned CI tools and typed vite config — [#15](https://github.com/tcivie/eepview/pull/15)
- 2026-10-03 — Add the docs-check job: code changes need a docs or changelog update — [#16](https://github.com/tcivie/eepview/pull/16)
- 2026-10-03 — Restore CodeQL and Scorecard, add dependency review and audit badges — [#26](https://github.com/tcivie/eepview/pull/26)
- 2026-10-03 — Add `reuse lint` and the DCO check — [#30](https://github.com/tcivie/eepview/pull/30)
- 2026-10-03 — Run zizmor with the pedantic persona; document every workflow permission — [#47](https://github.com/tcivie/eepview/pull/47)
- 2026-10-03 — docs-check: a feat PR needs a wiki change, and CHANGELOG.md is generated, not edited — [#50](https://github.com/tcivie/eepview/pull/50)
- 2026-10-09 — Fast lane for pull requests, heavy lane for release branches — [#89](https://github.com/tcivie/eepview/pull/89)
