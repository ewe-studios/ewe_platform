---
feature: "Body Types"
description: "Body type with Single (Bytes/Text) enum, JS value conversion via MemoryId transfer, From impls for common types, clonable body support"
status: "pending"
priority: "high"
depends_on: ["error-types"]
estimated_effort: "medium"
created: 2026-05-18
last_updated: 2026-05-18
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Body Types

## Overview

This feature implements the `Body` type — the abstraction for HTTP request bodies. It mirrors reqwest's `body.rs` architecture but adapted for the foundation_wasm ABI: instead of converting to `JsValue` (Uint8Array, JsString), bodies are written to shared memory and the `MemoryId` is passed across the ABI boundary.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | Body type, enums, From impls, memory transfer | `.agents/skills/rust-clean-code/skill.md` |

## Architecture (COMPREHENSIVE)

### File Structure

- `backends/foundation_wasm/src/http/body.rs` — Complete implementation

### Type Hierarchy

```rust
pub struct Body {
    inner: Inner,
}

enum Inner {
    Single(Single),
}

/// Crate-private. A fully-materialized body.
enum Single {
    /// Raw bytes (binary data, JSON, etc.)
    Bytes(Bytes),
    /// UTF-8 text (form data, plain text)
    Text(alloc::borrow::Cow<'static, str>),
}
```

**Note**: `MultipartForm` variant is added in feature 08 (multipart). This feature only implements `Single`.

### Key Methods

```rust
impl Body {
    /// Returns the body content as bytes, if it's a single body.
    /// Returns None for multipart bodies.
    pub fn as_bytes(&self) -> Option<&[u8]>;

    /// Writes the body to shared memory and returns the MemoryId.
    /// This is the primary way bodies cross the ABI boundary.
    pub(crate) fn write_to_memory(&self) -> WasmRequestResult<MemoryId>;

    /// Returns true if the body is empty.
    pub fn is_empty(&self) -> bool;

    /// Attempts to clone the body. Returns None if the body is
    /// not clonable (e.g., already consumed).
    pub fn try_clone(&self) -> Option<Body>;

    /// Returns the content length of the body, if known.
    pub fn content_length(&self) -> Option<u64>;
}
```

### From Implementations

```rust
impl From<Vec<u8>> for Body
impl From<&'static [u8]> for Body
impl From<String> for Body
impl From<&'static str> for Body
impl From<Bytes> for Body
```

These all map to `Inner::Single(Single::Bytes(...))` or `Inner::Single(Single::Text(...))` depending on input type.

### Memory Transfer (`write_to_memory`)

The core ABI transfer function:

```rust
impl Body {
    pub(crate) fn write_to_memory(&self) -> WasmRequestResult<MemoryId> {
        match &self.inner {
            Inner::Single(single) => match single {
                Single::Bytes(bytes) => {
                    // Allocate memory via ABI
                    let mem_id = internal_api::create_allocation(bytes.len() as u64, 0)?;
                    // Write bytes to shared memory
                    let alloc = internal_api::get_memory(mem_id);
                    // Copy bytes into alloc's buffer
                    alloc.copy_from_slice(bytes);
                    Ok(mem_id)
                }
                Single::Text(text) => {
                    let bytes = text.as_bytes();
                    // Same as Bytes path
                    let mem_id = internal_api::create_allocation(bytes.len() as u64, 0)?;
                    let alloc = internal_api::get_memory(mem_id);
                    alloc.copy_from_slice(bytes);
                    Ok(mem_id)
                }
            },
        }
    }
}
```

### Component Details

#### `Bytes` Type

Since this is `no_std`, we can't use `bytes::Bytes` from the `bytes` crate (which requires `std`). We have two options:

1. **Use `alloc::vec::Vec<u8>`** directly — simple, owns the data
2. **Define a thin wrapper** `struct Bytes(Vec<u8>)` for semantic clarity

Option 2 is preferred for API compatibility with the reqwest pattern and future `bytes` crate integration if `std` is available.

```rust
/// Owned bytes, equivalent to bytes::Bytes when std is available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bytes(alloc::vec::Vec<u8>);

impl Bytes {
    pub fn new() -> Self { Bytes(Vec::new()) }
    pub fn len(&self) -> usize { self.0.len() }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
    pub fn as_slice(&self) -> &[u8] { &self.0 }
}

impl Default for Bytes {
    fn default() -> Self { Self::new() }
}

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Self { Bytes(v) }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] { &self.0 }
}
```

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| `Bytes` wraps `Vec<u8>` | No_std compatible, simple | Pull in `bytes` crate — may not compile for wasm32 without std |
| `Cow<'static, str>` for text | Zero-copy for `&'static str`, alloc for `String` | Always convert to `Vec<u8>` — wasteful for static strings |
| `write_to_memory` as Body method | Encapsulates the ABI transfer logic | Caller handles memory — error-prone, leaks allocation responsibility |

## Implementation

### Files to Create/Modify

- `backends/foundation_wasm/src/http/body.rs` — New file: Body, Bytes, Single, From impls

### Tasks

- [ ] T1: Create `Bytes` wrapper around `Vec<u8>` with standard methods
- [ ] T2: Create `Single` enum with `Bytes` and `Text(Cow<'static, str>)` variants
- [ ] T3: Create `Inner` enum with `Single` variant (multipart added later)
- [ ] T4: Create `Body` struct with `as_bytes()`, `is_empty()`, `try_clone()`, `content_length()`
- [ ] T5: Implement `write_to_memory()` for ABI transfer
- [ ] T6: Implement `From<Vec<u8>>`, `From<&'static [u8]>`, `From<String>`, `From<&'static str>`, `From<Bytes>` for Body
- [ ] T7: Implement `Default` for Body (empty body)
- [ ] T8: Export types from `http/mod.rs`

## Testing

### Test Cases

1. **From impls**: `Body::from("hello")` creates `Single::Text`, `Body::from(vec![1,2,3])` creates `Single::Bytes`
2. **as_bytes**: Returns correct slice for both Bytes and Text variants
3. **is_empty**: Returns true for empty body, false for non-empty
4. **try_clone**: Returns Some with identical content for a clonable body
5. **content_length**: Returns correct length for Bytes and Text bodies
6. **write_to_memory**: Body bytes correctly written to shared memory, MemoryId returned
7. **Default**: Default body is empty

## Success Criteria

- [ ] All tasks completed
- [ ] Body type correctly wraps bytes and text
- [ ] All From impls compile and produce correct variants
- [ ] write_to_memory correctly transfers body to shared memory
- [ ] No regressions on native target
- [ ] try_clone works correctly for single bodies

---

_Created: 2026-05-18_
