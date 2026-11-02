# Feature 19 progress

_Written in ASD-STE100 Simplified Technical English._

**Abbreviations in this file.** virtual file system (VFS); continuous integration (CI).

## Landed

- **Linux CI gate fix: the `fuse_mount` example.** `examples/fuse_mount/main.rs` now
  imports `foundation_nativeapis::VfsFileSystem` and `FuseMountOptions`, and passes
  `FuseMountOptions::default()` to `FuseMount::new`. The example compiles in the
  `vfs-fuse` CI run.
- **Linux CI gate fix: the `copy_vfs` test.** `tests/copy_vfs/mod.rs` no longer mixes
  `#[traced_test]` with `#[valtron_test]` in one binary. The sync test keeps `#[test]`
  only. Both tests pass in the all-VFS CI run.

## Next

- Drop `#[traced_test]` from `tests/fjall_vfs/mod.rs` (28 sync tests) for the same
  reason as the `copy_vfs` fix. See learnings.md.
