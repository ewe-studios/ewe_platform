---
feature_name: "Scaffold Derive Macro"
description: "#[derive(Scaffold)] + #[scaffoldable] for automatic pub method forwarding, #[scaffold_impl] for manual trait delegation with #[scaffold_method] and #[scaffold_call] overrides. scaffold!() marker for delegated methods."
status: "pending"
priority: "medium"
phase: 3
created: 2026-06-05
updated: 2026-06-05
dependencies: []
tasks:
  completed: 0
  uncompleted: 21
  total: 21
  completion_percentage: 0%
---

# Feature 18: Scaffold Derive Macro

## Overview

A proc macro system for `foundation_macros` that eliminates boilerplate delegation code. Two pathways, one goal:

| Pathway | Use when | Listing required? |
|---------|----------|-------------------|
| **Automatic** — `#[scaffoldable]` + `#[derive(Scaffold)]` | You want ALL pub methods forwarded with zero listing | No |
| **Manual** — `#[scaffold_impl]` on impl block | You need trait impls, overrides, or selective delegation | Yes (use `scaffold!()` marker) |

Both pathways support `#[scaffold_call]` for custom access logic (Mutex guards, RefCell borrows, connection pools, etc.).

## The `scaffold!()` marker

Rust does not allow bodyless methods in `impl` blocks — only in `trait` definitions. To mark a method for delegation in `#[scaffold_impl]` blocks, use the `scaffold!()` macro as the method body:

```rust
#[scaffold_impl(via = "self.fs")]
impl VfsFileSystem for DirectoryDelta {
    // Delegated — scaffold!() is replaced with { self.fs.stat(path) }
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }

    // Override — real body, kept verbatim
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let inner = self.fs.open_directory(path)?;
        Ok(FilteredDirectory { inner })
    }
}
```

`scaffold!()` is a no-op declarative macro that compiles to `unreachable!()` if ever reached without the proc macro. The `#[scaffold_impl]` proc macro detects `{ scaffold!() }` bodies and replaces them with delegation code. Any other body is kept verbatim.

---

## The two pathways

### Pathway 1: Automatic — zero listing

Mark the inner type's impl block with `#[scaffoldable]`. Put `#[derive(Scaffold)]` on the outer struct. All pub methods appear automatically.

```rust
// Inner type — mark with #[scaffoldable]
#[scaffoldable]
impl TableInner {
    pub fn get_name(&self) -> String { "hello".to_string() }
    pub fn get_age(&self) -> u32 { 42 }
    pub fn set_name(&mut self, name: String) { /* ... */ }
    fn private_helper(&self) {} // ignored — not pub
}

// Outer type — derive Scaffold, point to field
#[derive(Scaffold)]
pub struct PublicTable {
    #[scaffold(field)]
    inner: Arc<TableInner>,
}

// Result: PublicTable automatically gets:
// pub fn get_name(&self) -> String { self.inner.get_name() }
// pub fn get_age(&self) -> u32 { self.inner.get_age() }
// pub fn set_name(&mut self, name: String) { self.inner.set_name(name) }
```

Multiple fields can be scaffolded:

```rust
#[derive(Scaffold)]
pub struct Gateway {
    #[scaffold(field)]
    auth: AuthService,
    #[scaffold(field)]
    db: DbService,
}
// Gets all pub methods from AuthService AND DbService
```

### Pathway 2: Manual — `#[scaffold_impl]` with full control

Write method signatures with `scaffold!()` bodies. Methods with real bodies are overrides. Supports `#[scaffold_method]` and `#[scaffold_call]` for fine-grained control.

```rust
#[scaffold_impl(via = "self.fs")]
impl VfsFileSystem for DirectoryDelta {
    type File = <NativeFs as VfsFileSystem>::File;
    type SeekableFile = <NativeFs as VfsFileSystem>::SeekableFile;
    type Directory = FilteredDirectory;

    // Override — real body, kept verbatim
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let inner = self.fs.open_directory(path)?;
        Ok(FilteredDirectory { inner })
    }

    // Scaffold — scaffold!() replaced with delegation
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }
    fn exists(&self, path: &str) -> VfsResult<bool> { scaffold!() }
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> { scaffold!() }
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> { scaffold!() }
    fn readlink(&self, path: &str) -> VfsResult<String> { scaffold!() }
    fn rename(&self, from: &str, to: &str) -> VfsResult<()> { scaffold!() }
    fn remove(&self, path: &str) -> VfsResult<()> { scaffold!() }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> { scaffold!() }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> { scaffold!() }
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> { scaffold!() }
    fn mkdir(&self, path: &str) -> VfsResult<()> { scaffold!() }
    fn capabilities(&self) -> VfsCapabilities { scaffold!() }
    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> { scaffold!() }
    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> { scaffold!() }
}
```

