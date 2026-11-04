---
feature: "io_uring is really tested in CI (EWE_REQUIRE_IO_URING=1), nativeapis uring tests fail loudly instead of skipping, foundation_core bare wasm32 build, reactor wasm dead code"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_iogate", "foundation_nativeapis", "foundation_proxy", "foundation_core", "foundation_reactor", ".github/workflows/check.yaml", "ci/packages.toml"]
catalogue_rows: [99, 100, 101, 103]
---

# Feature 78: io_uring in CI, core bare wasm, reactor wasm dead code

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Backlog entries that had no feature (owner priority 2026-11-04: valtron, nativeapis, reactor and io_uring first).

## E1. CI never runs the io_uring paths (backlog §9 "CI does not set `EWE_REQUIRE_IO_URING=1`")

Feature 72 E2 made the `foundation_iogate` and `foundation_proxy` io_uring tests fail when
`EWE_REQUIRE_IO_URING=1` and io_uring is missing. CI does not set the variable, so CI skips
them. The package jobs run in a Docker `container:` job, and Docker's default seccomp profile can
block `io_uring_setup`.
- Find out on CI, not by guess: add a short step in the `foundation_iogate` job that runs a
  probe (`io_uring_setup` through the crate's own detection) and prints the result.
- If the default seccomp profile blocks it, run the io_uring packages' container with
  `options: --security-opt seccomp=unconfined` (only those jobs, from `ci/packages.toml`), or
  run them on the runner host without the container. Document the choice in `ci/packages.toml`.
- Then set `EWE_REQUIRE_IO_URING=1` for `foundation_iogate`, `foundation_proxy` and
  `foundation_nativeapis` in CI, so a missing io_uring fails the job.
- The main session starts the run with `workflow_dispatch` (requirements.md §8.2 rule 4) and
  reads the result; you push the change and report.

## E2. nativeapis io_uring tests still skip without a check

`foundation_nativeapis` `uring_completion_*` and `completion_source_seam` tests skip when
io_uring is missing and do not read `EWE_REQUIRE_IO_URING`. Use the same helper as iogate
(one shared helper, not a copy, if a crate dependency allows it; else the same rule): with the
variable set, a missing io_uring fails the test with the reason; without it, the skip prints one
line that says why.

## E3. `foundation_core` does not build for wasm32 with neither `std` nor `alloc` (backlog §21)

`cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features` fails:
`valtron::Ctx` is unresolved in `executors/collect_next.rs` and `dependent_lift.rs`. Decide by
what the crate claims: if `no_std` without `alloc` is a supported target, gate the executors
that need `Ctx` on `alloc`; if not, make a bare build fail with a `compile_error!` that says
`alloc` is the minimum, and say it in the crate docs. Add the configuration to the
`foundation_core` `wasm-runs` in `ci/packages.toml` (as an expected build or an expected
error check).

## E4. `foundation_reactor` wasm dead code (`rearm_readable`, backlog §16)

This moves here from feature 49 E1. On wasm32, `Poll::rearm_readable` is never used. Gate it
on the native targets that call it (do not `allow(dead_code)`). Add the wasm32 check to
`foundation_reactor` in `ci/packages.toml` if it is not there.

## E5. `foundation_nativeapis` `vfs-preload` does not compile (found in feature 76)

`src/native/vfs/shim/mod.rs` (around lines 163 to 190) uses `guard` after it moved (`E0382`), so
the feature does not build and `tests/vfs_shim_vfs` cannot run. Fix the ownership (keep the
lock held for exactly the span the code needs; do not clone a guard), then run
`cargo test -p foundation_nativeapis --features vfs-preload --test vfs_shim_vfs` (its D1 setup
was moved to wrangler's local explorer API by feature 76 and is untested). Add the feature to
the package's CI runs in `ci/packages.toml` so it cannot break silently again.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
EWE_REQUIRE_IO_URING=1 cargo test -p foundation_iogate --profile dev
EWE_REQUIRE_IO_URING=1 cargo test -p foundation_nativeapis --profile dev
cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features
cargo check -p foundation_reactor --target wasm32-unknown-unknown
```
Full CI on GitHub (requirements.md §8.2 rule 4).

## Done when

- [ ] CI runs the io_uring tests with `EWE_REQUIRE_IO_URING=1` and they pass (or the job fails with a clear reason that the main session reports); nativeapis uring tests follow the same rule; the bare wasm32 core build is supported or refused with a clear error, checked in CI; reactor wasm dead code gone; `vfs-preload` builds and `vfs_shim_vfs` passes, and CI builds it; backlog entries deleted.
