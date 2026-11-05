---
feature: "The ptrace VFS follows `open(2)` truncation and `dup2` close rules"
status: "pending"
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

- [ ] The two new spawn tests fail on the old dispatch and pass on the new one.
- [ ] The two backlog §5 entries are deleted.
- [ ] The `vfs-native,vfs-ptrace` CI run passes on native amd64.
- [ ] CI green.
