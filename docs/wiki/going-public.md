# Going public checklist

Status on 2026-10-03:

| Step | Status |
|---|---|
| Secret scanning | Done 2026-10-03 (PO) |
| Push protection | Done 2026-10-03 (PO) |
| Private vulnerability reporting | Done 2026-10-03 (PO) |
| CodeQL | Done 2026-10-03 ([#26](https://github.com/tcivie/eepview/pull/26)) |
| Scorecard | Done 2026-10-03 ([#26](https://github.com/tcivie/eepview/pull/26)) |
| Dependency review | Done 2026-10-03 ([#26](https://github.com/tcivie/eepview/pull/26)) |
| README badges | Done 2026-10-03 ([#26](https://github.com/tcivie/eepview/pull/26)). Best Practices badge is not added. |
| `protect-main` required checks | Not done |
| Best Practices registration | Not done. Owner action at bestpractices.dev. |
| Release provenance check | Not done. Needs a release. |

Do these steps on the day the repo goes public.

## 1. Restore the workflows

CodeQL and Scorecard need a public repo on the free plan. They were removed so no check shows as skipped.

The squash commit of the PR that removed them is `3c3f92a`. Run:

```sh
git show 3c3f92a^:.github/workflows/codeql.yml > .github/workflows/codeql.yml
git show 3c3f92a^:.github/workflows/scorecard.yml > .github/workflows/scorecard.yml
```

Then remove the `if: ${{ !github.event.repository.private }}` lines. They are no longer needed.

## 2. Update the `protect-main` ruleset

Add these checks to the required checks:

- `codeql (actions)`
- `codeql (javascript-typescript)`
- `codeql (rust)`
- `dependency-review`

Do not add Scorecard. `scorecard.yml` has no `pull_request` trigger, so no PR reports it. A required Scorecard check would block every PR.

## 3. Turn on GitHub security features

Enable secret scanning and push protection:

```sh
gh api -X PATCH repos/tcivie/eepview \
  -F 'security_and_analysis[secret_scanning][status]=enabled' \
  -F 'security_and_analysis[secret_scanning_push_protection][status]=enabled'
```

Enable private vulnerability reporting:

```sh
gh api -X PUT repos/tcivie/eepview/private-vulnerability-reporting
```

## 4. Add dependency review

Add a workflow that uses `actions/dependency-review-action`, pinned by commit hash. Set `fail-on-severity: high`.

## 5. Register at bestpractices.dev

The owner registers the project at bestpractices.dev.

## 6. Add the README badges

OpenSSF Scorecard:

```md
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/tcivie/eepview/badge)](https://scorecard.dev/viewer/?uri=github.com/tcivie/eepview)
```

deps.rs:

```md
[![deps.rs](https://deps.rs/repo/github/tcivie/eepview/status.svg?path=src-tauri)](https://deps.rs/repo/github/tcivie/eepview?path=src-tauri)
```

OpenSSF Best Practices: add the badge that bestpractices.dev gives after step 5.

## 7. Check SECURITY.md

Open the advisory link in `SECURITY.md`. Make sure it works.

## 5. Check release provenance

The `publish` job in `release.yml` attests build provenance only when the repo is public. After the switch, run a release and check that the attest step ran. Then run `gh attestation verify <file> --repo tcivie/eepview` on one bundle.
