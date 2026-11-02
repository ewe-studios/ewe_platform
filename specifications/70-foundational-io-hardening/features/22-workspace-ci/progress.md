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

## In progress

- None.

## Next

- E4 and E3 stay for a later pass.
