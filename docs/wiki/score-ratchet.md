# Score ratchet

CI stops any change that lowers our external scores. A score can stay the same or go up. It can never go down. This works like the coverage ratchet in `heavy.yml`.

## What is tracked

The floors live in `.github/score-floors.json`.

- `scorecard`: the overall OpenSSF Scorecard score and the score of every check, from the published result at `api.scorecard.dev`. A score of -1 means "not applicable".
- `scorecard_pr`: the floors for the file-based checks, measured with the Scorecard CLI on a checkout. They differ from the published floors. The CLI cannot see the GitHub API or the history, so `License` scores 9 and `SAST` scores 10 locally.
- `bestpractices`: the three badge percentages from `bestpractices.dev/projects/15180.json`.

## The two jobs

The `score-ratchet (pr)` job is in `.github/workflows/ci.yml`, the fast lane. It moved there from `score-ratchet.yml`. The `score-ratchet (published)` job is in `.github/workflows/score-ratchet.yml`, which keeps only that job.

`scripts/score-ratchet.sh` runs both. `scripts/score-ratchet-test.sh` tests it with a fake `docker`; the `pr` job runs that test first. It needs `curl` and `jq`. The `pr` mode also needs Docker.

| Check | When it runs | What it does |
| --- | --- | --- |
| `score-ratchet (pr)` | Each pull request (`ci.yml`) | Runs the Scorecard CLI on the PR checkout for Binary-Artifacts, Dangerous-Workflow, Pinned-Dependencies, Token-Permissions, SAST, Security-Policy, License, Fuzzing, Dependency-Update-Tool and Packaging. Fails when one scores below its `scorecard_pr` floor. It stops an unpinned action or a broad token permission. |
| `score-ratchet (published)` | Push to main, every day, and by hand (`score-ratchet.yml`) | Compares the published Scorecard and Best Practices scores with the floors. Fails when one is lower. Opens or updates one issue titled "Score dropped" with a table. |

The CLI runs from the official image `ghcr.io/ossf/scorecard`, pinned by digest. The `gcr.io/openssf/scorecard` registry refuses anonymous pulls at the time of writing, and `ghcr.io` carries the same release.

## Raise a floor

When a score goes up, the script prints "raise the floor" lines and the full floors JSON to paste. The `published` mode raises `scorecard` and `bestpractices`. The `pr` mode raises `scorecard_pr`. Do this:

1. Run `scripts/score-ratchet.sh published` or `scripts/score-ratchet.sh pr`.
2. Copy the printed JSON (it follows the line "Paste this into") into `.github/score-floors.json`.
3. Commit it in a PR. Only raise floors. Never lower one.

## When a score drops

1. Open the "Score dropped" issue or the failed PR check. Read the table: name, floor, now.
2. Find the change that caused it. For a PR, fix the PR.
3. For the published job, look at the Scorecard run and the Best Practices answers. Fix the cause.
4. Do not lower a floor to turn the check green. A floor may go down only when the Scorecard project changes how a check scores. Say so in the PR and get the owner to approve it.
5. Close the issue when the job passes again.

The `gate` job of `ci.yml` needs `score-ratchet (pr)`. A pull request that lowers a file-based score fails the gate, and the gate is a required check. The `published` job is not a required check.

## History

- Added in [#40](https://github.com/tcivie/eepview/pull/40).
- The `score-ratchet (pr)` job moved into `ci.yml` in [#89](https://github.com/tcivie/eepview/pull/89).
