<!-- SPDX-License-Identifier: MIT -->

# Reproducible builds

Status: settings only. No CI job checks it.

The release build uses deterministic settings. They cost nothing. CI does not verify that two builds of one commit match. The per-PR double-build check was removed on 2026-10-03, because it doubled build time for little value. Reproducibility is no longer verified in CI.

## How the build is made repeatable

`scripts/repro-env.sh` prints the environment. `release.yml` adds it to `GITHUB_ENV` before the Linux build:

- `SOURCE_DATE_EPOCH` is the commit time.
- `CARGO_INCREMENTAL=0`.
- `RUSTFLAGS` has `--remap-path-prefix` for the home and checkout paths, so the binary holds no build path.

The build runs `npx tauri build --no-bundle -- --locked`, then `scripts/split-debug.sh`, then `npx tauri bundle`.

## What is not claimed

Do not treat any release file as reproducible. Nobody checks it on any PR or release. The last check, in PR [#37](https://github.com/tcivie/eepview/pull/37), found the stripped Linux binary and the frontend `dist/` identical in two builds. It found the `.deb` different, because of packaging time stamps. The AppImage, the macOS `.dmg` files and the Windows installer were never checked.

## Hardening

`scripts/check-hardening.sh` checks the Linux binary in the release build:

```
ok   PIE (ELF type):  Type: DYN (Position-Independent Executable file)
ok   PIE (FLAGS_1):  Flags: NOW PIE
ok   RELRO segment:  GNU_RELRO
ok   BIND_NOW (full RELRO):  (FLAGS) BIND_NOW
ok   NX stack (GNU_STACK RW, not RWE):  GNU_STACK ... RW
```

That means: position-independent (PIE), full RELRO with `BIND_NOW`, and a non-executable stack (NX).

To check one binary on your machine:

```sh
./scripts/check-hardening.sh src-tauri/target/x86_64-unknown-linux-gnu/release/eepview
```

## History

- [#37](https://github.com/tcivie/eepview/pull/37): repeatable Linux builds, `reproducible` CI job, hardening check.
- 2026-10-03, PR to be linked: removed the `reproducible` CI job and `scripts/repro-check.sh`. The deterministic settings stay.
