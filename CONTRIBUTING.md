# Contributing to eepview

eepview is a small desktop browser for I2P sites. It uses Tauri 2, Rust, TypeScript, and the system WebView. The project is a work in progress. Small, focused changes are welcome.

## Prerequisites

- Rust. The version is pinned in `rust-toolchain.toml`. Rustup installs it for you.
- Node 24 and npm.
- On Linux, the WebKitGTK packages. The list is in `.github/workflows/ci.yml`.
- The tools that the git hooks use: lefthook, taplo, shellcheck, actionlint, gitleaks, and lizard.

Install lizard with `pip install -r scripts/requirements-lint.txt`.

## Setup

```sh
npm ci
lefthook install
npm run tauri dev
```

## Workflow

See the [roadmap](https://github.com/tcivie/eepview/wiki/roadmap) for what comes next.

1. Branch from `main`. Use a prefix: `feat/`, `fix/`, `docs/`, `ci/`, or `chore/`.
2. Make small commits. Write each message in the Conventional Commits format, for example `fix: reject a loopback address`.
3. Open one pull request for each change.
4. Make sure that all required checks pass.
5. A maintainer merges the pull request with a squash merge.

Nobody can push to `main` directly. Do not bypass the hooks with `--no-verify`.

## Code rules

The hooks and CI enforce these rules.

**Rust**

- Clippy runs with `pedantic` and `-D warnings`.
- `unsafe_code` is set to `deny`.
- Cognitive complexity is 10 or less.
- A function has 40 lines or fewer.
- A function has 5 parameters or fewer.
- Nesting is 3 levels or fewer.
- The limits are in `src-tauri/Cargo.toml` and `src-tauri/clippy.toml`.

**TypeScript**

- TypeScript runs in strict mode.
- Biome checks the lint rules and the format. Run `npx biome check .`.
- Biome limits cognitive complexity to 10 and a function to 40 lines.

**Both languages**

- `scripts/complexity.sh` runs lizard. It limits cyclomatic complexity to 10, a function to 40 lines, and parameters to 5.

Do not add a lint exclusion of any kind. Fix the code instead.

## Dependencies

Avoid new dependencies. If you add one, give the reason in the pull request. The dependency must pass the Socket.dev review.

cargo-deny allows crates from crates.io only. It blocks wildcard versions. npm install scripts are denied by default. The `allowScripts` field in `package.json` lists the exceptions.

## Privacy rules

- Add no telemetry.
- Add no external fonts, CDNs, or remote assets to the bundled pages.
- Add no new network path that breaks the "No-leak design" or "Consent first" principles. These documents arrive in docs/ with Phase 1.

## Report a security issue

Do not use a public issue. Follow [SECURITY.md](SECURITY.md).
