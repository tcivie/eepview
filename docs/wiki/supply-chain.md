# Supply chain

The build uses few dependencies. Tools and actions are pinned. Every dependency change is reviewed.

## How it works

- cargo-deny checks bans and sources on every PR (`security.yml`).
- Socket.dev reviews dependency changes for advisories, licenses and supply-chain risk. The config is `socket.yml`.
- Dependabot opens update PRs (`.github/dependabot.yml`).
- CI actions are pinned by full commit SHA.
- CI tools (actionlint, gitleaks, lizard) are pinned by version and SHA-256 hash.

## How to use / run locally

- Run `cargo deny --manifest-path src-tauri/Cargo.toml check bans sources`.
- Install lint tools with `pip install --require-hashes -r scripts/requirements-lint.txt`.
- Give a reason for each new dependency in the PR.

## Limits

- Socket policy lives in the Socket dashboard, not in the repo.
- cargo-deny does not check advisories or licenses. Socket does.

## History

- 2026-10-03 — Repo bootstrap with cargo-deny and Dependabot — [#1](https://github.com/tcivie/eepview/pull/1)
- 2026-10-03 — Cut dependencies flagged by Socket — [#10](https://github.com/tcivie/eepview/pull/10)
- 2026-10-03 — Hash-pinned CI tools — [#15](https://github.com/tcivie/eepview/pull/15)
