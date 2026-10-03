# Fuzzing

Status: in progress, in [#62](https://github.com/tcivie/eepview/pull/62).

Fuzzing feeds random input to the parsers that decide what eepview may load. It checks that they never panic and never let a clearnet address through. It runs on every code change and once a day.

## How it works

- The targets are cargo-fuzz targets in `src-tauri/fuzz/`. That crate has its own workspace, so `cargo build --workspace` in `src-tauri` never builds it. Each target calls the public API of `eepview_lib` and asserts only what the docs state.
- ClusterFuzzLite runs them in CI. `.clusterfuzzlite/` holds the build image (pinned by digest), `build.sh` and `project.yaml`. `.github/workflows/fuzz.yml` runs the targets.
- A pull request that changes `src-tauri/src/` or the fuzz setup gets a code-change run of 20 minutes in all, about 5 minutes per target.
- Main gets a batch run every day at 03:41 UTC, 60 minutes in all. The corpus lives in the Actions cache, so each run starts from the last one.
- Only the fuzz job uses a nightly toolchain, pinned to one date (`nightly-2026-10-01`, set in `.clusterfuzzlite/build.sh`). The app stays on the stable toolchain of `rust-toolchain.toml`. Nothing sets `RUSTC_BOOTSTRAP`.
- The fuzz job is not a required check.

| Target | API | Properties |
|---|---|---|
| `host` | `net::host::is_i2p_host` | Never panics. An accepted host ends in `.i2p`, is lower-case ASCII, and has no port, no user info, no empty label and no `xn--` label. |
| `address_bar` | `nav::classify` | Never panics. A web target is `http(s)` on an I2P host with no user info. An internal target is a bundled page. A search is one word with no dot and no colon. |
| `gatekeeper_request` | `net::http::{Head::parse, plan, plan_inner}` | Never panics. The gatekeeper forwards, tunnels or relays an I2P host only. A tunnel request keeps the tunnel host. |
| `url_rules` | `net::rules::engine_allows`, `nav::guard`, `nav::is_allowed`, `nav::host_of` | For any URL, the L3 engine rule and the L4 guard allow `http(s)` only on an I2P host, and they agree with each other. |

## How to use / run locally

Install cargo-fuzz and the pinned nightly once:

```sh
cargo install cargo-fuzz --locked
rustup toolchain install nightly-2026-10-01 --profile minimal
```

Run one target from `src-tauri`. Put the build output on a big disk with `CARGO_TARGET_DIR`:

```sh
cd src-tauri
cargo +nightly-2026-10-01 fuzz run host -- -max_total_time=60 -dict=fuzz/dictionaries/host.dict
```

`cargo +nightly-2026-10-01 fuzz list` shows every target. The first build compiles Tauri, so it takes a few minutes. It opens no window.

To replay a crash input: `cargo +nightly-2026-10-01 fuzz run <target> <file>`.

## Where crashes show up

- On a pull request, a crash fails the `fuzz` job. The log shows the input and the stack. The job uploads the input as the `fuzz-crashes` artifact.
- A crash in the daily run fails the `fuzz` run on main in the Actions tab, with the same log and artifact.
- A crash is a bug. Fix it and add a regression test for the input. Do not silence the target.

## Limits

- The targets check the properties above, not full behaviour. The unit tests and the [leak test](leak-test.md) check the rest.
- The router proxy check, the sockets and the web views are not fuzzed. They need a runtime.

## History

- 2026-10-03 — Add cargo-fuzz targets and ClusterFuzzLite — [#62](https://github.com/tcivie/eepview/pull/62)
