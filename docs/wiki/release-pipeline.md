# Release pipeline

Status: shipped.

A `v*` tag builds installers for four targets, adds SBOMs and checksums, and makes a draft GitHub release.

## How it works

- A push of a tag that matches `v*` starts `release.yml`. A manual run (`workflow_dispatch`) does the same, with the `dry_run` input.
- The `gates` job runs `scripts/release-gates.sh`. It reads the required checks from the active ruleset of the default branch. It fails when the tagged commit is not on `main`, or when a required check has no `success` run on that commit. The `build` job needs `gates`. The release no longer calls `lint.yml`, `security.yml` or `ci.yml`.
- The `build` job has four legs. Each runs `npx tauri build` with no third-party release action.
  - `x86_64-pc-windows-msvc` on `windows-2025`: NSIS installer.
  - `aarch64-apple-darwin` on `macos-15`: `.dmg`.
  - `x86_64-apple-darwin` on `macos-15`: `.dmg`.
  - `x86_64-unknown-linux-gnu` on `ubuntu-24.04`: AppImage and `.deb`.
- The `sbom` job writes two CycloneDX files. One is for the Rust crates (`cargo cyclonedx`). One is for the npm production dependencies (`npm sbom`).
- The `publish` job flattens all artifacts into one folder and writes `SHA256SUMS` for every file.
- The same job attests build provenance with `actions/attest-build-provenance`. This step runs only when the repo is public, because private repos need GitHub Enterprise for attestations.
- Last, `gh release create --draft --generate-notes` uploads the files. A person reads the draft and publishes it.

## How to run

1. Dry run. It builds everything and skips `publish`:
   ```sh
   gh workflow run release.yml -f dry_run=true --ref main
   ```
2. Real release. Bump the version, merge it, then push a tag from `main`:
   ```sh
   git tag v0.1.0 && git push origin v0.1.0
   ```
3. Open the draft release, check the files and `SHA256SUMS`, then publish it.

## Limits

- `publish` runs only for a `refs/tags/v*` ref. A manual run on a branch never makes a release.
- The macOS release ships only the `.dmg`. `bundle.macOS.signingIdentity` is `-` in `src-tauri/tauri.conf.json`, so the Tauri bundler ad-hoc signs the `.app` before it builds the `.dmg`. A build step mounts the `.dmg` and runs `codesign --verify --deep --strict` and `codesign -dv` on the app. It fails unless the report says `Signature=adhoc`. There is no Developer ID signature and no notarization.
- Windows installers are not signed.
- Provenance is skipped while the repo is private. See [going-public.md](going-public.md).
- The `gates` job needs a green `main` at the tagged commit. A dry run on `main` fails while the checks of `main` HEAD are red or still running.
- The pipeline has no lint exclusion. actionlint and `zizmor --offline` report nothing.

## History

- [#14](https://github.com/tcivie/eepview/pull/14): release pipeline with SBOM, checksums and provenance.
- [#23](https://github.com/tcivie/eepview/pull/23): release gates job without lint exclusions; the macOS app is signed inside the dmg.
