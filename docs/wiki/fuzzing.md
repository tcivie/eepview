# Fuzzing

Status: in progress, in [#62](https://github.com/tcivie/eepview/pull/62).

Fuzzing feeds random input to the parsers that decide what eepview may load. It checks that they never panic and never let a clearnet address through. It runs on every code change and once a day.

## How it works

- The targets are cargo-fuzz targets in `src-tauri/fuzz/`. That crate has its own workspace. It is not a member of the app workspace, so `cargo build --workspace` and the release build never compile it, and it adds nothing to `src-tauri/Cargo.lock`. Each target calls the public API of `eepview_lib` and asserts only what the docs state.
- `libfuzzer-sys` is pinned to `=0.4.13`. Its build script compiles libFuzzer (C++), which Socket flags. The isolation above is why that is safe.
- The fuzz build starts from the lock file of the app (`src-tauri/Cargo.lock`), so the fuzzer tests the versions that ship, `url` included. `fuzz/Cargo.lock` is not committed.
- ClusterFuzzLite runs the targets in CI. `.clusterfuzzlite/` holds the build image (Ubuntu 24.04, pinned by digest), `build.sh` and `project.yaml`. `.github/workflows/fuzz.yml` runs the targets. The Tauri crates link GTK and WebKitGTK, so `build.sh` copies their shared libraries next to each target.
- A pull request that changes `src-tauri/src/` or the fuzz setup gets a code-change run of 20 minutes in all, about 5 minutes per target.
- Main gets a batch run every day at 03:41 UTC, 60 minutes in all. ClusterFuzzLite stores the corpus as a GitHub Actions artifact and lists it with the workflow token (`actions: read`), so each run starts from the last one.
- The fuzz job and the branch-coverage job use one pinned nightly toolchain, named in `scripts/nightly-toolchain.txt`. The app stays on the stable toolchain of `rust-toolchain.toml`. Nothing sets `RUSTC_BOOTSTRAP`.
- The fuzz job is not a required check. The `lint` workflow runs rustfmt and clippy (pedantic, `-D warnings`) on the fuzz crate too.

| Target | API | Properties |
|---|---|---|
| `host` | `net::host::is_i2p_host` | Never panics. An accepted host ends in `.i2p`, is lower-case ASCII, and has no port, no user info, no empty label and no `xn--` label. |
| `address_bar` | `nav::classify` | Never panics. A web target is `http(s)` on an I2P host with no user info. An internal target is a bundled page. A search is one word with no dot and no colon. |
| `gatekeeper_request` | `net::http::{Head::parse, plan, plan_inner, upstream_request}` | Never panics. The gatekeeper forwards, tunnels or relays an I2P host only. The request that it sends to the router has one `Host` header, the planned host, and no bare CR or LF inside a line. |
| `url_rules` | `nav::guard`, `nav::is_allowed`, `nav::host_of`, `net::rules::{engine_allows, I2P_URL_PATTERN}` | For any `http(s)` URL, the guard and the engine rule allow only an I2P host with no user info, and the L3b WebKit pattern allows exactly what the guard allows. |

## How to use / run locally

Install cargo-fuzz and the pinned nightly once:

```sh
cargo install cargo-fuzz --locked
rustup toolchain install "$(cat scripts/nightly-toolchain.txt)" --profile minimal
```

Prepare and run one target from `src-tauri`. Put the build output on a big disk with `CARGO_TARGET_DIR`:

```sh
cd src-tauri
mkdir -p ../dist                      # tauri::generate_context! needs the folder
cp Cargo.lock fuzz/Cargo.lock         # fuzz the versions that ship
cargo "+$(cat ../scripts/nightly-toolchain.txt)" fuzz run host -- -max_total_time=60 -dict=fuzz/dictionaries/host.dict
```

`cargo fuzz list` shows every target. The URL targets use `fuzz/dictionaries/url.dict`. The first build compiles Tauri, so it takes a few minutes. It opens no window. On macOS the sanitizer build of the `cdylib` fails to link; use `--sanitizer none` with `RUSTFLAGS="-Clink-arg=-Wl,-undefined,dynamic_lookup"`.

To replay a crash input: `cargo fuzz run <target> <file>`.

## Where crashes show up

- On a pull request, a crash fails the `fuzz` job. The log shows the input and the stack. The job uploads the input as the `fuzz-crashes` artifact.
- A crash in the daily run fails the `fuzz` run on main in the Actions tab, with the same log and artifact.
- A crash is a bug. Fix it and add a regression test for the input. Do not silence the target.

## Limits

- The targets check the properties above, not full behaviour. The unit tests and the [leak test](leak-test.md) check the rest.
- The router proxy check, the sockets and the web views are not fuzzed. They need a runtime.

## History

- 2026-10-03 — Add cargo-fuzz targets and ClusterFuzzLite — [#62](https://github.com/tcivie/eepview/pull/62)
