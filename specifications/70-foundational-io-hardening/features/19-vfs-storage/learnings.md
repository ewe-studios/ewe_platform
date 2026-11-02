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
