# Documentation

The wiki source lives in the repo. A workflow publishes it to the GitHub wiki.

## How it works

- Edit `docs/wiki/` in the repo. Review it in a PR like code.
- On a push to main that changes `docs/wiki/**`, `.github/workflows/wiki.yml` runs `scripts/publish-wiki.sh`.
- The script copies the pages to the wiki repo. `README.md` becomes `Home.md`. Relative links are rewritten.
- The script adds a `_Footer.md` that says the wiki is generated. It commits as github-actions[bot] only if something changed.
- `docs-check` (`scripts/docs-check.sh`) fails a PR whose title starts with `feat` and that changes no page under `docs/wiki/`. Other types, such as `fix`, `ci`, `chore` and `test`, pass without a wiki change. It also fails a PR that edits `CHANGELOG.md` (see [Release pipeline](release-pipeline.md#changelog-and-release-notes)), or that leaves a wiki page out of the index. The PR title comes from the event payload (`GITHUB_EVENT_PATH`). Run it locally with `BASE_REF=main PR_TITLE='feat: x' ./scripts/docs-check.sh`; `scripts/docs-check.test.sh` tests it. The workflow is `.github/workflows/docs-check.yml`.

## How to use / run locally

- Add or change a page in `docs/wiki/`. Link it from `docs/wiki/README.md`.
- Run `BASE_REF=main PR_TITLE='<your PR title>' ./scripts/docs-check.sh`.
- Run the workflow by hand: `gh workflow run wiki.yml`.

## Limits

- Never edit the GitHub wiki by hand. The next run overwrites it.
- A page removed from `docs/wiki/` is not removed from the wiki.

## History

- 2026-10-03 — Add the wiki, AGENTS.md, docs-check and wiki publishing — [#16](https://github.com/tcivie/eepview/pull/16)
- 2026-10-03 — docs-check: a feat PR needs a wiki change, and CHANGELOG.md is generated, not edited — [#50](https://github.com/tcivie/eepview/pull/50)
