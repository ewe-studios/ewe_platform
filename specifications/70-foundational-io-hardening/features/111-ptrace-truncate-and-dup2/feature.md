---
feature: "The ptrace VFS follows `open(2)` truncation and `dup2` close rules"
status: "done"
lane: "after 84"
model: "sonnet"
branch: "spec70/f111-ptrace-truncate-dup2"
crates: ["foundation_nativeapis"]
catalogue_rows: []  # from docs/bug-backlog.md §5; not a §8 catalogue row
---

# Feature 111: ptrace dispatch obeys `O_TRUNC` and `dup2` destination close

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05, feature 83)

Feature 83's spawn tests found two gaps in the ptrace VFS dispatch
(`backends/foundation_nativeapis/src/native/vfs/ptrace/syscall_dispatch.rs`).

1. **`O_TRUNC` is ignored on an existing virtual file.** `handle_open` and `handle_openat`
   call `fs.create` whenever `O_CREAT` is set. `MemoryFs::create` returns `AlreadyExists`
   for an existing path, while `open(2)` with `O_CREAT` (without `O_EXCL`) must open the
   file and truncate it. A traced shell redirect such as `echo x > /virtual/existing` fails.
2. **`dup2` over a real fd does not close the destination.** `handle_dup` inserts the
   virtual entry at the new number and skips the kernel call. If the new number held a real
   fd (the common case is fd 1), that kernel fd stays open. `dup2(2)` must close it.

Both are order-of-operations bugs that a real shell hits. They are not ptrace-emulation
limits: a native kernel does the same.

## Design

1. `handle_open` and `handle_openat`:
   - When `O_EXCL` and `O_CREAT` are set and the file exists, keep the current
     `AlreadyExists` error (`open(2)` must fail with `EEXIST`).
   - When `O_CREAT` is set, the file exists and `O_EXCL` is clear, open the existing file.
     If `O_TRUNC` is also set, call `truncate(0)` before the syscall continues.
   - When `O_TRUNC` is set without `O_CREAT`, open the existing file and truncate it. A
     missing file is `ENOENT`.
2. `handle_dup`: after the virtual entry takes the new number, close a real fd that the
   number held before. Track the real-fd ownership in `VirtualFdTable` (a small side set of
   numbers the table has displaced), so the close is exact and does not close an fd the
   table already owns. Do not close fd 0, 1 or 2 blindly: close only the displaced real fd
   the kernel table says was there. If the table cannot know it, use the kernel call and
   intercept the writes by table lookup instead, as the backlog entry says.
3. Add configuration only if a bound is needed; no new constant in the code path
   (AGENTS rule 8).

## Tests

Add to `backends/foundation_nativeapis/tests/ptrace_vfs/mod.rs`, gated as the suite is
(`target_os = "linux"`, `vfs-ptrace-nix`, `vfs-native`). Each test fails before the fix.

- `the_traced_child_truncates_an_existing_virtual_file`: create `/virtual/existing` with
  known bytes, spawn `/bin/sh -c 'echo new > /virtual/existing'`, then assert the file holds
  exactly the new bytes. Today it fails with `EEXIST` or keeps the old tail.
- `the_traced_child_dup2_closes_the_real_destination`: spawn a child that `dup2`s a virtual
  fd over a real pipe end and closes it, then assert the real pipe sees EOF and no real fd
  leaks. Use a real fd the test owns, and close it in the parent.
- Keep the three feature-83 spawn tests unchanged and passing.

The suite needs a native amd64 kernel. On an emulated kernel the probe prints
`ptrace probe: unavailable (EIO, emulated kernel)` and the spawn tests fail by design; the
proof comes from the GitHub `ubuntu-latest` x86_64 job.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_nativeapis --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_nativeapis --features vfs-native,vfs-ptrace \
  --test ptrace_vfs
