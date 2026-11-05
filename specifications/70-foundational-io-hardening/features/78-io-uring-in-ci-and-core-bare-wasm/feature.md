---
feature: "io_uring is really tested in CI (EWE_REQUIRE_IO_URING=1), nativeapis uring tests fail loudly instead of skipping, foundation_core bare wasm32 build, reactor wasm dead code"
status: "done"
lane: "A"
model: "sonnet"
branch: "spec70/f77-f62-f78-readiness-ci"
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

- [x] CI runs the io_uring tests with `EWE_REQUIRE_IO_URING=1` and they pass (or the job fails with a clear reason that the main session reports); nativeapis uring tests follow the same rule; the bare wasm32 core build is supported or refused with a clear error, checked in CI; reactor wasm dead code gone; `vfs-preload` builds and `vfs_shim_vfs` passes, and CI builds it; backlog entries deleted.

## Result (2026-11-05)

**E1.** `ci/packages.toml` now sets `EWE_REQUIRE_IO_URING=1` for `foundation_iogate`,
`foundation_proxy` and `foundation_nativeapis`. The three packages get
`container-options = "--security-opt seccomp=unconfined"`: Docker's default seccomp profile
blocks `io_uring_setup`, so only the io_uring jobs leave the default profile. `ci.py` adds that
string to the package job's `container:` job, its `docker run` in the artifact path, and the
matrix carries it. A new Linux-only target, `foundation_iogate/tests/io_uring_probe.rs`, starts
the completion reactor through the crate's own detection and prints
`io_uring probe: available` or `io_uring probe: unavailable: <reason>`. The workflow runs it
before the package tests in both job paths. The main session must start `workflow_dispatch` and
read the probe line; this host cannot run io_uring.

**E2.** `foundation_nativeapis/tests/common/mod.rs` holds one helper, `io_uring_missing`, with
the `Requires::{IoUring, Completion}` choice. The four suites (`uring_completion_tests`,
`uring_completion_send_tests`, `uring_completion_syscall_tests`, `completion_source_seam_tests`)
call it. With `EWE_REQUIRE_IO_URING=1` a missing io_uring or completion tier fails the test with
the reason; without it, one line says the test skipped. The suites were not compiled in CI
before: they need `feature = "uring"`, and no run enabled it. `fd,uring` is now a CI run. The
`io-uring` dependency moved to the Linux target table in `foundation_nativeapis/Cargo.toml`, so
the feature compiles (to the empty Linux-gated tests) on other targets.

**E3.** Decision: `std` is the minimum, not `alloc`. `valtron::Ctx` and its executors need
`std`, and `valtron::ctx` documents that `alloc`-only support is deferred; a bare
`--no-default-features` build emits 15 unresolved-`Ctx` errors. The crate root now has a
`compile_error!` that names `std`, plus a "Feature sets" section in the crate docs. `ci.py`
gained a `wasm-expect-error` setting: `scripts/ci/expect_compile_error.py` runs the build and
requires that exact message. `ci/packages.toml` uses it for `foundation_core`. The supported
minimum (`--no-default-features --features std`) was already a wasm run and still passes.

**E4.** `Poll::rearm_readable` is now `#[cfg(unix)]`, the same condition as its one caller,
`Reactor::clear` in the Unix-only reactor module. `foundation_reactor` gets a wasm32 check in
`ci/packages.toml` (`wasm = "enforce"`, `wasm-runs = [""]`), and that check passes.

**E5.** The E0382 was the `drop(guard)` in the D1 and R2 error closures of
`src/native/vfs/shim/mod.rs`: it moved the guard before `VfsRuntime { guard: Some(guard) }`.
The closures now only panic, and a panic unwinds the frame and drops the guard. `vfs-preload`,
`vfs-preload,vfs-d1` and `vfs-preload,vfs-r2` all build. `vfs-preload` is its own CI run. The
`vfs_shim_vfs` target compiles on macOS (0 tests, Linux-gated); the real run needs Linux and
wrangler's local explorer API, so this host did not run it.

Backlog entries deleted: the §9 io_uring CI entry, the §16 `rearm_readable` entry, the §22
bare-core entry, and the `vfs-preload` entry. Added: the bare build's extra error noise (§4) and
the two `ignore`d `foundation_nativeapis` doc examples (§5).