---

## Attribute reference

### `#[scaffoldable]` — on inner type's impl block

Marks an impl block so its pub methods can be auto-forwarded by `#[derive(Scaffold)]`.

```rust
#[scaffoldable]
impl MyService {
    pub fn process(&self, input: &[u8]) -> Vec<u8> { /* ... */ }
    pub fn status(&self) -> Status { /* ... */ }
    fn internal(&self) {} // skipped — not pub
}
```

**How it works**: Generates a hidden `macro_rules!` template encoding all pub method signatures. The template is named `__scaffold_methods_{TypeName}` and is invocable by `#[derive(Scaffold)]` to stamp out forwarding methods on any outer struct.

```rust
// Auto-generated by #[scaffoldable]
macro_rules! __scaffold_methods_MyService {
    ($self_access:expr) => {
        pub fn process(&self, input: &[u8]) -> Vec<u8> { $self_access.process(input) }
        pub fn status(&self) -> Status { $self_access.status() }
    };
}
```

### `#[derive(Scaffold)]` — on outer struct

Generates forwarding methods for all `#[scaffold(field)]`-annotated fields whose inner types have `#[scaffoldable]` impl blocks.

```rust
#[derive(Scaffold)]
pub struct Wrapper {
    #[scaffold(field)]
    inner: Arc<MyService>,
}
```

### `#[scaffold(field)]` — on struct field

Marks a field for automatic method forwarding. The macro detects the wrapper type and uses the appropriate access pattern.

### `#[scaffold_call(call = { ... })]` — on struct field (default call strategy)

Sets the default access expression for a field. Defines how the field is accessed before the method is called.

```rust
#[derive(Scaffold)]
pub struct LockedStore {
    #[scaffold(field)]
    #[scaffold_call(call = { self.data.lock().unwrap() })]
    data: Arc<Mutex<InMemoryStore>>,
}
```

The `call` block is a `{ ... }` token tree containing arbitrary multiline Rust code. The block's final expression is the target on which `.method_name(args)` gets called.

If no `#[scaffold_call]` is present, the macro uses **built-in presets** based on the detected wrapper type (see below).

### `#[scaffold_impl(via = "...")]` — on impl block

Manual pathway. Methods with `scaffold!()` bodies get delegation generated. Methods with real bodies are kept as overrides.

```rust
#[scaffold_impl(via = "self.fs")]
impl VfsFileSystem for DirectoryDelta {
    type File = NativeFile;
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }  // generated
    fn open(&self, path: &str) -> VfsResult<Self::File> {                  // kept
        custom_logic()
    }
}
```

The macro:
1. Walks the impl block items
2. Methods with `{ scaffold!() }` body → replaced with `{ <via_expr>.method_name(arg1, arg2, ...) }`
3. Methods with any other body → kept verbatim
4. Associated types, consts → kept verbatim

### `#[scaffold_method(via = "...")]` — on individual fn (inside `#[scaffold_impl]` or standalone)

Per-method opt-in delegation. Delegates THIS specific method to a specific field. Useful when different methods delegate to different fields. The method must have a `scaffold!()` body.

```rust
impl ComplexService for Gateway {
    #[scaffold_method(via = "self.auth")]
    fn authenticate(&self, token: &str) -> Result<User> { scaffold!() }

    #[scaffold_method(via = "self.db")]
    fn query(&self, sql: &str) -> Result<Rows> { scaffold!() }

    #[scaffold_method(via = "self.cache")]
    fn cached_get(&self, key: &str) -> Option<Value> { scaffold!() }

    fn health_check(&self) -> Status {
        // Custom: aggregate health from all inner services
        Status::ok()
    }
}
```

### `#[scaffold_call(call = { ... })]` — on individual fn (inside `#[scaffold_impl]`)

Per-method override of the access expression. The `call` block takes a `{ ... }` token tree — arbitrary multiline Rust code. The final expression in the block is the target. The method must have a `scaffold!()` body.

