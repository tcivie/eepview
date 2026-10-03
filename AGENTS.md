# AGENTS.md

Working rules for any human or AI agent on this repo.

## Workflow

- Start one branch per unit of work from main.
- Commit small after each working change.
- Write Conventional Commits.
- Open a PR to main. Enable squash auto-merge: `gh pr merge --auto --squash`.
- Never use `--admin`. Never bypass the ruleset. Never use `--no-verify`.
- Keep the branch up to date with main.
- Before you finish, fix and resolve every review thread. Zero unresolved.
- Before you finish, make sure every required check is green.
- Own your PR until it is merged. Do not stop while it is open: update the branch when it is behind, fix failing checks, and fix and resolve every review thread. Spike PRs are the exception: they close with a comment that says where the findings live.

## Quality

- Use NO lint exclusions of any kind: `#[allow]`, `#[expect]`, `biome-ignore`, `@ts-ignore`, `@ts-expect-error`, `noqa`, `shellcheck disable`, config relaxations.
- Run clippy pedantic with `-D warnings`.
- Keep cognitive and cyclomatic complexity at 10 or less.
- Keep functions at 40 lines or less, 5 parameters or less, nesting 3 or less.
- Never hide tool output behind a pipe without `set -o pipefail`.

## Tests

- Write unit tests for every pure module.
- Coverage only goes up. The ratchet is in `ci.yml`.
- Add no coverage exclusions.
- Tests check the requirement, not the code. The agent that writes the tests is not the agent that writes the code. It reads the requirement and the public interface only, never the implementation. Each test names its requirement. Never edit a test to match the code. See `docs/wiki/testing-policy.md`.

## Security

- Read `docs/wiki/no-leak-architecture.md` before you touch networking or webviews. ADR 0001 lands with the browser shell PR, as `docs/wiki/adr-0001-no-leak-architecture.md`.
- The five layers are mandatory.
- Keep the architecture test and the leak test green.
- Add no new HTTP client crates.
- Keep dependencies minimal. Give a reason for each new one in the PR. It must pass Socket.
- Pin actions by full SHA.
- Set `permissions: {}` and add the minimum per job.
- Set `persist-credentials: false` on checkout.
- Add no clearnet code path without explicit user consent.

## Documentation

- All docs live in the wiki. The source is docs/wiki/ in this repo; edit it there in a PR, and CI publishes it. ADRs are docs/wiki/adr-NNNN-<slug>.md. The interface contract is docs/wiki/ipc-contract.md.
- Do not add Markdown files outside docs/wiki/. The allowed root files are listed in scripts/docs-check.sh.
- A PR that adds or changes a user-visible feature or a quality gate must update its `docs/wiki` page. Create the page if it is new.
- Edit `docs/wiki/` in the repo. The GitHub wiki is generated from it.
- Add a History line with the PR link.
- Update the status in `docs/wiki/README.md`.
- Do not edit CHANGELOG.md in a pull request. The release job generates it from the commit titles, so write a clear Conventional Commit PR title.
- Get the number with `gh pr view --json number` after you open the PR. Then push the docs commit.
- CI (`docs-check`) fails a `feat` PR that does not change a page under `docs/wiki/`, and any PR that edits `CHANGELOG.md`.

## Writing style

- Write short sentences.
- Use active voice.
- Use simple words.

## Privacy

- Never reference the owner's other projects, companies or people.
