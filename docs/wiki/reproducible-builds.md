<!-- SPDX-License-Identifier: MIT -->

# Reproducible builds

Status: shipped.

The Linux release binary is built the same way twice. The CI job `reproducible (ubuntu-24.04)` proves it on every PR. The numbers below come from the job log of PR [#37](https://github.com/tcivie/eepview/pull/37).

## What is reproducible

The job builds the commit twice, in two different directories, and compares the outputs.

| Output | Result | Gate |
| --- | --- | --- |
| Stripped Linux binary (`x86_64-unknown-linux-gnu`) | identical | fails the job if it differs |
| Frontend `dist/` (tar, sorted, fixed time) | identical | fails the job if it differs |
| Debug symbols (`.debug`) | compared and reported | no |
| `.deb` | differs | no |

The job passed, so the binary and `dist/` hashes match. The SHA-256 values of both builds are in the job summary (`Reproducible build` table), not in the step log.

## What is not reproducible

The `.deb` differs between the two builds. `diffoscope` shows only time stamps:

- Every file in the `.deb`, and in its `control.tar.gz` and `data.tar.gz`, carries the time of the build (10:46:51 in build A, 10:50:14 in build B). It does not carry `SOURCE_DATE_EPOCH`.
- The two inner archives differ in size by 1 or 2 bytes, because of those time stamps.
- The binary inside the `.deb` has the same size in both builds (4733928 bytes).

So the content is the same and the packaging time is not. The AppImage, the macOS `.dmg` files and the Windows installer are not checked. Do not treat them as reproducible.

## How the build is made repeatable

`scripts/repro-env.sh` prints the environment:

- `SOURCE_DATE_EPOCH` is the commit time.
- `CARGO_INCREMENTAL=0`.
- `RUSTFLAGS` has `--remap-path-prefix` for the home and checkout paths, so the binary holds no build path.

The build runs `npx tauri build --no-bundle -- --locked`, then `scripts/split-debug.sh`, then `npx tauri bundle`.

## Hardening

`scripts/check-hardening.sh` checks the Linux binary. The release build and the `reproducible` job print the same result:

```
ok   PIE (ELF type):  Type: DYN (Position-Independent Executable file)
ok   PIE (FLAGS_1):  Flags: NOW PIE
ok   RELRO segment:  GNU_RELRO
ok   BIND_NOW (full RELRO):  (FLAGS) BIND_NOW
ok   NX stack (GNU_STACK RW, not RWE):  GNU_STACK ... RW
```

That means: position-independent (PIE), full RELRO with `BIND_NOW`, and a non-executable stack (NX).

## Check it on your machine

On Linux (`ubuntu-24.04`, with the WebKitGTK build dependencies):

```sh
./scripts/repro-check.sh /tmp/repro
```

It builds twice with `git worktree`, prints a table of both hashes, and runs the hardening check. Install `diffoscope` to see the `.deb` differences. To check one binary alone:

```sh
./scripts/check-hardening.sh src-tauri/target/x86_64-unknown-linux-gnu/release/eepview
```

## History

- [#37](https://github.com/tcivie/eepview/pull/37): repeatable Linux builds, `reproducible` CI job, hardening check.
