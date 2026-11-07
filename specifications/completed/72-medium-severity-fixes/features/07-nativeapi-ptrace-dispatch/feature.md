---
feature: "Ptrace open, dup and write dispatch follow the kernel rules"
status: "in-review"
priority: "medium"
depends_on: []
crates: ["foundation_nativeapis"]
issues: [205, 206, 207, 208]
model: "sonnet"
branch: "spec72/f07-nativeapi-ptrace-dispatch"
---

# Feature 07: Ptrace dispatch correctness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** file descriptor (fd); open file description flag (close-on-exec
is `O_CLOEXEC`); virtual file system (VFS); AeroSpace and Defence Industries Association of
Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Fix four syscall dispatch defects in one file. Close issues
[#205](https://github.com/ewe-studios/ewe_platform/issues/205),
[#206](https://github.com/ewe-studios/ewe_platform/issues/206),
[#207](https://github.com/ewe-studios/ewe_platform/issues/207) and
[#208](https://github.com/ewe-studios/ewe_platform/issues/208).

## 2. Problem

File: `backends/foundation_nativeapis/src/native/vfs/ptrace/syscall_dispatch.rs`.

1. **Open mode.** `flags_to_perm` (`:1243`) computes `(flags >> 16) & 0o777`, but the mode is
   a separate argument: `arg2` of `open` and `arg3` of `openat`. `handle_open` (`:382`) and
   `handle_openat` (`:407`) pass the flags. A created file gets wrong permission bits.
   `handle_creat` already uses its own mode argument.
2. **Close-on-exec.** `handle_dup` (`:1178`) copies the source entry, including its flags.
   `dup` and `dup2` must clear `O_CLOEXEC`; `dup3` must set it from `arg2`, which is never
   read (`:1221` inserts the entry unchanged).
3. **fd range.** `handle_dup` takes `new_fd = args.arg1` with no check (`:1186-1221`). A
   negative number or one at or above `RLIMIT_NOFILE` must return `EBADF`.
4. **Append.** `open_virtual_path` stores the open flags, but `handle_vfs_write` (`:477`)
   always writes at the tracked offset and never checks `O_APPEND` (`:497`). A traced `>>`
   redirect overwrites from the start.

## 3. Requirements

1. `handle_open` passes `arg2`, and `handle_openat` passes `arg3`, to the open path. Delete
   `flags_to_perm` or leave it unused with a removal note.
2. `dup`/`dup2` clear `O_CLOEXEC` on the copy. `dup3` reads `arg2` and sets or clears the
   flag from it.
3. `handle_dup` rejects `new_fd < 0` and `new_fd >= RLIMIT_NOFILE` with `EBADF`. Read the
   tracee's limit, do not guess it.
4. `handle_vfs_write` moves the offset to the file size before the write when the entry has
   `O_APPEND`.
5. Every new branch returns the errno the kernel returns.
6. No new `unwrap` or `expect` on a syscall result.

## 4. Design

Keep the changes local to `syscall_dispatch.rs`, plus a small helper for the tracee's
`RLIMIT_NOFILE` if none exists. Each fix gets its own dispatch test that feeds the same
arguments the kernel would.

The range check reads the tracee's soft limit from `/proc/<pid>/limits`. It does not guess
the value and does not use the tracer's own limit.

## 5. Tasks

- [x] Read the four sites and the `VirtualFdEntry` flag type.
- [x] Fix the open mode argument.
- [x] Fix the close-on-exec handling.
- [x] Add the fd range check.
- [x] Add the append seek.
- [x] Add the four regression tests.
- [x] Run the verify commands. Record the results.
- [x] Close #205, #206, #207 and #208 in the PR body.

## 6. Tests

In `backends/foundation_nativeapis/tests/`:

1. `open_uses_its_mode_argument`: `openat` with mode `0o640` creates an entry with `0o640`.
2. `dup_clears_close_on_exec` and `dup3_sets_close_on_exec_from_its_flags`.
3. `dup_rejects_an_out_of_range_fd`: a negative number and `RLIMIT_NOFILE` both return
   `EBADF`.
4. `an_append_write_goes_to_the_end`: open with `O_APPEND`, write twice, and assert the
   offsets and the final bytes.

Each test must fail on the old code.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_nativeapis
cargo clippy --profile uat -p foundation_nativeapis --all-targets -- -D warnings
cargo test --profile uat -p foundation_nativeapis
```

## 8. Done when

- The four behaviours match the kernel rules.
- Each regression test failed on the old code.
- Issues #205–#208 are closed by the PR.
- `progress.md` and `learnings.md` record the change.
