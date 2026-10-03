# Documentation

The wiki source lives in the repo. A workflow publishes it to the GitHub wiki.

## How it works

- Edit `docs/wiki/` in the repo. Review it in a PR like code.
- On a push to main that changes `docs/wiki/**`, `.github/workflows/wiki.yml` runs `scripts/publish-wiki.sh`.
- The script copies the pages to the wiki repo. `README.md` becomes `Home.md`. Relative links are rewritten.
- The script adds a `_Footer.md` that says the wiki is generated. It commits as github-actions[bot] only if something changed.
- `docs-check` (`scripts/docs-check.sh`) fails a PR that changes code without a docs or changelog update, or that leaves a wiki page out of the index.

## How to use / run locally

- Add or change a page in `docs/wiki/`. Link it from `docs/wiki/README.md`.
- Run `BASE_REF=main ./scripts/docs-check.sh`.
- Run the workflow by hand: `gh workflow run wiki.yml`.

## Limits

- Never edit the GitHub wiki by hand. The next run overwrites it.
- A page removed from `docs/wiki/` is not removed from the wiki.

## History

- 2026-10-03 — Add the wiki, AGENTS.md, docs-check and wiki publishing — [#16](https://github.com/tcivie/eepview/pull/16)
