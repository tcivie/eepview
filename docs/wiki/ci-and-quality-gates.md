# CI and quality gates

Every change to main passes lint, complexity limits, security scans and a ruleset.

## How it works

- `.github/workflows/lint.yml` runs rustfmt and clippy (pedantic, `-D warnings`), biome and tsc, taplo, shellcheck, lizard complexity and actionlint.
- `.github/workflows/ci.yml` builds and tests the app.
- `.github/workflows/security.yml` runs cargo-deny, gitleaks and zizmor.
- Complexity limits: cognitive and cyclomatic complexity 10 or less, 40 lines per function, 5 parameters, nesting 3.
- No lint exclusions exist. The code is fixed instead.
- Socket reviews every dependency change.
- A repository ruleset blocks direct pushes to main. A pull request needs green required checks.

## How to use / run locally

- Install the hooks once: `lefthook install`.
- Rust: `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings`.
- Web: `npx biome ci .` and `npm run typecheck`.
- Complexity: `./scripts/complexity.sh`.

## Limits

- CodeQL and Scorecard are off until the repo is public. See [going-public](../going-public.md).
- The Rust job needs the WebKitGTK packages on Linux.

## History

- 2026-10-03 — Repo bootstrap: skeleton, linters, security scans, CI — [#1](https://github.com/tcivie/eepview/pull/1)
- 2026-10-03 — Remove the warnings from the CI logs — [#3](https://github.com/tcivie/eepview/pull/3)
- 2026-10-03 — Remove every clippy lint exclusion — [#5](https://github.com/tcivie/eepview/pull/5)
- 2026-10-03 — Remove CodeQL and Scorecard until the repo is public — [#9](https://github.com/tcivie/eepview/pull/9)
- 2026-10-03 — Cut dependencies flagged by Socket — [#10](https://github.com/tcivie/eepview/pull/10)
- 2026-10-03 — Hash-pinned CI tools and typed vite config — [#15](https://github.com/tcivie/eepview/pull/15)
