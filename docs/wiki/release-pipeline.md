# Release pipeline

Status: shipped.

A `v*` tag on a release branch builds installers for four targets, adds SBOMs, debug symbols and checksums, signs every file with Sigstore, and makes a draft GitHub release.

## How it works

- A release comes from a release branch. See [Release branches](#release-branches).
- A push of a tag that matches `v*` starts `release.yml`. A manual run (`workflow_dispatch`) does the same, with the `dry_run` input.
- The `gates` job runs `scripts/release-gates.sh`. It fails when the tagged commit is not on a `release/*` branch. It reads the checks that the ruleset of the release branches requires: `gate` and `heavy-gate`. It fails when one of them has no `success` run on the tagged commit or on the head of the pull request that merged it. These checks run on pull requests, so a squash commit has none of its own. That is why the script reads the head of the pull request too. The `build` job needs `gates`. The release does not call `ci.yml` or `heavy.yml`.
- The `build` job has four legs. Each uses the Tauri CLI with no third-party release action:
  1. `scripts/repro-env.sh` sets `SOURCE_DATE_EPOCH`, `CARGO_INCREMENTAL=0` and `--remap-path-prefix`. See [Reproducible builds](reproducible-builds.md).
  2. `npx tauri build --no-bundle -- --locked` builds the release binary.
  3. `scripts/split-debug.sh` moves the debug symbols into a separate file (see below).
  4. On Linux, `scripts/check-hardening.sh` checks PIE, full RELRO and NX.
  5. `npx tauri bundle` builds the installers from the stripped binary.
  - `x86_64-pc-windows-msvc` on `windows-2025`: NSIS installer.
  - `aarch64-apple-darwin` on `macos-15`: `.dmg`.
  - `x86_64-apple-darwin` on `macos-15`: `.dmg`.
  - `x86_64-unknown-linux-gnu` on `ubuntu-24.04`: AppImage and `.deb`.
- The `sbom` job writes two CycloneDX files. One is for the Rust crates (`cargo cyclonedx`). One is for the npm production dependencies (`npm sbom`).
- The `publish` job flattens all artifacts into one folder and writes `SHA256SUMS` for every file.
- The same job signs every file, `SHA256SUMS` included, with Sigstore `cosign sign-blob`. The signature is keyless: the job's OIDC token gets a short-lived certificate from Fulcio, and the Rekor transparency log records each signature. Each file gets a `<file>.sigstore.json` bundle. The job then runs `cosign verify-blob` on every bundle against the identity of this workflow run, and fails on a bad signature.
- The same job attests build provenance with `actions/attest-build-provenance`.
- The same job writes the release notes with git-cliff (see [Changelog and release notes](#changelog-and-release-notes)).
- Last, `gh release create --draft --notes-file` uploads the files and the `.sigstore.json` bundles. A person reads the draft and publishes it.

## Release branches

- Cut a release branch from main: `git push origin <sha>:refs/heads/release/v0.1`.
- A ruleset on `refs/heads/release/**` blocks deletion and force pushes. It requires a pull request (squash). It requires the checks `gate` (the fast lane) and `heavy-gate` (the heavy lane). See [CI and quality gates](ci-and-quality-gates.md).
- A change reaches a release branch only through a pull request. That pull request runs both lanes.
- No workflow runs on a push to a release branch. The zizmor cache-poisoning audit flags a cache in a workflow that runs on such a push, and `release.yml` builds without caches.
- To put `heavy-gate` on the cut commit itself, run `heavy.yml` on the release branch by hand: `gh workflow run heavy.yml --ref release/v0.1`.

## Changelog and release notes

- Nobody edits `CHANGELOG.md` in a PR. This stops the merge conflicts that a shared line caused. `docs-check` fails a PR that changes it.
- git-cliff builds the changelog and the release notes from the Conventional Commit titles on `main`. `cliff.toml` groups `feat` under Added, `fix` under Fixed, `security` under Security, `perf` and `refactor` under Changed, and `docs`, `ci`, `test` and `chore` under their own sections. It links each `(#N)` to its pull request and skips merge commits.
- The squash-merge title is the commit title. Write a clear Conventional Commit PR title.
- `scripts/release-notes.sh` prints the notes for one tag: the commits since the previous `v*` tag. The `publish` job installs a pinned git-cliff with `scripts/install-git-cliff.sh`, which checks the SHA-256 of the download. The notes become the draft release body.
- The release job writes the release notes. It does not commit `CHANGELOG.md`, because `main` is protected. `CHANGELOG.md` is a snapshot of the generated history. A maintainer refreshes it in a PR with `scripts/changelog.sh --write origin/main`. It needs git-cliff (`brew install git-cliff`).
- `docs-check` accepts a `CHANGELOG.md` change only when the file equals the output of `scripts/changelog.sh` for the base branch tip, or for the commit where the PR branched off. A merge to `main` after that does not turn the PR red.

## Release files

| File | What it is |
| --- | --- |
| `eepview_<version>_x64-setup.exe` | Windows NSIS installer |
| `eepview_<version>_aarch64.dmg`, `eepview_<version>_x64.dmg` | macOS disk images |
| `eepview_<version>_amd64.AppImage`, `eepview_<version>_amd64.deb` | Linux packages |
| `eepview-<target>.debug` | Linux debug symbols (DWARF), split with `objcopy --only-keep-debug` |
| `eepview-<target>.dSYM.zip` | macOS debug symbols (`split-debuginfo = "packed"`) |
| `eepview-<target>.pdb` | Windows debug symbols |
| `eepview.cdx.json`, `npm.cdx.json` | CycloneDX SBOMs for the Rust crates and the npm packages |
| `SHA256SUMS` | SHA-256 of every file above |
| `<file>.sigstore.json` | Sigstore bundle (signature, certificate, Rekor entry) for each file |

The debug symbols hold function names and line tables (`debug = "limited"`). The shipped binaries are stripped. To read a Linux crash backtrace, put `eepview-x86_64-unknown-linux-gnu.debug` next to the binary; the binary has a `.gnu_debuglink` to it. On macOS, unzip the `.dSYM` next to the app, or pass it to `atos -o`.

## Verify a download

Check the Sigstore signature. The certificate must name `release.yml` on a `v*` tag of this repo:

```sh
cosign verify-blob \
  --bundle eepview_0.1.0_amd64.deb.sigstore.json \
  --certificate-identity-regexp '^https://github.com/tcivie/eepview/.github/workflows/release.yml@refs/tags/v' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  eepview_0.1.0_amd64.deb
```

Check the build provenance with the GitHub CLI:

```sh
gh attestation verify eepview_0.1.0_amd64.deb --repo tcivie/eepview
```

Check the hash. Verify `SHA256SUMS` itself first, with the two commands above, then:

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

## How to run

1. Dry run. It builds, signs and verifies everything, but skips the attestation and the release:
   ```sh
   gh workflow run release.yml -f dry_run=true --ref main
   ```
2. Real release. Cut a release branch from main. Bump the version before the cut, or in a pull request into the release branch. Push a signed tag on a commit of the release branch. The checks `gate` and `heavy-gate` must be green on that commit or on the head of the pull request that merged it. To tag the cut commit itself, run `heavy.yml` on the branch by hand first. See [Release and support](release-and-support.md#signed-tags).
   ```sh
   git push origin <sha>:refs/heads/release/v0.1
   git tag -s v0.1.0 -m "eepview v0.1.0" && git push origin v0.1.0
   ```
3. Open the draft release, check the files and `SHA256SUMS`, then publish it.

## Limits

- `publish` attests and releases only for a `refs/tags/v*` ref that is not a dry run. A manual run on a branch never makes a release. It still signs the files, so each dry run adds entries to the public Rekor log.
- A dry run signs with the identity of its own ref (`refs/heads/...`). Those signatures do not match the `refs/tags/v` pattern above, by design.
- The macOS release ships only the `.dmg`. `bundle.macOS.signingIdentity` is `-` in `src-tauri/tauri.conf.json`, so the Tauri bundler ad-hoc signs the `.app` before it builds the `.dmg`. A build step mounts the `.dmg` and runs `codesign --verify --deep --strict` and `codesign -dv` on the app. It fails unless the report says `Signature=adhoc`. It also fails unless the app's `Info.plist` sets `NSAppTransportSecurity` > `NSAllowsArbitraryLoadsInWebContent` to true; without it no `http://` eepsite loads ([Browser shell](browser-shell.md), B1). There is no Developer ID signature and no notarization.
- Windows installers are not signed.
- The `gates` job skips its check on a dry run on any branch except a `release/*` branch, so you can test a pipeline change before merge. A dry run on a release branch runs the check. It fails while `gate` or `heavy-gate` is missing, red or still running on the head of that branch.
- The pipeline has no lint exclusion. actionlint and `zizmor --offline --persona=pedantic` report nothing.

## History

- [#14](https://github.com/tcivie/eepview/pull/14): release pipeline with SBOM, checksums and provenance.
- [#24](https://github.com/tcivie/eepview/pull/24): release gates job without lint exclusions; the macOS app is signed inside the dmg.
- [#37](https://github.com/tcivie/eepview/pull/37): Sigstore signatures for every release file, provenance without the private-repo guard, separate debug symbols, repeatable build environment.
- [#39](https://github.com/tcivie/eepview/pull/39): the gate also reads the checks of the merged PR head.
- [#47](https://github.com/tcivie/eepview/pull/47): permission comments in `release.yml`; zizmor runs with the pedantic persona.
- [#50](https://github.com/tcivie/eepview/pull/50): generated changelog and release notes from commit titles with git-cliff.
- [#85](https://github.com/tcivie/eepview/pull/85): the dmg check also requires the App Transport Security key for web content in the app.
- [#89](https://github.com/tcivie/eepview/pull/89): releases come from `release/*` branches, and `gates` reads the checks `gate` and `heavy-gate` of the release-branch ruleset.
