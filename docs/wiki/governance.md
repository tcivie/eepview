# Governance

Status: shipped.

eepview is run by one maintainer. This page says how decisions are made and how that can change.

## How it works

- The model is BDFL, a benevolent dictator for life. One maintainer, [@tcivie](https://github.com/tcivie), decides. There is no vote and no committee.
- Contributors work in the open. They propose a change in an issue or a pull request.
- The maintainer reads the proposal and replies. The maintainer may accept it, ask for changes, or decline it with a reason.
- A change that shapes the architecture needs an ADR. An ADR is an architecture decision record. It is a page in this wiki, named `adr-NNNN-<slug>.md`. See [Decisions](decisions.md).
- A change to the no-leak design, the IPC contract or a quality gate needs a wiki update in the same PR.
- Nobody pushes to `main`. Every change goes through a pull request and green required checks. See [CI and quality gates](ci-and-quality-gates.md).
- Everyone follows the [code of conduct](https://github.com/tcivie/eepview/blob/main/CODE_OF_CONDUCT.md).

## How a contributor becomes a maintainer

1. Send several good pull requests over time. They should pass review with few changes.
2. Show that you follow the [coding standards](coding-standards.md), the [testing policy](testing-policy.md) and the no-leak rules.
3. Ask the maintainer in an issue, or the maintainer asks you.
4. The maintainer decides. A new maintainer gets write access, then review rights. Admin rights are given only when the [access continuity](access-continuity.md) plan needs them.
5. A maintainer who is inactive for 12 months, or who asks to leave, loses access.

## Limits

- One person is a single point of failure. See [Access continuity](access-continuity.md) and [Roles](roles.md).
- The project has no second maintainer today, so no change gets a review from a second maintainer. The maintainer reviews the changes of others. Automated checks review the maintainer's own changes.

## History

- 2026-10-03 — Add the governance pages — see CHANGELOG.
