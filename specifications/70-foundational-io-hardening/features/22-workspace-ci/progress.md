# Feature 22: progress

Branch: `fix/io-20-vfs-process-ipc`.

## Done

- **E1.** `scripts/ci/ci.py` now picks a target directory per worktree. The
  default is `<repo>/target`. When `CARGO_TARGET_DIR` is set, the script uses
  `$CARGO_TARGET_DIR/<checkout-name>`. `.agents/AGENTS.md` documents this rule
  where worktrees are described. The backlog entry "Shared target dir
  clobbering" is deleted in the same commit.
- **E2.** `crates/ewe_deployables` is deleted. No code uses it, and the
  per-provider deployment crates cover it. The stale comment in the workspace
  `Cargo.toml` is removed. The backlog entry "`crates/ewe_deployables` doesn't
  build" is deleted in the same commit.
- **E5.** The `Command::Bindgen(args)` binding in `foundation_testbed` now lives
  behind a `run_bindgen_command` helper gated with the same `cfg` as its use, so
  the `wasm-embedded-js` build no longer warns about an unused `args`. The
  backlog entry "`wasm-testbed` warns on `Command::Bindgen(args)`" is deleted in
  the same commit. A new backlog entry records the `deno_v8` unused-dependency
  manifest lint surfaced by cargo nightly 1.100.0.
- **E6.** A new `mise.wasm.toml` environment pins only deno and wasm-bindgen,
  sets `auto_install = false`, and holds the two wasm deno tasks. `mise.toml`
  drops those two tools and tasks. `MISE_ENV=wasm mise run test:wasm-bindgen:deno`
  now installs only deno and wasm-bindgen instead of the whole toolchain. The
  backlog entry "Running a wasm mise task installs every tool" is deleted in the
  same commit.
- **E7.** The four SignalR frame helpers move from `server/signalr_do.rs` (wasm
  only) to the portable `core::notifications` module as public functions. The
  unused `notifications::self` import and the unneeded `mut resp` are dropped,
  and both inline test modules move to `tests/notifications.rs` (rule 7). Moving
  the test exposed and fixed a real bug: `decode_signalr_frame` decoded the whole
  payload as a scalar instead of the array's first element. `foundation_keychain`
  bumps to 0.1.3 for the new public API. The §21 keychain backlog entry is
  deleted in the same commit; a new entry records the `keychain.rs` recursion
  lint. A follow-up commit cleans the 13 pedantic clippy warnings the native
  clippy gate now surfaces for the moved helpers.
- **E8.** The unused `worker::Error` import in
  `foundation_deployment_cloudflare/src/workers/durable_object.rs` is dropped.
  The §21 cloudflare entry is replaced with a new entry for the five
  `unused_dependencies` manifest lints the wasm check surfaces (out of E8
  scope; recorded per rule 9).
- **E4.** A synthetic `wasm-tests` CI package (in `scripts/ci/ci.py` and
  `ci/packages.toml`) checks every crate with wasm tests. Its build step runs
  `cargo check --tests --target wasm32-unknown-unknown` for foundation_core
  (`std,js-wasmbindgen`), foundation_netio (`wasm-fetch`), foundation_wireguard
  (`wasm-bindgen-test`) and the standalone `foundation_core/integration/wasm_tests`
  crate. Its test step runs `mise run test:wasm-testbed:deno` and
  `mise run test:wasm-bindgen:deno` under `MISE_ENV=wasm`. To make those checks
  pass, this pass also gates the native-only `stream_future` test module out of
  the wasm build, fixes the rotted netio and wireguard wasm tests, moves
  testbed's `foundation_core/multi` feature to a native-only target table,
  gates `qemu_control` out of wasm, and vendors a boringtun patch that adds the
  missing wasm32 sleepyinstant backend. `wasm-bindgen` is restored to
  `mise.toml [tools]` (the CI image version check needs it). The two §16
  wasm-runtime backlog entries are deleted in the same commit; a new entry
  records the foundation_reactor wasm dead-code warning.
- **E3.** The generated Cloudflare groups split into one crate per group (43
  crates: `foundation_cloudflare_<group>`). The generator now writes them:
  `foundation_codegentools` gained `split_groups` plus a `check()` that stages
  the split trees in a scratch dir and merges them, and `foundation_openapi`
  gained `write_split`, `write_group_crate`, `write_umbrella_shim` and
  `rewrite_umbrella_for_split_str`. The shared resource types live in
  `foundation_cloudflare_types` (feature 17 E2); each group crate's local
  `shared` module re-exports them and `foundation_deployment::providers::common`.
  The umbrella re-exports every group behind `cloudflare_<group>` features, so
  `cloudflare`, `cloudflare_all`, `default` and `full` keep the old callers
  unchanged (`generated::<group>::<Type>`). The umbrella bumps to 0.2.0;
  `foundation_keychain` bumps to 0.1.4 and `foundation_proxy` to 0.1.6 for the
  new requirement. The §16 "Cloudflare crate size" backlog entry is deleted, and
  the §21 entry is corrected (the split removed `base85rs` and `sha2`). Build:
  `cargo build -p foundation_deployment_cloudflare --profile uat` drops from
  103.8 s to 60.8 s (warm); the umbrella rlib shrinks from 88,171,184 to
  978,392 bytes, the 43 group crates total 84,422,184 bytes, and the shared
  types crate is 9,122,824 bytes. Umbrella tests pass (17), and the proxy and
  keychain build unchanged on native and wasm.

## In progress

- None.

## Next

- None.
