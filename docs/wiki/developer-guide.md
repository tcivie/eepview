<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/tcivie/eepview/main/assets/brand/eepview-logo-horizontal-dark.svg">
    <img alt="eepview" src="https://raw.githubusercontent.com/tcivie/eepview/main/assets/brand/eepview-logo-horizontal-light.svg" width="280">
  </picture>
</p>

# Developer guide

Status: the browser shell is in a PR. Parts marked "(coming in v0.1)" are not on main yet.

Read [AGENTS.md](https://github.com/tcivie/eepview/blob/main/AGENTS.md) first. It holds the rules.

## Repo layout

| Path | Holds |
| --- | --- |
| `src-tauri/` | The Rust core: Tauri app, router logic, proxy, window shell. |
| `src/ui/` | The bundled pages: toolbar, internal pages, styles, mock data. |
| `tests/leak/` | The leak test harness (coming in v0.1). |
| `scripts/` | CI and lint helper scripts, wiki publishing. |
| `docs/wiki/` | All docs. CI publishes them to the GitHub wiki. |

## Architecture

One OS window holds three kinds of web view. (Coming in v0.1.)

```
+---------------- window ------------------+
| toolbar   (bundled, IPC)                 |
+------------------------------------------+
| internal  (bundled, IPC)  eepview://...  |   shown for internal pages
| tab-<n>   (remote, NO IPC)               |   one per web tab
+------------------------------------------+
   tab-<n> --> gatekeeper (127.0.0.1) --> I2P router --> I2P network
   Rust core --I2PControl--> router   (verify, status, stats)
```

- `toolbar` and `internal` load bundled pages and may call Rust commands.
- `tab-<n>` loads a remote `.i2p` page and has no IPC. A hostile site cannot reach a command.
- The gatekeeper is a loopback proxy in Rust. It forwards `.i2p` hosts only.
- No `tab-<n>` exists before the router passes verification.

Read [No-leak architecture](no-leak-architecture.md) and the [Browser shell](browser-shell.md) page before you touch networking or web views. The command and event list is in the IPC contract (coming in v0.1).

## Build, run, test

Requirements: Rust 1.96 (pinned in `rust-toolchain.toml`), Node.js 22 or later, and the OS libraries for Tauri.

```sh
npm ci
npm run tauri dev
```

| Task | Command |
| --- | --- |
| Rust tests | `cargo nextest run --manifest-path src-tauri/Cargo.toml` |
| UI tests | `npm test` |
| UI coverage gate | `npm run test:coverage` |
| Leak harness | see [Leak test](leak-test.md) (coming in v0.1) |
| Type check | `npm run typecheck` |

See [Coverage](coverage.md) for the ratchet.

## Fast local loop

- Run the Rust tests with cargo-nextest: `cargo nextest run --manifest-path src-tauri/Cargo.toml`. Install it with `brew install cargo-nextest` or `cargo install cargo-nextest --locked`. It runs each test in its own process, and it is the runner that CI uses. See [Testing policy](testing-policy.md).
- Keep one `CARGO_TARGET_DIR` for every worktree. The dependencies then build once.
- sccache is optional. Set `RUSTC_WRAPPER=sccache` and it caches the dependency builds across target dirs.

## Lint

```sh
npm run lint
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
./scripts/complexity.sh
```

Install the hooks once with `lefthook install`. They run the fast checks before each commit. CI runs the full set. A pull request runs the fast lane, and only the jobs of the areas that it changes. A pull request into a release branch also runs the heavy lane. See [CI and quality gates](ci-and-quality-gates.md).

## The rules

- No lint exclusions of any kind.
- Functions have 40 lines or less, 5 parameters or less, nesting 3 or less. Complexity is 10 or less.
- No new HTTP client crates.
- Pin actions by full SHA.
- No clearnet code path without explicit user consent.
- Never use `--no-verify` or `--admin`.

The full list is in [AGENTS.md](https://github.com/tcivie/eepview/blob/main/AGENTS.md).

## Add a feature end to end

1. Start a branch from main.
2. Contract: add the command, event or type to the IPC contract and to `src/ui/contract.ts`.
3. Rust: build the command in `src-tauri/src/`. Keep pure logic in small modules. Touch no networking or web view code outside the places the architecture test allows.
4. UI: call the command from `src/ui/`. Add the page or control. Check light and dark.
5. Tests: unit tests for every pure module. Coverage only goes up. Run the architecture and leak tests if you touched networking.
6. Wiki: add or update the feature page in `docs/wiki/`, link it from `docs/wiki/README.md`, and add a History line with the PR link.
7. Open the PR with a clear Conventional Commit title, and turn on auto-merge with `gh pr merge <n> --squash --auto`. Own the PR until it is merged: fix a red check at once, and resolve review threads while CI runs. Merge main in only when the PR has a conflict (DIRTY). If a check is red on main too, report it and do not change the gate in your PR. See the Workflow section of AGENTS.md. Do not edit `CHANGELOG.md`. The release job generates it from the PR titles.

## History

- 2026-10-03 — Write the page — [#28](https://github.com/tcivie/eepview/pull/28)
- 2026-10-03 — Do not edit CHANGELOG.md in a PR; the release job generates it — [#50](https://github.com/tcivie/eepview/pull/50)
- 2026-10-03 — PR ownership rules: auto-merge, first red check, threads during CI, merge main on DIRTY — [#67](https://github.com/tcivie/eepview/pull/67)
- 2026-10-09 — Fast lane for pull requests, heavy lane for release branches — [#89](https://github.com/tcivie/eepview/pull/89)
