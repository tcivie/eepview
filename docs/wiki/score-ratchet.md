# Score ratchet

CI stops a pull request that lowers a file-based OpenSSF Scorecard check. A score can stay the same or go up. It can never go down. This works like the coverage ratchet in `heavy.yml`.

## What is tracked

The floors live in `.github/score-floors.json`, under `scorecard_pr`. The Scorecard CLI measures them on a checkout. The CLI cannot see the GitHub API or the history, so `License` scores 9 and `SAST` scores 10 locally. A score of -1 means "not applicable".

## The job

The `score-ratchet (pr)` job is in `.github/workflows/ci.yml`, the fast lane. It runs on each pull request. It runs the Scorecard CLI on the PR checkout for Binary-Artifacts, Dangerous-Workflow, Pinned-Dependencies, Token-Permissions, SAST, Security-Policy, License, Fuzzing, Dependency-Update-Tool and Packaging. It fails when one check scores below its floor.

It is the only PR check that stops these changes:

- a Docker `FROM` with no digest
- a `pip install` with no `--require-hashes`
- a committed binary

`scripts/score-ratchet.sh pr` runs the check. It needs Docker and `jq`. `scripts/score-ratchet-test.sh` tests it with a fake `docker`. The job runs that test first.

The CLI runs from the official image `ghcr.io/ossf/scorecard`, pinned by digest. The `gcr.io/openssf/scorecard` registry refuses anonymous pulls at the time of writing, and `ghcr.io` carries the same release.

The published Scorecard result is not ratcheted. `.github/workflows/scorecard.yml` still publishes it and uploads the SARIF to code scanning.

## Raise a floor

When a score goes up, the script prints "raise the floor" lines and the full floors JSON to paste. Do this:

1. Run `scripts/score-ratchet.sh pr`.
2. Copy the printed JSON (it follows the line "Paste this into") into `.github/score-floors.json`.
3. Commit it in a PR. Only raise floors. Never lower one.

## When a score drops

1. Open the failed PR check. Read the table: name, floor, now.
2. Find the change in the PR that caused it, and fix the PR.
3. Do not lower a floor to turn the check green. A floor may go down only when the Scorecard project changes how a check scores. Say so in the PR and get the owner to approve it.

The `gate` job of `ci.yml` needs `score-ratchet (pr)`. A pull request that lowers a file-based score fails the gate, and the gate is a required check.

## History

- Added in [#40](https://github.com/tcivie/eepview/pull/40).
- The `score-ratchet (pr)` job moved into `ci.yml` in [#89](https://github.com/tcivie/eepview/pull/89).
- The `score-ratchet (published)` job, its daily run and its "Score dropped" issue were removed in [#109](https://github.com/tcivie/eepview/pull/109). The PR job stays.
