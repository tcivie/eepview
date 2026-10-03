# Release and support

Status: shipped. eepview has no release yet.

## Versioning

- eepview follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a minor version may break things. Each change is in `CHANGELOG.md`.
- A release is a git tag `vX.Y.Z` on `main`.

## Support

- Only the latest release is supported. It gets bug fixes and security fixes.
- An older release gets no fix. Upgrade to the latest release.
- Before the first release, nothing is supported. See [SECURITY.md](https://github.com/tcivie/eepview/blob/main/SECURITY.md).

## Upgrade path

1. Download the latest release for your system.
2. Install it over the old version. Do not uninstall first.
3. Your data stays. The bookmarks, history and settings live in the app config directory, not in the install folder.
4. The stores have a `version` field. It is there for future migrations.

## How releases are verified

- A tag starts `release.yml`. The lint, security and test jobs must pass first. See [Release pipeline](release-pipeline.md).
- The release holds installers for four targets, two CycloneDX SBOMs and a `SHA256SUMS` file.
- A user checks a download with `sha256sum -c SHA256SUMS`.
- The workflow attests build provenance with Sigstore, keyless, through GitHub Actions OIDC. A user checks it with `gh attestation verify <file> --repo tcivie/eepview`. The step runs only while the repository is public.
- A person reads the draft release and publishes it.

## Limits

- Windows installers and the macOS `.dmg` are not signed with a platform certificate.
- There is no in-app update. The user installs the new version by hand.

## History

- 2026-10-03 — Add the release and support policy — see CHANGELOG.