```rust
#[scaffold_impl(via = "self.inner")]
impl MixedAccess for MyType {
    // Uses block-level via = "self.inner"
    fn fast_read(&self) -> u64 { scaffold!() }

    // Override: this one needs a write lock
    #[scaffold_call(call = { self.inner.write().unwrap() })]
    fn slow_write(&mut self, val: u64) { scaffold!() }
}
```

Complex multiline example:

```rust
#[scaffold_impl(via = "self.pool")]
impl DbQueries for PooledDb {
    #[scaffold_call(call = {
        let conn = self.pool.get()
            .expect("connection pool exhausted");
        tracing::trace!("acquired db connection");
        conn
    })]
    fn find_user(&self, id: u64) -> Option<User> { scaffold!() }

    fn count_tables(&self) -> usize { scaffold!() }  // uses block-level via

    fn health(&self) -> bool {
        self.pool.status().is_healthy()
    }
}
```

---

## Built-in call presets

When the macro detects wrapper types (from the struct field type), it auto-selects the appropriate access pattern. These are used when no explicit `#[scaffold_call]` is present.

| Detected field type | Generated access for `&self` | Generated access for `&mut self` |
|---|---|---|
| `T` | `self.field` | `self.field` |
| `Box<T>` | `self.field` | `self.field` |
| `Arc<T>` | `self.field` | `self.field` |
| `Rc<T>` | `self.field` | `self.field` |
| `Mutex<T>` | `self.field.lock().unwrap()` | `self.field.lock().unwrap()` |
| `RwLock<T>` | `self.field.read().unwrap()` | `self.field.write().unwrap()` |
| `Arc<Mutex<T>>` | `self.field.lock().unwrap()` | `self.field.lock().unwrap()` |
| `Arc<RwLock<T>>` | `self.field.read().unwrap()` | `self.field.write().unwrap()` |
| `Rc<RefCell<T>>` | `self.field.borrow()` | `self.field.borrow_mut()` |
| `RefCell<T>` | `self.field.borrow()` | `self.field.borrow_mut()` |

Explicit `#[scaffold_call(call = { ... })]` always overrides presets.

---

## Precedence rules

When multiple sources of delegation exist, resolution order (highest wins):

**Inside `#[scaffold_impl]` blocks:**

1. **Method body is NOT `scaffold!()`** → kept verbatim, no delegation
2. **`#[scaffold_call(call = { ... })]` on the method** → use this call block
3. **`#[scaffold_method(via = "...")]` on the method** → use this via expression
4. **`#[scaffold_call(call = { ... })]` on the impl block** → use this call block
5. **`#[scaffold_impl(via = "...")]` on the impl block** → use this via expression
6. **`scaffold!()` body with no delegation target** → compile error

**For `#[derive(Scaffold)]` automatic pathway:**

1. **`#[scaffold_call(call = { ... })]` on the struct field** → use this call block for all methods
2. **Built-in preset** from wrapper type detection → use preset
3. **Plain field type** → `self.field`

---

## Complete examples

### Automatic pathway — zero listing

```rust
use foundation_macros::{scaffoldable, Scaffold};

#[scaffoldable]
impl InMemoryStore {
    pub fn get(&self, key: &str) -> Option<Vec<u8>> { /* ... */ }
    pub fn set(&self, key: &str, value: Vec<u8>) { /* ... */ }
    pub fn delete(&self, key: &str) -> bool { /* ... */ }
    pub fn keys(&self) -> Vec<String> { /* ... */ }
    pub fn clear(&mut self) { /* ... */ }
    fn compact_internal(&self) {} // not pub — skipped
}

#[derive(Scaffold)]
pub struct ThreadSafeStore {
    #[scaffold(field)]
    #[scaffold_call(call = { self.inner.lock().unwrap() })]
    inner: Arc<Mutex<InMemoryStore>>,
}

// ThreadSafeStore automatically gets:
// pub fn get(&self, key: &str) -> Option<Vec<u8>> { { self.inner.lock().unwrap() }.get(key) }
// pub fn set(&self, key: &str, value: Vec<u8>) { { self.inner.lock().unwrap() }.set(key, value) }
// pub fn delete(&self, key: &str) -> bool { { self.inner.lock().unwrap() }.delete(key) }
// pub fn keys(&self) -> Vec<String> { { self.inner.lock().unwrap() }.keys() }
// pub fn clear(&mut self) { { self.inner.lock().unwrap() }.clear() }
```

