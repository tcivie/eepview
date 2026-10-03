# Contributing to eepview

eepview is a small desktop browser for I2P sites. It uses Tauri 2, Rust, TypeScript, and the system WebView. The project is a work in progress. Small, focused changes are welcome.

## Prerequisites

- Rust. The version is pinned in `rust-toolchain.toml`. Rustup installs it for you.
- Node 24 and npm.
- On Linux, the WebKitGTK packages. The list is in `.github/workflows/ci.yml`.
- The tools that the git hooks use: lefthook, taplo, shellcheck, actionlint, zizmor (1.30.1 or newer), gitleaks, and lizard.

Install lizard with `pip install --no-deps --require-hashes -r scripts/requirements-lint.txt`. Install reuse with `pipx install 'reuse[charset-normalizer]'`. CI installs reuse from `scripts/requirements-reuse.txt`.

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

## Developer Certificate of Origin

eepview uses the [Developer Certificate of Origin](https://developercertificate.org/) (DCO). It says that you wrote the change, or that you have the right to send it under the project license.

Sign off every commit. Add the `-s` flag:

```sh
git commit -s -m "fix: reject a loopback address"
```

This adds a line like `Signed-off-by: Your Name <you@example.com>`. The name and email must match the commit author. The `dco` check fails a pull request that has an unsigned commit. To fix older commits, run `git rebase --signoff main` and push again.

## Licensing headers

Each source file starts with two SPDX lines: `SPDX-FileCopyrightText: 2026 The eepview contributors` and `SPDX-License-Identifier: MIT`. Use the comment syntax of the file type. Files that cannot hold a comment, such as JSON and images, are listed in `REUSE.toml`. Run `reuse lint` before you push. The git hook runs `reuse lint-file` on the staged files. See [Coding standards](https://github.com/tcivie/eepview/wiki/coding-standards).

## Tests

A new feature needs tests. A bug fix needs a regression test that fails before the fix. Read the [testing policy](https://github.com/tcivie/eepview/wiki/testing-policy).

## Wanted: co-maintainer

eepview has one maintainer. The project wants a second active maintainer who can review, merge and release. If you want to help, send good pull requests and open an issue. See [Governance](https://github.com/tcivie/eepview/wiki/governance) and [Access continuity](https://github.com/tcivie/eepview/wiki/access-continuity).

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

## Governance

Read [Governance](https://github.com/tcivie/eepview/wiki/governance) and [Roles](https://github.com/tcivie/eepview/wiki/roles) to see how decisions are made.

## Report a security issue

Do not use a public issue. Follow [SECURITY.md](SECURITY.md).
