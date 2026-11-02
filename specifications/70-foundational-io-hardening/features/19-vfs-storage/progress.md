# Feature 19 progress

_Written in ASD-STE100 Simplified Technical English._

**Abbreviations in this file.** virtual file system (VFS); continuous integration (CI).

## Landed

- **Linux CI gate fix: the `fuse_mount` example.** `examples/fuse_mount/main.rs` now
  imports `foundation_nativeapis::VfsFileSystem` and `FuseMountOptions`, and passes
  `FuseMountOptions::default()` to `FuseMount::new`. The example compiles in the
  `vfs-fuse` CI run.