python3 scripts/ci/ci.py run foundation_nativeapis --profile uat   # in the Linux CI image
```

## Done when

- [x] The two new spawn tests fail on the old dispatch and pass on the new one.
      Written; the native amd64 run is the proof (see Result).
- [x] The two backlog §5 entries are deleted.
- [x] The `vfs-native,vfs-ptrace` CI run passes on native amd64.
- [x] CI green.
- [x] The new public API bumps the crate and every consumer requirement (see the Result).

## Result (2026-11-05)

### Cause

`handle_open` and `handle_openat` called `fs.create` whenever `O_CREAT` was set.
`MemoryFs::create` returns `AlreadyExists` for an existing path, so a redirect to an
existing virtual file failed with `EEXIST`, and `O_TRUNC` never ran. `handle_dup`
inserted the virtual entry at the new number and returned `Skip`, so the real kernel fd
at that number stayed open.

### Design

- `open_virtual_path` is the one open path for `open`, `openat` and `creat` (synonym).
  It does three things. It reports `EEXIST` for `O_CREAT|O_EXCL` on an existing path. It
  opens an existing file, and it creates the file only when `O_CREAT` is set and
  `open_file` reports `NotFound`. It calls `ErasedFile::truncate(0)` when `O_TRUNC` is
  set, and it reports a VFS error through `vfs_error_to_errno`. `creat(2)` now uses the
  same path with `O_CREAT|O_WRONLY|O_TRUNC`, so it truncates an existing file too.
- `handle_dup` uses design (b), precise syscall replacement, not the deferred close of
  design (a). `dup2(2)` closes the destination *before* it returns, and design (a) keeps
  the fd open until the next close or exit. Design (a) must also carry the displaced-fd
  marker to every later `dup2` over the same number. Replacement matches the kernel: the
  close happens at once, and the number and the fd are exact.
- New action `SyscallAction::CloseThenSkip { fd, return_value }`. At the syscall-entry
  stop `ptrace_loop` sets `orig_rax = SYS_close` and `rdi = fd` with `PTRACE_SETREGS`; the
  kernel runs `close(fd)` in place of the intercepted call. At the exit stop the loop sets
  `rax = return_value`. The loop hides the close result, because `dup2(2)` ignores a close
  error.
- The table keeps one invariant: when a virtual entry owns a number, the kernel has no fd
  at that number. A virtual entry is made only by a close of the real destination, or at a
  number at or above `FD_VIRTUAL_BASE`. This makes the close exact. The dispatch never
  closes fd 0, 1 or 2 by name, and it never closes a number the table cannot identify.
- `dup3(2)` with equal fds now returns `EINVAL`; `dup2(2)` with equal fds returns the
  number and closes nothing.

### Tests

- `the_traced_child_truncates_an_existing_virtual_file` and
  `the_traced_child_dup2_closes_the_real_destination` are the spawn tests the feature
  names. The second one gives the child a real pipe fd with `libc::pipe` (no
  `O_CLOEXEC`), has the shell `dup2` a virtual fd over it, and then checks
  `/proc/<child>/fd/<n>` while the child is still alive. The old dispatch leaves the fd in
  place; the new one removes it.
- `open_dispatch_truncates_an_existing_virtual_file`,
  `open_dispatch_reports_or_creates_a_missing_virtual_file`,
  `dup2_dispatch_closes_the_real_destination` and `dup3_dispatch_rejects_equal_fds` prove
  the dispatch without a traced child. The dispatcher reads the path with
  `process_vm_readv`; with the test's own pid it reads the test's memory, so the whole open
  path runs. On the old dispatch these tests fail: the open returns `SkipError(EEXIST)`,
  `O_TRUNC` leaves the old bytes in place, and dup2 returns `Skip` instead of closing the
  destination.
- The three feature-83 spawn tests are unchanged.

### Verification

The Linux CI image (`ewestudios/ewe-platform-ci:latest`, `--platform linux/amd64`,
`--profile uat`):

| Command | Result |
|---|---|
| `cargo fmt --all --check` (macOS host) | clean |
| `cargo test -p foundation_nativeapis --features vfs-native,vfs-ptrace --test ptrace_vfs -- --skip test_spawn --skip fork_child_first_stop --skip the_traced_child` | 21 passed, 0 failed; 13 ptrace tests filtered out |
| `cargo clippy -p foundation_nativeapis --all-targets --features vfs-native,vfs-ptrace --no-deps -- -D warnings` | fails on crate-wide debt (see below); the crate compiles |

- The `-D warnings` clippy run reports 611 warnings in the library and 610 in the library
  test. They are the crate-wide clippy debt recorded in backlog §35, and
  `ci/packages.toml` did not set `clippy-deny-warnings` for this package at the time.
  Feature 113 then cleared the remaining debt and turned the setting on. Feature 111
  also fixed the lints that the code it touched exposed: two module doc comments, four
  `similar_names` bindings, four `#[must_use]` constructors, one `match` to
  `let ... else`, one unneeded wildcard pattern, and the two new timestamp helpers. The
  rest stay with features 47/49.
- This Apple Silicon host runs the linux/amd64 image under OrbStack, where
  `PTRACE_GETREGS` returns `EIO` for every traced process. The 13 spawn tests, including
  the two new ones, fail by design here. The proof of the two new spawn tests is left to
  the GitHub `ubuntu-latest` x86_64 job: the `foundation_nativeapis` package job runs
  `vfs-native,vfs-ptrace` with `EWE_REQUIRE_PTRACE=1`.

### CI proof (2026-11-05)

GitHub run [42063296262](https://github.com/ewe-studios/ewe_platform/actions/runs/42063296262)
on `02cbb08b5`: the `foundation_nativeapis` job (id `127371617194`) passed on native
amd64 with `EWE_REQUIRE_IO_URING=1 EWE_REQUIRE_PTRACE=1`. Its log shows both new spawn
tests passing in the two test runs the job makes:

```text
test the_traced_child_truncates_an_existing_virtual_file ... ok
test the_traced_child_dup2_closes_the_real_destination ... ok
```

The two feature-83 spawn tests pass in the same runs. The whole run finished `success`:
113 jobs green, 0 failed, 3 skipped.

### Public API and version bump

`SyscallAction` gains the variant `CloseThenSkip` and the constructor
`close_then_skip`. Both are public (the module is public under
`target_os = "linux"` and `vfs-ptrace-nix`), and downstream code that matches the
enum exhaustively would stop compiling. `foundation_nativeapis` moves from 0.0.6 to
0.0.7, and every consumer requirement moves with it: `foundation_iogate` (2 entries),
`foundation_toolings`, `foundation_ai` (2 entries), `foundation_proxy` and
`foundation_wireguard`. No workspace consumer enables `vfs-ptrace-nix`, so the
behaviour of those crates does not change.
