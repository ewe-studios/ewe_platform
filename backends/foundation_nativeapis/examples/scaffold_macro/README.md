# Example: Scaffold Macro

## Purpose

Demonstrates the `#[scaffold_impl]` procedural macro and `scaffold!()` delegation pattern for implementing the `VfsFileSystem` trait with minimal boilerplate. This example shows how to create a decorator filesystem that overrides only specific methods (e.g., for logging or auditing) while delegating everything else to an inner implementation via generated code.

## Prerequisites

- Rust toolchain with workspace dependencies
- Feature flag: `vfs-native` (for `MemoryFs` and macro support)

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs-native --example scaffold_macro
```

## Architecture

The example defines `LoggingFs`, a wrapper around `MemoryFs` that adds audit logging to `create()` and `mkdir()` operations. Instead of manually implementing all 20+ methods of `VfsFileSystem`, it uses:

1. **`#[scaffold_impl(via = "self.inner")]`** — A proc macro attribute that generates delegation code for all trait methods not explicitly overridden. The `via` clause specifies the field to delegate to.

2. **`scaffold!()` macro calls** — Inside the impl block, calling `scaffold!()` expands to a delegation statement like `self.inner.method_name(args)`. This makes the delegation explicit and auditable.

3. **Selective overriding** — Only `create()` and `mkdir()` have custom logic (printing a log line before delegating). Everything else uses `scaffold!()` for pass-through.

The pattern enables rapid development of VFS decorators (logging, caching, metrics, access control) without repetitive boilerplate.

## Expected Output

```
=== scaffold!() Macro Example ===

Demonstrates trait delegation via #[scaffold_impl] + scaffold!()
Only create() and mkdir() are overridden; everything else delegates.

[audit] mkdir "/data" mode=0o755
[audit] create "/data/report.csv" mode=0o644

Read back: "name,age\nalice,30\n"
stat: size=17, type=File

=== Done ===
```

## Key APIs Demonstrated

- `#[scaffold_impl(via = "self.inner")]` — Proc macro for automatic trait delegation
- `scaffold!()` — Expands to a delegation call on the inner field
- `VfsFileSystem::create(path, mode)` — Overridden with logging
- `VfsFileSystem::mkdir(path)` — Overridden with logging
- `VfsFileSystem::write_file(path, data)` — Delegated via `scaffold!()`
- `VfsFileSystem::read_file(path)` — Delegated via `scaffold!()`
- `VfsFileSystem::stat(path)` — Delegated via `scaffold!()`

## Where to Use This

- **Audit logging** — Log every filesystem operation for compliance debugging
- **Metrics collection** — Track latency, error rates, and throughput per operation
- **Caching layers** — Intercept `read()` calls to serve from a cache before delegating
- **Access control** — Add permission checks before delegating to the underlying FS
- **Mocking** — Create test doubles that record calls while delegating to a real backend
- **Rapid prototyping** — Experiment with VFS extensions without implementing the full trait

## Implementation Pattern

```rust
struct LoggingFs {
    inner: MemoryFs,
    label: String,
}

#[scaffold_impl(via = "self.inner")]
impl VfsFileSystem for LoggingFs {
    type File = <MemoryFs as VfsFileSystem>::File;
    type SeekableFile = <MemoryFs as VfsFileSystem>::SeekableFile;
    type Directory = <MemoryFs as VfsFileSystem>::Directory;

    // Override specific methods
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> {
        println!("[{}] create {:?} mode={:o}", self.label, path, mode);
        self.inner.create(path, mode)  // or: scaffold!()
    }

    // Delegate everything else
    fn capabilities(&self) -> VfsCapabilities { scaffold!() }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }
    // ... remaining methods auto-generated
}
```

## Related

- Feature spec: `specifications/37-overlay-vfs/features/05-scaffold-macro/feature.md`
- Source: `src/shared/vfs/scaffold.rs` (macro definitions)
- Trait definition: `src/shared/vfs/traits.rs` (`VfsFileSystem`)
- Proc macro crate: `foundation_macros/src/scaffold_impl.rs`