### Automatic with preset detection (no scaffold_call needed)

```rust
#[derive(Scaffold)]
pub struct SimpleWrapper {
    #[scaffold(field)]
    inner: Arc<TableInner>,  // Arc<T> preset: self.inner (deref)
}

#[derive(Scaffold)]
pub struct LockedWrapper {
    #[scaffold(field)]
    inner: Mutex<TableInner>,  // Mutex preset: self.inner.lock().unwrap()
}
```

### Manual pathway — trait impl with overrides

```rust
#[scaffold_impl(via = "self.fs")]
impl VfsFileSystem for DirectoryDelta {
    type File = <NativeFs as VfsFileSystem>::File;
    type SeekableFile = <NativeFs as VfsFileSystem>::SeekableFile;
    type Directory = FilteredDirectory;

    // Override — real body, kept verbatim
    fn open_directory(&self, path: &str) -> VfsResult<Self::Directory> {
        let inner = self.fs.open_directory(path)?;
        Ok(FilteredDirectory { inner })
    }

    // Scaffold — scaffold!() replaced with delegation
    fn stat(&self, path: &str) -> VfsResult<VfsMetadata> { scaffold!() }
    fn exists(&self, path: &str) -> VfsResult<bool> { scaffold!() }
    fn chmod(&self, path: &str, mode: u32) -> VfsResult<()> { scaffold!() }
    fn symlink(&self, target: &str, link: &str) -> VfsResult<()> { scaffold!() }
    fn readlink(&self, path: &str) -> VfsResult<String> { scaffold!() }
    fn rename(&self, from: &str, to: &str) -> VfsResult<()> { scaffold!() }
    fn remove(&self, path: &str) -> VfsResult<()> { scaffold!() }
    fn open(&self, path: &str, mode: OpenMode) -> VfsResult<Self::File> { scaffold!() }
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> { scaffold!() }
    fn create(&self, path: &str, mode: u32) -> VfsResult<Self::File> { scaffold!() }
    fn mkdir(&self, path: &str) -> VfsResult<()> { scaffold!() }
    fn capabilities(&self) -> VfsCapabilities { scaffold!() }
    fn read_file(&self, path: &str) -> VfsResult<Vec<u8>> { scaffold!() }
    fn write_file(&self, path: &str, data: &[u8]) -> VfsResult<()> { scaffold!() }
}
```

### Manual — scaffold_call block with multiline setup

```rust
#[scaffold_impl(via = "self.pool")]
impl DbQueries for PooledDb {
    #[scaffold_call(call = {
        let conn = self.pool.get().expect("pool exhausted");
        tracing::trace!("acquired connection");
        conn
    })]
    fn find_user(&self, id: u64) -> Option<User> { scaffold!() }

    #[scaffold_call(call = {
        let conn = self.pool.get().expect("pool exhausted");
        tracing::trace!("acquired connection");
        conn
    })]
    fn count_records(&self, table: &str) -> u64 { scaffold!() }

    fn health(&self) -> bool {
        self.pool.status().is_healthy()
    }
}
```

### Manual — mixed per-method delegation to different fields

```rust
impl ComplexService for Gateway {
    #[scaffold_method(via = "self.auth")]
    fn authenticate(&self, token: &str) -> Result<User> { scaffold!() }

    #[scaffold_method(via = "self.db")]
    fn query(&self, sql: &str) -> Result<Rows> { scaffold!() }

    #[scaffold_method(via = "self.cache")]
    fn cached_get(&self, key: &str) -> Option<Value> { scaffold!() }

    fn health_check(&self) -> Status {
        Status::ok()
    }
}
```

### Manual — scaffold_impl with block-level scaffold_call

```rust
#[scaffold_impl]
#[scaffold_call(field = "data", call = { self.data.lock().unwrap() })]
impl DataStore for LockedStore {
    fn get(&self, key: &str) -> Option<String> { scaffold!() }
    fn set(&self, key: &str, value: String) { scaffold!() }
    fn delete(&self, key: &str) -> bool { scaffold!() }
    fn keys(&self) -> Vec<String> { scaffold!() }

    fn clear(&self) {
        let mut guard = self.data.lock().unwrap();
        guard.clear();
        tracing::info!("store cleared");
    }
}
```

