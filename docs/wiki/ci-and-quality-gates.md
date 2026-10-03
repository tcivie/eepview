# CI and quality gates

Every change to main passes lint, complexity limits, security scans and a ruleset.

## How it works

- `.github/workflows/lint.yml` runs rustfmt and clippy (pedantic, `-D warnings`), biome and tsc, taplo, shellcheck, lizard complexity and actionlint.
- `.github/workflows/ci.yml` builds and tests the app.
- `.github/workflows/security.yml` runs cargo-deny, gitleaks and zizmor. zizmor runs with `--persona=pedantic` in CI and in lefthook, and every permission has a comment that says why.
- `.github/workflows/codeql.yml` runs CodeQL (`security-extended`) for actions, javascript-typescript and rust. Each matrix entry has a fixed job name, `codeql (<language>)`.
- `.github/workflows/scorecard.yml` runs OpenSSF Scorecard, publishes the result and uploads the SARIF to code scanning.
- `.github/workflows/dependency-review.yml` fails a PR that adds a dependency with a high severity advisory.
- Complexity limits: cognitive and cyclomatic complexity 10 or less, 40 lines per function, 5 parameters, nesting 3.
- No lint exclusions exist. The code is fixed instead.
- Socket reviews every dependency change.
- The `docs-check` job (`scripts/docs-check.sh`) fails a PR that changes code or workflows without a CHANGELOG or docs/wiki update. It also checks that every wiki page is indexed.
- A repository ruleset blocks direct pushes to main. A pull request needs green required checks.

## How to use / run locally

- Install the hooks once: `lefthook install`.
- Rust: `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings`.
- Web: `npx biome ci .` and `npm run typecheck`.
- Complexity: `./scripts/complexity.sh`.

## Limits

- The Scorecard badge shows "invalid repo path" until the first Scorecard run is published.
- The Rust job needs the WebKitGTK packages on Linux.

## History

- 2026-10-03 — Repo bootstrap: skeleton, linters, security scans, CI — [#1](https://github.com/tcivie/eepview/pull/1)
- 2026-10-03 — Remove the warnings from the CI logs — [#3](https://github.com/tcivie/eepview/pull/3)
- 2026-10-03 — Remove every clippy lint exclusion — [#5](https://github.com/tcivie/eepview/pull/5)
- 2026-10-03 — Remove CodeQL and Scorecard until the repo is public — [#9](https://github.com/tcivie/eepview/pull/9)
- 2026-10-03 — Cut dependencies flagged by Socket — [#10](https://github.com/tcivie/eepview/pull/10)
- 2026-10-03 — Hash-pinned CI tools and typed vite config — [#15](https://github.com/tcivie/eepview/pull/15)
- 2026-10-03 — Add the docs-check job: code changes need a docs or changelog update — [#16](https://github.com/tcivie/eepview/pull/16)
- 2026-10-03 — Restore CodeQL and Scorecard, add dependency review and audit badges — [#26](https://github.com/tcivie/eepview/pull/26)
- 2026-10-03 — Run zizmor with the pedantic persona; document every workflow permission — [#47](https://github.com/tcivie/eepview/pull/47)
