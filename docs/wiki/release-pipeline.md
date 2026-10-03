# Release pipeline

Status: shipped.

A `v*` tag builds installers for four targets, adds SBOMs, debug symbols and checksums, signs every file with Sigstore, and makes a draft GitHub release.

## How it works

- A push of a tag that matches `v*` starts `release.yml`. A manual run (`workflow_dispatch`) does the same, with the `dry_run` input.
- The `gates` job runs `scripts/release-gates.sh`. It reads the required checks from the active ruleset of the default branch. It fails when the tagged commit is not on `main`, or when a required check has no `success` run on that commit or on the head of the pull request that merged it. Some required checks, such as `docs-check`, run on pull requests only. The `build` job needs `gates`. The release no longer calls `lint.yml`, `security.yml` or `ci.yml`.
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
- Last, `gh release create --draft --generate-notes` uploads the files and the `.sigstore.json` bundles. A person reads the draft and publishes it.

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
2. Real release. Bump the version, merge it, then push a signed tag from `main`. See [Release and support](release-and-support.md#signed-tags).
   ```sh
   git tag -s v0.1.0 -m "eepview v0.1.0" && git push origin v0.1.0
   ```
3. Open the draft release, check the files and `SHA256SUMS`, then publish it.

## Limits

- `publish` attests and releases only for a `refs/tags/v*` ref that is not a dry run. A manual run on a branch never makes a release. It still signs the files, so each dry run adds entries to the public Rekor log.
- A dry run signs with the identity of its own ref (`refs/heads/...`). Those signatures do not match the `refs/tags/v` pattern above, by design.
- The macOS release ships only the `.dmg`. `bundle.macOS.signingIdentity` is `-` in `src-tauri/tauri.conf.json`, so the Tauri bundler ad-hoc signs the `.app` before it builds the `.dmg`. A build step mounts the `.dmg` and runs `codesign --verify --deep --strict` and `codesign -dv` on the app. It fails unless the report says `Signature=adhoc`. There is no Developer ID signature and no notarization.
- Windows installers are not signed.
- The `gates` job skips its check on a branch dry run, so a pipeline change can be tested before merge. It needs a green `main` at the tagged commit. A dry run on `main` fails while the checks of `main` HEAD are red or still running.
- The pipeline has no lint exclusion. actionlint and `zizmor --offline --persona=pedantic` report nothing.

## History

- [#14](https://github.com/tcivie/eepview/pull/14): release pipeline with SBOM, checksums and provenance.
- [#24](https://github.com/tcivie/eepview/pull/24): release gates job without lint exclusions; the macOS app is signed inside the dmg.
- [#37](https://github.com/tcivie/eepview/pull/37): Sigstore signatures for every release file, provenance without the private-repo guard, separate debug symbols, repeatable build environment.
- [#39](https://github.com/tcivie/eepview/pull/39): the gate also reads the checks of the merged PR head.
- [#47](https://github.com/tcivie/eepview/pull/47): permission comments in `release.yml`; zizmor runs with the pedantic persona.
