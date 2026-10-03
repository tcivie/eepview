# Coding standards

Status: shipped.

This page lists the style rules, the tool that enforces each one, and what a reviewer checks.

## Style guides and enforcement

| Language or file | Standard | Enforced by |
| --- | --- | --- |
| Rust | `rustfmt` format. Clippy `pedantic` with `-D warnings`. `unsafe_code` is denied. | `cargo fmt --check` and `cargo clippy` in `lint.yml` and in lefthook. Config in `src-tauri/Cargo.toml` and `src-tauri/clippy.toml`. |
| TypeScript and JavaScript | Biome lint and format. `tsc` in strict mode. | `npx biome ci .` and `npm run typecheck` in `lint.yml`. Biome runs in lefthook. |
| HTML and CSS | Biome format. | Biome. |
| TOML | `taplo` format and lint. | `lint.yml` and lefthook. |
| Shell | `shellcheck`. `set -euo pipefail`. | `lint.yml` and lefthook. |
| GitHub workflows | `actionlint`, `zizmor`. Actions pinned by full SHA. `permissions: {}` with the minimum per job. `persist-credentials: false`. | `lint.yml`, `security.yml`, lefthook. |
| Python | Python is used only for the lizard stubs in `scripts/lizard-stubs`. There is no Python linter in CI today. Keep these files small. | Review. |
| License headers | Each source file has an SPDX header. Other files are covered by `REUSE.toml`. | `reuse lint` in `lint.yml` and in lefthook. |
| Commit sign-off | Each commit has a `Signed-off-by` line. | The `dco` workflow. |
| Docs | All docs live in `docs/wiki/`. | `docs-check`. |

## Complexity limits

- Cognitive and cyclomatic complexity are 10 or less.
- A function has 40 lines or fewer.
- A function has 5 parameters or fewer.
- Nesting is 3 levels or fewer.
- Clippy, Biome and `scripts/complexity.sh` (lizard) enforce these limits.

## No exclusions

Nothing may switch a check off. That means no `#[allow]`, `#[expect]`, `biome-ignore`, `@ts-ignore`, `@ts-expect-error`, `noqa`, `shellcheck disable`, no config relaxation and no coverage exclusion. Fix the code instead. One known exception exists in `release.yml`. It is tracked in the [security review](security-review.md).

## Code review

Every change goes through a pull request. A reviewer checks:

- **Tests.** New code has unit tests. A bug fix has a regression test that fails before the fix. See [Testing policy](testing-policy.md).
- **Docs.** The `docs/wiki` page is updated. `CHANGELOG.md` has a line with the PR link.
- **No-leak invariants.** The change adds no clearnet code path. All five layers stay. The architecture test and the leak test stay green. No webview gets IPC for web content.
- **Dependencies.** No new dependency without a reason in the PR. No new HTTP client crate. The dependency passes Socket.
- **Quality gates.** No lint exclusion, no coverage exclusion, no complexity above the limits.
- **Workflows.** Actions are pinned by SHA. Permissions are the minimum.
- **Privacy.** No telemetry, no remote fonts, no CDN, no remote asset.
- **Sign-off.** Every commit has `Signed-off-by`.

The maintainer reviews pull requests from contributors. Automated checks review the maintainer's own pull requests. A second human reviewer is not available today. See [Governance](governance.md).

## History

- 2026-10-03 — Add the coding standards page — [#30](https://github.com/tcivie/eepview/pull/30).
