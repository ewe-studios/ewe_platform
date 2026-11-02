# Feature 19 learnings

_Written in ASD-STE100 Simplified Technical English._

**Abbreviations in this file.** virtual file system (VFS); continuous integration (CI);
Filesystem in Userspace (FUSE).

## Linux CI gate fixes

### The `fuse_mount` example did not compile

**Root cause.** `examples/fuse_mount/main.rs` called `write_file` and `mkdir_all` on
`MemoryFs` without the `VfsFileSystem` trait in scope. Rust finds these methods only
when the trait is imported. The same file called `FuseMount::new` with one argument.
The constructor takes two arguments: the filesystem and a `FuseMountOptions`.

**Fix.** Import `foundation_nativeapis::VfsFileSystem`. Import `FuseMountOptions` with
`FuseMount`. Pass `FuseMountOptions::default()` as the second argument.

### The `copy_vfs` test panicked on a second tracing subscriber

**Root cause.** `tests/copy_vfs/mod.rs` held one `#[test] #[traced_test]` test and one
`#[valtron_test]` test in one binary. `#[valtron_test]` installs the process tracing
subscriber with `try_init`, which ignores a second install. `#[traced_test]` calls
`set_global_default` and panics when a subscriber is already set. When the valtron test
ran first, the traced test panicked with `SetGlobalDefaultError`.

**Fix.** Remove `#[traced_test]` from the sync test and remove the
`use tracing_test::traced_test;` import. The valtron subscriber covers the process. The
sync test checks chunk sizes only; it emits no tracing events.

## Noticed (not fixed here)

- **`tests/fjall_vfs/mod.rs` mixes the same two attributes.** It has 28 sync tests with
  `#[traced_test]` and six async tests with `#[valtron_test]`. The sync tests are
  registered first, so the first sync test usually installs the subscriber before the
  async tests run. The CI has not failed on it. The same fix applies: drop
  `#[traced_test]` from the sync tests and the `tracing_test` import.
