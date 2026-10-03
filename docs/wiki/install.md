<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/tcivie/eepview/main/assets/brand/eepview-logo-horizontal-dark.svg">
    <img alt="eepview" src="https://raw.githubusercontent.com/tcivie/eepview/main/assets/brand/eepview-logo-horizontal-light.svg" width="280">
  </picture>
</p>

# Install eepview

Status: in progress. eepview has no release yet. This page says how installing will work.

## What you need

- macOS 14 or later.
- Windows 10 or later, with WebView2.
- Linux with WebKitGTK 4.1.

eepview uses the web view of your operating system. It does not ship its own browser engine.

## Download

Releases will appear on the [GitHub Releases page](https://github.com/tcivie/eepview/releases) (when released). Pick the file for your system.

## Verify the download

Every release lists a `SHA256SUMS` file. Check your file against it.

```sh
shasum -a 256 <file>
```

Compare the result with the line for your file in `SHA256SUMS`. They must match.

Every release file also has a build attestation. It proves that GitHub Actions built the file from this repository. Check it with the GitHub CLI.

```sh
gh attestation verify <file> --repo tcivie/eepview
```

Do not run a file that fails one of these checks. See [Release pipeline](release-pipeline.md) for how the files are built.

## First-run warnings

eepview is not signed with a paid developer certificate yet. Your system may warn you. This is expected.

- macOS Gatekeeper says the app is from an "unidentified developer". Right-click the app, choose Open, and confirm.
- Windows SmartScreen says it protected your PC. Choose More info, then Run anyway.

Run the verify steps above first. Then the warning is safe to pass.

## Build from source

You can build eepview yourself instead. The [developer guide](developer-guide.md) lists the steps.

## Next

Read [First run](first-run.md).

## History

- 2026-10-03 — Write the page — [#28](https://github.com/tcivie/eepview/pull/28)