---

## Generated code

### From `via` expression

```rust
// Input
#[scaffold_impl(via = "self.fs")]
impl Trait for Wrapper {
    fn stat(&self, path: &str) -> Result<Meta> { scaffold!() }
}

// Generated
impl Trait for Wrapper {
    fn stat(&self, path: &str) -> Result<Meta> {
        self.fs.stat(path)
    }
}
```

### From `call` block

```rust
// Input
#[scaffold_impl]
#[scaffold_call(field = "data", call = { self.data.lock().unwrap() })]
impl Store for Locked {
    fn get(&self, key: &str) -> Option<String> { scaffold!() }
}

// Generated
impl Store for Locked {
    fn get(&self, key: &str) -> Option<String> {
        { self.data.lock().unwrap() }.get(key)
    }
}
```

### From multiline `call` block

```rust
// Input
#[scaffold_call(call = {
    let conn = self.pool.get().expect("pool exhausted");
    tracing::trace!("acquired connection");
    conn
})]
fn query(&self, sql: &str) -> Rows { scaffold!() }

// Generated
fn query(&self, sql: &str) -> Rows {
    {
        let conn = self.pool.get().expect("pool exhausted");
        tracing::trace!("acquired connection");
        conn
    }.query(sql)
}
```

### From `#[scaffoldable]` automatic pathway

```rust
// #[scaffoldable] on impl InMemoryStore generates:
macro_rules! __scaffold_methods_InMemoryStore {
    ($access:expr) => {
        pub fn get(&self, key: &str) -> Option<Vec<u8>> { $access.get(key) }
        pub fn set(&self, key: &str, value: Vec<u8>) { $access.set(key, value) }
    };
}

// #[derive(Scaffold)] on ThreadSafeStore invokes:
impl ThreadSafeStore {
    // Expanded from __scaffold_methods_InMemoryStore with
    // access = { self.inner.lock().unwrap() }
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        { self.inner.lock().unwrap() }.get(key)
    }
    pub fn set(&self, key: &str, value: Vec<u8>) {
        { self.inner.lock().unwrap() }.set(key, value)
    }
}
```

---

## The `scaffold!()` macro

Defined as a simple `macro_rules!` in `foundation_macros`:

```rust
/// Marker for methods that should be delegated by `#[scaffold_impl]`.
///
/// Compiles to `unreachable!()` if the proc macro hasn't processed it,
/// ensuring a clear panic instead of silent misbehavior.
#[macro_export]
macro_rules! scaffold {
    () => {
        unreachable!("scaffold!() was not processed by #[scaffold_impl] — did you forget the attribute on the impl block?")
    };
}
```

The `#[scaffold_impl]` proc macro detects method bodies that are exactly `{ scaffold!() }` (by token comparison) and replaces them. If a method body contains `scaffold!()` mixed with other code, it's kept verbatim (the `scaffold!()` call would panic at runtime, catching misuse).

---

## How `#[scaffoldable]` works (proc macro mechanics)

`#[scaffoldable]` is a `proc_macro_attribute` applied to an impl block. It:

1. Keeps the original impl block unchanged (pass-through)
2. Extracts all `pub fn` method signatures (name, receiver, params, return type)
3. Skips non-pub methods
4. Generates a companion `macro_rules!` macro named `__scaffold_methods_{TypeName}`
5. The generated macro accepts an `$access:expr` parameter and expands to forwarding methods

The `macro_rules!` macro is the bridge between `#[scaffoldable]` and `#[derive(Scaffold)]`. It encodes method signatures as tokens that the derive macro can stamp out with any access expression.

`#[derive(Scaffold)]` then:
1. Walks struct fields looking for `#[scaffold(field)]`
2. Determines the access expression (from `#[scaffold_call]`, preset, or plain `self.field`)
3. Invokes `__scaffold_methods_{InnerTypeName}!(access_expr)` in an `impl OuterStruct { ... }` block

---

## Tasks

### Research & Design
- [ ] Survey existing crates (`delegate`, `ambassador`, `enum_dispatch`) for prior art and edge cases
- [ ] Define exact error messages for invalid usage
- [ ] Validate `macro_rules!` bridge approach works across modules (same crate)

