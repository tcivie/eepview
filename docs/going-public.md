# Going public checklist

Do these steps on the day the repo goes public.

## 1. Restore the workflows

CodeQL and Scorecard need a public repo on the free plan. They were removed so no check shows as skipped.

Replace `<this-commit>` with the squash commit of the PR that removed them. Then run:

```sh
git show <this-commit>^:.github/workflows/codeql.yml > .github/workflows/codeql.yml
git show <this-commit>^:.github/workflows/scorecard.yml > .github/workflows/scorecard.yml
```

Then remove the `if: ${{ !github.event.repository.private }}` lines. They are no longer needed.

## 2. Update the `protect-main` ruleset

Add these checks to the required checks:

- The CodeQL analyze checks.
- Scorecard.

## 3. Add the README badges

OpenSSF Scorecard:

```md
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/tcivie/eepview/badge)](https://scorecard.dev/viewer/?uri=github.com/tcivie/eepview)
```

deps.rs:

```md
[![deps.rs](https://deps.rs/repo/github/tcivie/eepview/status.svg?path=src-tauri)](https://deps.rs/repo/github/tcivie/eepview?path=src-tauri)
```

OpenSSF Best Practices: the owner registers the project at bestpractices.dev. Then add the badge that the site gives.

## 4. Check SECURITY.md

Open the advisory link in `SECURITY.md`. Make sure it works.
