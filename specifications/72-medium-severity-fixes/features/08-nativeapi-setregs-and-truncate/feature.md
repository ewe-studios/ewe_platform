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
2. `ftruncate` (`backends/foundation_nativeapis/src/native/vfs/shim/mod.rs:742`) converts
   `length` with `usize::try_from(length.cast_unsigned()).unwrap_or(usize::MAX)`. A negative
   length becomes a huge size, so the shim attempts a huge resize instead of returning
   `EINVAL`. The sibling `truncate` (`:726-729`) has the same conversion.

## 3. Requirements

1. The `Skip` and `SkipError` arms return the setregs error as `CloseThenSkip` does.
2. The error names the operation and the pid where the existing error type allows it.
3. Both `truncate` and `ftruncate` reject `length < 0` with `EINVAL` before the resize.
4. No `unwrap_or` maps a bad input to a valid huge value.
5. The shim returns the errno through its existing return path.

## 4. Design

- In `nix_backend.rs`, replace `.ok()` with `?` and the same `VfsError::Backend` mapping the
  `CloseThenSkip` arm uses.
- In `shim/mod.rs`, add an early `if length < 0 { return -EINVAL; }` in both functions. Keep
  the successful conversion explicit.

## 5. Tasks

- [ ] Read both sites and the `VfsError` mapping.
- [ ] Propagate the setregs error.
- [ ] Add the negative-length check to both functions.
- [ ] Add the regression tests.
- [ ] Run the verify commands. Record the results.
- [ ] Close #209 and #210 in the PR body.

## 6. Tests

In `backends/foundation_nativeapis/tests/`:

1. `a_failed_setregs_is_reported`: inject a failure and assert the loop returns the error,
   not that it continues. If the failure is hard to inject, test the helper that maps the
   result.
2. `truncate_rejects_a_negative_length` and `ftruncate_rejects_a_negative_length`: both
   return `EINVAL` and leave the file size unchanged. On the old code the size changes or the
   call attempts a huge resize.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_nativeapis
cargo clippy --profile uat -p foundation_nativeapis --all-targets -- -D warnings
cargo test --profile uat -p foundation_nativeapis
```

## 8. Done when

- No ignored setregs result remains on the traced path.
- A negative length is `EINVAL` in both functions.
- The regression tests failed on the old code.
- Issues #209 and #210 are closed by the PR.
- `progress.md` and `learnings.md` record the change.
