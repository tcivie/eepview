<!-- SPDX-License-Identifier: MIT -->

# Release and support

Status: in progress.

How the maintainer cuts a release. The build itself is in [Release pipeline](release-pipeline.md).

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

## History

- [#37](https://github.com/tcivie/eepview/pull/37): signed tags documented.
