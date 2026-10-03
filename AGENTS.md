# AGENTS.md

Working rules for any human or AI agent on this repo.

## Workflow

- Start one branch per unit of work from main.
- Commit small after each working change.
- Write Conventional Commits.
- Open a PR to main. Turn on squash auto-merge as soon as the PR is ready: `gh pr merge <n> --squash --auto`. Spike PRs are the exception: never turn on auto-merge for them.
- Never use `--admin`. Never bypass the ruleset. Never use `--no-verify`.
- A PR does not need to be up to date with main. Merge main in only when the PR has a conflict (DIRTY).
- Resolve review threads while CI runs, not after. Fix each thread, then resolve it. Resolve after an answer only when the reviewer agrees or the thread is a question. Never resolve a thread with a reply to avoid a fix. Auto-merge does not fire while a thread is open. Zero unresolved.
- When any check turns red, fix it at once. Do not wait for the other checks to finish. Fix a red test in the code, or ask the test agent. Never edit the test to match the code. A push cancels the other running jobs, so they restart after it.
- If a check is red on main too, the cause is on main. Look for an open issue about it first and comment there; open a new issue only if none exists. Do not change the gate in your PR. Keep watching: when main turns green, re-run the failed check on your PR.
- You own your PR until it is MERGED. Do not stop while it is open: fix failing checks and resolve every review thread. Spike PRs are the exception: they close with a comment that says where the findings live.
- After your PR merges, remove your worktree (`git worktree remove <path>`) and any CARGO_TARGET_DIR or scratch folder you created. Leave nothing behind.

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
- A PR that changes the UI runs scripts/screenshots.sh and commits the updated images.
- A UI change uses the shared components in src/ui/ui.css (docs/wiki/ui-components.md). scripts/style-check.sh fails on a raw spacing, radius or font size and on an inline style.
- CI (`docs-check`) fails a `feat` PR that does not change a page under `docs/wiki/`, and any PR that edits `CHANGELOG.md`.

## Writing style

- Write short sentences.
- Use active voice.
- Use simple words.

## Privacy

- Never reference the owner's other projects, companies or people.
