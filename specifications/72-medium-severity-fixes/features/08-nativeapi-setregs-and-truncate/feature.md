---
feature: "The ptrace loop reports setregs failures and truncate rejects a negative length"
status: "pending"
priority: "medium"
depends_on: ["07-nativeapi-ptrace-dispatch"]
crates: ["foundation_nativeapis"]
issues: [209, 210]
model: "sonnet"
branch: "spec72/f08-nativeapi-setregs-and-truncate"
---

# Feature 08: Ptrace setregs and truncate

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** invalid argument error (`EINVAL`); virtual file system (VFS);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Goal

Stop swallowing `PTRACE_SETREGS` failures and reject a negative truncate length. Close issues
[#209](https://github.com/ewe-studios/ewe_platform/issues/209) and
[#210](https://github.com/ewe-studios/ewe_platform/issues/210).

## 2. Problem

1. `ptrace_loop` (`backends/foundation_nativeapis/src/native/vfs/ptrace/nix_backend.rs`) calls
   `ptrace::setregs(pid, r).ok()` for `Skip` (`:243`) and `SkipError` (`:248`). If the call
   fails, the tracee sees the kernel's result instead of the emulated one and the tracer
   reports nothing. Only `CloseThenSkip` (`:255`) maps the error to `VfsError::Backend`.
2. `ftruncate` (`backends/foundation_nativeapis/src/native/vfs/shim/mod.rs:754`) converts
   `length` with `usize::try_from(length.cast_unsigned()).unwrap_or(usize::MAX)` (`:761`). A
   negative length becomes a huge size, so the shim attempts a huge resize instead of
   returning `EINVAL`. The sibling `truncate` (`:732`) has the same conversion (`:739`).

## 3. Requirements

1. The `Skip` and `SkipError` arms return the setregs error as `CloseThenSkip` does.
2. The error names the operation and the pid where the existing error type allows it.
3. Both `truncate` and `ftruncate` reject `length < 0` with `EINVAL` before the resize.
4. No `unwrap_or` maps a bad input to a valid huge value.
5. The shim returns the errno through its existing return path.

## 4. Design

Research notes (2026-11-07, main session). These correct the first draft.

1. **Setregs helper.** The three arms (`Skip`, `SkipError`, `CloseThenSkip`) do the same thing:
   write `rax`, then call `ptrace::setregs`. Extract one public function in `nix_backend.rs`:

   ```rust
   pub fn report_return_value(
       pid: Pid,
       mut regs: libc::user_regs_struct,
       value: u64,
   ) -> VfsResult<()>
   ```

   It sets `regs.rax = value`, calls `ptrace::setregs(pid, regs)` and maps an error to
   `VfsError::Backend { message }`. The message names the value and the pid, as the current
   `CloseThenSkip` message does. The `Skip` arm passes `return_value as u64`. The `SkipError`
   arm passes `(-errno) as u64`. The `CloseThenSkip` arm passes `return_value as u64` and
   keeps its extra `close` message only if the value differs in wording; otherwise all three
   use one message. A `?` in each arm propagates the error.

   A live tracee cannot be made to fail `setregs` on purpose. The helper is public, so the
   test calls it with a pid that does not exist. `ptrace` then returns `ESRCH`, which is a
   real failure and a real error mapping. This is the sanctioned route: project rule 7 says
   make the right item public rather than test private code.

   A new public item is a version bump (memory rule). `foundation_nativeapis` goes from
   0.0.9 to 0.0.10, and the 6 manifests that require it move with it (`foundation_iogate`
   has two requirement lines).

2. **Truncate check.** The shim error convention is `set_errno(libc::EINVAL); return -1;`
   (for example `shim/mod.rs:278`, `:672`, `:945`). `return -EINVAL` from the first draft is
   wrong for this file. Put the check at the first line of both functions, before the path
   match:

   ```rust
   if length < 0 {
       set_errno(libc::EINVAL);
       return -1;
   }
   ```

   The kernel returns `EINVAL` for a negative length on every path, real or virtual, so the
   early check is correct for both. It also makes the test simple: no prefix setup is needed
   to reach the check.

3. **Conversion.** Replace `usize::try_from(length.cast_unsigned()).unwrap_or(usize::MAX)` with
   an explicit conversion. After the check, `length` is not negative, so the conversion cannot
   fail on a 64-bit target and can fail on a 32-bit target. Map a failure to `EINVAL` through
   `set_errno` and `-1`, never to `usize::MAX`.

## 5. Tasks

- [ ] Read both sites and the `VfsError` mapping.
- [ ] Add `report_return_value` and use it in the three arms.
- [ ] Add the negative-length check and the explicit conversion to both functions.
- [ ] Bump `foundation_nativeapis` to 0.0.10 and update the 6 consumer requirements and the
      example lockfiles.
- [ ] Add the test target `vfs_shim_truncate` to `Cargo.toml`.
- [ ] Add the regression tests.
- [ ] Run the verify commands. Record the results.
- [ ] Close #209 and #210 in the PR body.
- [ ] Record the change in `progress.md` and `learnings.md`.

## 6. Tests

Both tests go in `backends/foundation_nativeapis/tests/`. Never inline.

1. `tests/ptrace_vfs/mod.rs`, `a_failed_setregs_is_reported` (the file already gates on
   Linux and `vfs-ptrace-nix`). Call
   `nix_backend::report_return_value(Pid::from_raw(i32::MAX), regs, 0)`, with `regs` from
   `std::mem::zeroed()`, and assert `Err`. The error must be `VfsError::Backend` and its
   message must contain `setregs` and the pid. On the old code the function does not exist,
   so the test does not compile.
2. New test target `vfs_shim_truncate`: `tests/vfs_shim_truncate/mod.rs` with
   `#![cfg(all(feature = "vfs-preload", target_os = "linux"))]`, and a `[[test]]` entry with
   `required-features = ["vfs-preload"]`. One test does all of this, in order:
   - make a `TempDir`, create `<dir>/file` with 4 bytes;
   - set `FOUNDATION_VFS_PREFIX=/virtual` and `FOUNDATION_VFS_ROOT=<dir>` before the first
     shim call, so the two `LazyLock` statics read them;
   - call `shim::truncate(c"/virtual/file", -1)`: assert `-1` and
     `Error::last_os_error().raw_os_error() == Some(libc::EINVAL)`; assert the file is still
     4 bytes;
   - call `shim::open(c"/virtual/file", libc::O_RDWR, 0)` — the signature takes the mode —
     to get a virtual fd (>= 10_000), then call `shim::ftruncate(fd, -1)`: assert `-1` and
     `EINVAL`; `shim::close(fd)`; assert the file is still 4 bytes. A real fd from `open`
     would fall to `real::ftruncate`, which is not the code under test, so assert the fd is
     virtual first.

   On the old code the negative length reaches `data.resize(usize::MAX, 0)`, which panics with
   a capacity error, so both calls fail the test.

The shim test calls the functions in the test process, not through `LD_PRELOAD`. That is
enough: the statics and the VFS stack are the same ones the shim uses.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_nativeapis
cargo clippy --profile uat -p foundation_nativeapis --no-deps --all-targets -- -D warnings
# Tests, as CI runs them (the new target needs its feature):
cargo test --profile uat -p foundation_nativeapis --features vfs-preload,vfs-ptrace-nix,vfs-native
cargo test --profile uat -p foundation_nativeapis --test ptrace_vfs --features vfs-ptrace-nix,vfs-native
```

The ptrace spawn tests cannot run on this arm64 host under amd64 emulation
(`PTRACE_GETREGS` returns `EIO`; see `learnings.md`, feature 07). Run the non-spawn subset
locally and let native Linux CI prove the rest. Strict clippy stops on the known blockers
#336 and #200, so use `--no-deps` and record that in the PR.

## 8. Done when

- No ignored setregs result remains on the traced path.
- A negative length is `EINVAL` in both functions, and no `unwrap_or` maps it to a huge value.
- The regression tests failed on the old code (one by compile error, two by panic).
- Issues #209 and #210 are closed by the PR.
- `progress.md` and `learnings.md` record the change.
