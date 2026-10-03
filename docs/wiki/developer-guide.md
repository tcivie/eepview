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
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml` |
| UI tests | `npm test` |
| UI coverage gate | `npm run test:coverage` |
| Leak harness | see [Leak test](leak-test.md) (coming in v0.1) |
| Type check | `npm run typecheck` |

See [Coverage](coverage.md) for the ratchet.

## Lint

```sh
npm run lint
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
./scripts/complexity.sh
```

Install the hooks once with `lefthook install`. They run the fast checks before each commit. CI runs the full set. See [CI and quality gates](ci-and-quality-gates.md).

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
7. Add a line to `CHANGELOG.md`, open the PR, and run `gh pr merge --auto --squash`.
