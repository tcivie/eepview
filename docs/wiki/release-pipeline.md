# Release pipeline

Status: shipped.

A `v*` tag builds installers for four targets, adds SBOMs and checksums, and makes a draft GitHub release.

## How it works

- A push of a tag that matches `v*` starts `release.yml`. A manual run (`workflow_dispatch`) does the same, with the `dry_run` input.
- The `lint`, `security` and `ci` jobs call the reusable workflows `lint.yml`, `security.yml` and `ci.yml`. The `build` job needs all three.
- The `build` job has four legs. Each runs `npx tauri build` with no third-party release action.
  - `x86_64-pc-windows-msvc` on `windows-2025`: NSIS installer.
  - `aarch64-apple-darwin` on `macos-15`: `.app` and `.dmg`.
  - `x86_64-apple-darwin` on `macos-15`: `.app` and `.dmg`.
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
- On macOS only the `.app` gets an ad-hoc signature. The `.dmg` is not rebuilt, so the app inside it is unsigned. There is no Developer ID signature and no notarization.
- Windows installers are not signed.
- Provenance is skipped while the repo is private. See [going-public.md](../going-public.md).
- `release.yml` calls the reusable workflows with `./`. zizmor asks for `$/`, and actionlint rejects it, so one inline zizmor ignore stays.

## History

- [#14](https://github.com/tcivie/eepview/pull/14): release pipeline with SBOM, checksums and provenance.
