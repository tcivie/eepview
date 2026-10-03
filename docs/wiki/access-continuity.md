# Access continuity

Status: shipped. One gap is open.

This page says what happens if the maintainer cannot act. Today there is one maintainer. We say so plainly.

## What exists today

- **Successor setting.** GitHub lets a personal account name a successor. A successor can take over the repositories of the account if the owner can no longer act. See GitHub: [Maintaining ownership continuity of your personal account's repositories](https://docs.github.com/en/account-and-profile/how-tos/account-management/maintaining-ownership-continuity-of-your-personal-accounts-repositories). The maintainer will name a successor. No successor is named yet.
- **Recovery material.** The account recovery codes and the release signing setup are kept in a password manager. The password manager has an emergency-access contact.
- **No private release key.** Releases carry no private key that one person holds. The release workflow attests build provenance with `actions/attest-build-provenance`. That action signs keyless with Sigstore, using the GitHub Actions OIDC identity. A successor with admin rights on the repository can release with no key handover. See [Release pipeline](release-pipeline.md). The attestation step runs only while the repository is public.
- **Everything is in the repository.** The code, the docs, the CI and the release steps are in git. A new maintainer can rebuild all of it.

## What is still needed

- A second person with admin rights on the repository.
- A named successor in the GitHub successor settings.
- A second active maintainer who can review and release.

## Open gap

This is open. There is no second active maintainer. If the maintainer is unavailable, nobody else can merge, release or answer a security report until the successor process finishes. These OpenSSF Gold criteria stay unmet until a second maintainer joins: `bus_factor`, `contributors_unassociated` and `two_person_review`.

The project wants a co-maintainer. See the [roadmap](roadmap.md) and [Governance](governance.md).

## History

- 2026-10-03 — Add the governance pages — [#30](https://github.com/tcivie/eepview/pull/30).