### `scaffold!()` marker macro
- [ ] Define `scaffold!()` macro_rules in foundation_macros
- [ ] Fallback body: `unreachable!()` with descriptive message
- [ ] Export via `#[macro_export]`

### `#[scaffoldable]` — companion macro
- [ ] Parse impl block: extract type name and all pub fn signatures
- [ ] Generate `macro_rules! __scaffold_methods_{TypeName}` with forwarding bodies
- [ ] Pass through original impl block unchanged
- [ ] Handle `&self`, `&mut self`, and `self` receivers
- [ ] Handle generic params and lifetimes on methods
- [ ] Wire into `foundation_macros/src/lib.rs` as `proc_macro_attribute`

### `#[derive(Scaffold)]` — struct derive
- [ ] Parse struct fields for `#[scaffold(field)]` annotations
- [ ] Parse optional `#[scaffold_call(call = { ... })]` on fields
- [ ] Detect wrapper type for built-in presets (Arc, Mutex, RwLock, RefCell, etc.)
- [ ] Resolve access expression: explicit call block > preset > plain `self.field`
- [ ] Invoke `__scaffold_methods_{TypeName}!` with resolved access expression
- [ ] Generate `impl OuterStruct { ... }` with forwarded methods
- [ ] Wire into `foundation_macros/src/lib.rs` as `proc_macro_derive`

### `#[scaffold_impl]` — manual impl block delegation
- [ ] Parse `#[scaffold_impl(via = "expr")]` attribute on impl blocks
- [ ] Detect `{ scaffold!() }` bodies via token comparison
- [ ] Walk impl block items: `scaffold!()` bodies get delegation, other bodies kept
- [ ] Parse `#[scaffold_method(via = "expr")]` on individual methods
- [ ] Parse `#[scaffold_call(call = { ... })]` on individual methods or impl block level
- [ ] Apply precedence rules (method-level > block-level)
- [ ] Extract argument names from signatures (skip self receiver)
- [ ] Generate delegation bodies: `via` → `expr.method(args)`, `call` → `{ block }.method(args)`
- [ ] Wire into `foundation_macros/src/lib.rs` as `proc_macro_attribute`

### Tests — automatic pathway
- [ ] `test_scaffoldable_generates_macro` — #[scaffoldable] produces __scaffold_methods macro
- [ ] `test_derive_scaffold_basic` — all pub methods forwarded automatically
- [ ] `test_derive_scaffold_arc` — Arc<T> field, deref access
- [ ] `test_derive_scaffold_mutex` — Mutex<T> field, preset lock access
- [ ] `test_derive_scaffold_explicit_call` — #[scaffold_call] on field overrides preset
- [ ] `test_derive_scaffold_multiple_fields` — two scaffolded fields, methods from both
- [ ] `test_scaffoldable_skips_private` — private methods not forwarded

### Tests — manual pathway
- [ ] `test_scaffold_impl_basic` — all scaffold!() methods delegated
- [ ] `test_scaffold_impl_override` — real body kept, scaffold!() delegated
- [ ] `test_scaffold_method_selective` — only tagged methods delegated
- [ ] `test_scaffold_call_block_mutex` — call block with lock().unwrap()
- [ ] `test_scaffold_call_block_multiline` — multiline call block with setup code
- [ ] `test_scaffold_call_per_method_override` — block-level via + per-method call override
- [ ] `test_mixed_via_sources` — different methods via scaffold_method to different fields

### Tests — error cases (trybuild)
- [ ] `trybuild_fail_scaffold_without_attribute` — scaffold!() without #[scaffold_impl] panics at runtime
- [ ] `trybuild_fail_no_scaffoldable` — compile error if inner type missing #[scaffoldable]
- [ ] `trybuild_fail_no_delegation_target` — compile error if scaffold!() method has no via/call

### Apply to VFS
- [ ] Refactor DirectoryDelta VfsFileSystem impl to use #[scaffold_impl]
- [ ] Refactor FilteredDirectory VfsDirectory impl to use #[scaffold_impl]

## Verification

- `cargo test -p foundation_macros` passes (all scaffold tests + trybuild)
- `cargo test -p foundation_nativeapis --features vfs-native` passes after refactor
- Trybuild tests verify compile errors for invalid usage
- Generated code is identical in behavior to hand-written delegation
- `scaffold!()` without proc macro panics with clear message


## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

_Created: 2026-06-05_
