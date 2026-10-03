<!-- SPDX-License-Identifier: MIT -->

# Release and support

Status: in progress. eepview has no release yet.

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
- Each release file has a keyless Sigstore signature, made through GitHub Actions OIDC. The workflow also attests build provenance. The commands to check both are in [Release pipeline](release-pipeline.md#verify-a-download). The attestation step runs only while the repository is public.
- A person reads the draft release and publishes it.

## Signed tags

The maintainer signs each release tag with an SSH signing key. This is a practice, not a gate: `release.yml` does not check the tag signature. The release files carry their own Sigstore signatures, see [Release pipeline](release-pipeline.md#verify-a-download).

One-time setup:

```sh
git config --global gpg.format ssh
git config --global user.signingkey ~/.ssh/id_ed25519.pub
```

Add the same public key on GitHub as a **signing key** (Settings, SSH and GPG keys). GitHub then marks the tag as Verified.

Cut the release from `main`:

```sh
git switch main && git pull --ff-only
git tag -s v0.1.0 -m "eepview v0.1.0"
git push origin v0.1.0
```

Check a tag locally. `git tag -v` needs a file that lists the keys you trust:

```sh
echo "$(git config user.email) $(cat ~/.ssh/id_ed25519.pub)" > ~/.config/git/allowed_signers
git config --global gpg.ssh.allowedSignersFile ~/.config/git/allowed_signers
git tag -v v0.1.0
```

## Limits

- Windows installers and the macOS `.dmg` have no platform code-signing certificate. The Sigstore signature does not replace one.
- There is no in-app update. The user installs the new version by hand.

## History

- 2026-10-03 — Add the release and support policy — [#30](https://github.com/tcivie/eepview/pull/30).
- [#37](https://github.com/tcivie/eepview/pull/37): signed tags documented.
