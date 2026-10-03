# Roles

Status: shipped.

eepview has three roles. Today one person holds the first and the third.

## Roles

| Role | Who today | What the role does |
| --- | --- | --- |
| Maintainer | [@tcivie](https://github.com/tcivie) | Decides what merges. Reviews pull requests. Writes ADRs. Cuts releases. Triages issues. Owns the repository settings and the ruleset. |
| Contributor | Anyone | Opens issues. Sends pull requests. Follows [CONTRIBUTING.md](https://github.com/tcivie/eepview/blob/main/CONTRIBUTING.md). Signs off every commit (DCO). Adds tests and docs for the change. |
| Security contact | The maintainer, `gleb@tcivie.com` | Receives private reports. Runs the steps in [Vulnerability response](vulnerability-response.md). Publishes the advisory. Credits the reporter. |

## What each role may not do

- A contributor does not merge. The maintainer merges.
- The maintainer does not bypass the ruleset, use `--admin` or use `--no-verify`.
- Nobody discusses an unfixed vulnerability in public.

## Limits

- One person holds two roles. This is an open gap. See [Access continuity](access-continuity.md).
- The project wants a second maintainer. See [Governance](governance.md) for the path.

## History

- 2026-10-03 — Add the governance pages — see CHANGELOG.
