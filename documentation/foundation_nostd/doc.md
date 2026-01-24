---
module: foundation_nostd
language: rust
status: active
last_updated: 2026-01-14
maintainer: ewe-platform team
related_specs: []
---

# foundation_nostd - Documentation

## Overview
`foundation_nostd` is a foundational no-std compatible crate that provides core primitives and utilities for the ewe-platform project. It serves as the base dependency for all no-std environments, offering synchronization primitives, embeddable file traits, and raw memory part utilities without requiring the Rust standard library.

## Purpose and Responsibility
This crate acts as the foundational layer for no-std environments in the ewe-platform ecosystem. Its primary responsibilities include:
- Providing no-std compatible synchronization primitives (via re-export of spin)
- Defining traits and types for embeddable files and directories
- Offering low-level memory utilities (RawParts for Vec decomposition)
- Serving as a minimal dependency for WASM and other no-std targets
- Establishing common abstractions used by all other foundation crates

## Module Location
- **Path**: `backends/foundation_nostd/`
- **Entry Point**: `src/lib.rs`
- **Language**: Rust 2021 edition (no_std)
- **Package Manager**: Cargo
- **Version**: 0.0.4

## What It Implements

### Core Modules

#### 1. **embeddable** (Line 5)
- **What**: Traits and types for embedding files and directories at compile-time
- **Why**: Enables static asset inclusion in no-std environments (WASM, embedded systems)
- **How**: Trait definitions with associated constants and methods for file access
- **Key Traits**:
  - `HasCompression`: Indicates compression method used for data
  - `FileData`: Read UTF8/UTF16 data from embedded files
  - `EmbeddableFile`: Complete file interface with metadata
  - `DirectoryData`: Access files within embedded directories by index or path
  - `EmbeddableDirectory`: Complete directory interface with file iteration
- **Key Types**:
  - `DataCompression`: Enum (NONE, GZIP, BROTTLI)
  - `FileInfo`: Static file metadata (path, name, hash, etag, mime type, modified date)
  - `OwnedFileInfo`: Owned version with String fields
  - `DirectoryInfo`: Directory metadata
  - `FsInfo`: Enum (Dir or File)
  - `StaticDirectoryData`: Tuple type for embedded directory data
- **Key Features**:
  - Zero-allocation file access (static data)
  - Compression support (gzip, brotli)
  - Both UTF-8 and UTF-16 data support
  - Mime type detection
  - File hashing (SHA-256)
  - ETags for caching
  - Last-modified timestamps

#### 2. **spin (re-export)** (Line 6)
- **What**: Re-export of spin crate synchronization primitives
- **Why**: Provides no-std compatible locks for multi-threaded access
- **How**: Public re-export via `pub use spin;`
- **Key Types** (from spin v0.10):
  - `Mutex`: Spinlock-based mutual exclusion
  - `RwLock`: Readers-writer lock
  - `SpinMutex`: Explicit spinlock mutex
- **Features Used**: `rwlock`, `mutex`, `spin_mutex`
- **Note**: These are spinlocks, not OS-backed locks - suitable for short critical sections

#### 3. **raw_parts** (Line 7)
- **What**: Utility for decomposing and reconstructing Vec<T>
- **Why**: Safe abstraction over Vec::from_raw_parts for FFI and memory operations
- **How**: Wrapper struct with named fields (ptr, length, capacity)
- **Key Type**:
  - `RawParts<T>`: Struct with ptr (*mut T), length (u64), capacity (u64)
- **Key Methods**:
  - `from_vec(vec: Vec<T>) -> Self`: Decompose vector
  - `unsafe fn into_vec(self) -> Vec<T>`: Reconstruct vector
- **Source**: Adapted from https://github.com/artichoke/raw-parts (MIT license)
- **Benefits**:
  - Named fields prevent confusing length/capacity
  - Safer than raw Vec::from_raw_parts usage
  - Useful for FFI boundaries
  - Tested with roundtrip verification

## What It Imports

### External Dependencies
- **spin** (v0.10): Spinlock-based synchronization primitives
  - Features: `rwlock`, `mutex`, `spin_mutex`
  - Default features disabled for minimal footprint

### Workspace Dependencies
None - this is the most foundational crate with minimal external dependencies.

## Public API

### Re-exported Items
```rust
pub mod embeddable;  // File/directory embedding traits
pub use spin;        // Synchronization primitives
pub mod raw_parts;   // Vec decomposition utilities
```

### Key Public Items

#### From `embeddable` Module
- **Traits**:
  - `HasCompression`: Query compression type
  - `FileData: HasCompression`: Read file data (UTF8/UTF16)
  - `EmbeddableFile: FileData`: Complete file interface
  - `DirectoryData: HasCompression`: Directory file access
  - `EmbeddableDirectory: DirectoryData`: Complete directory interface
- **Types**:
  - `DataCompression`: NONE | GZIP | BROTTLI
  - `FileInfo`: Static file metadata (&'static str fields)
  - `OwnedFileInfo`: Owned file metadata (String fields)
  - `DirectoryInfo`: Directory metadata
  - `FsInfo`: Dir(DirectoryInfo) | File(OwnedFileInfo)
  - `StaticDirectoryData`: (usize, &'static str, &'static str, &'static [u8], Option<&'static [u8]>)

#### From `spin` Re-export
- `Mutex<T>`: Spinlock mutex
- `RwLock<T>`: Readers-writer lock
- `SpinMutex<T>`: Explicit spinlock

#### From `raw_parts` Module
- `RawParts<T>`: Vec decomposition wrapper
  - Fields: `ptr: *mut T`, `length: u64`, `capacity: u64`
  - Methods: `from_vec()`, `into_vec()` (unsafe)
  - Implements: From<Vec<T>>, Debug, PartialEq, Eq, Hash

## Feature Flags

This crate has no feature flags - it provides a minimal, stable interface.

## Architecture

### Design Patterns Used
- **Trait-Based Abstractions**: Embeddable* traits define interfaces without implementation
- **Zero-Cost Abstractions**: All embeddable data is static, no runtime cost
- **Associated Types/Constants**: `EmbeddableDirectory` uses associated constants for data
- **Newtype Pattern**: `RawParts<T>` wraps raw parts with meaningful names
- **Re-export Pattern**: Exposes spin crate primitives without duplication

### Module Structure
```
foundation_nostd/
├── src/
│   ├── lib.rs                    # Crate root (no_std)
│   │   ├── pub mod embeddable
│   │   ├── pub use spin
│   │   └── pub mod raw_parts
│   ├── embeddable.rs             # File/directory embedding (331 lines)
│   │   ├── DataCompression enum
│   │   ├── FsInfo enum
│   │   ├── DirectoryInfo struct
│   │   ├── OwnedFileInfo struct
│   │   ├── FileInfo struct
│   │   ├── HasCompression trait
│   │   ├── FileData trait
│   │   ├── EmbeddableFile trait
│   │   ├── StaticDirectoryData type alias
│   │   ├── DirectoryData trait
│   │   └── EmbeddableDirectory trait
│   └── raw_parts.rs              # Vec utilities (279 lines)
│       ├── RawParts<T> struct
│       ├── From<Vec<T>> impl
│       ├── Debug, PartialEq, Eq, Hash impls
│       ├── from_vec() method
│       ├── into_vec() unsafe method
│       └── roundtrip test
└── Cargo.toml
```

## Key Implementation Details

### Performance Considerations
- **Zero-Cost Embeddable**: All embedded data is `&'static` - no runtime allocation
- **Spinlocks**: Efficient for short critical sections, no syscalls
- **Inline Functions**: Most functions are small and #[must_use] annotated
- **Static Dispatch**: Trait-based with no dynamic dispatch overhead

### Security Considerations
- **Unsafe Isolation**: `into_vec()` is marked unsafe, forcing explicit acknowledgment
- **Type Safety**: Strong typing prevents confusion (e.g., length vs capacity in RawParts)
- **Immutable Defaults**: File data is static and immutable

### Concurrency/Async Handling
- **Spinlocks**: spin::Mutex and RwLock for concurrent access
- **No Async**: Pure synchronous primitives (async is in foundation_core)
- **No-std Compatible**: Works in single-threaded and multi-threaded contexts

### Platform Compatibility
- **No-std**: Core feature - works in any Rust environment
- **No Allocator Required**: Except for owned types (OwnedFileInfo)
- **Extern Alloc**: Uses `extern crate alloc` for Vec/String in embeddable module
- **WASM Compatible**: Designed for WASM and embedded systems
- **Cross-Platform**: No OS-specific code

### Embeddable System Design
The embeddable trait system is designed for use with proc macros (foundation_macros):
1. **Compile-Time Embedding**: Files are read and embedded during compilation
2. **Static Storage**: Data stored as `&'static [u8]` arrays
3. **Metadata Generation**: Hash, ETag, MIME type computed at compile-time
4. **Compression**: Optional gzip or brotli compression
5. **UTF-16 Support**: Both UTF-8 and UTF-16 encodings for JS interop
6. **Path Flexibility**: Access files by full path or relative-to-parent path

## Tests

### Test Coverage
- **raw_parts Module**: `roundtrip` test (lines 264-277)
  - Verifies Vec → RawParts → Vec conversion
  - Tests capacity preservation
  - Tests length preservation
  - Tests pointer identity

### Testing Strategy
- Unit tests for critical functionality
- Tests use `extern crate alloc` for std types
- Focus on correctness of unsafe operations
- Roundtrip tests for bidirectional conversions

### Missing Tests
- No tests for embeddable traits (tested indirectly via foundation_macros)
- No concurrency tests for spin primitives (delegated to spin crate)

## Dependencies and Relationships

### Depends On
- **spin** (v0.10): External crate for synchronization primitives
  - Features: `rwlock`, `mutex`, `spin_mutex`
  - No default features

### Used By
- **foundation_wasm**: Uses spin locks and embeddable traits
- **foundation_core**: Uses spin locks for compatibility layer
- **foundation_macros**: Generates code implementing embeddable traits
- **foundation_ai**: Depends on this crate
- **All other foundation crates**: Transitive dependency

### Sibling Modules
- **foundation_wasm**: WASM-specific implementations
- **foundation_core**: Higher-level abstractions
- **foundation_macros**: Compile-time code generation

## Configuration

### Feature Flags
None - this crate provides a fixed, minimal interface.

### Build Configuration
- **no_std**: Crate attribute `#![no_std]`
- **extern crate alloc**: For Vec/String support
- **Edition**: Rust 2021

### Dependency Configuration
- **spin**: Custom feature selection (`rwlock`, `mutex`, `spin_mutex`)
- **spin**: `default-features = false` for minimal footprint

## Known Issues and Limitations

### Current Limitations
1. **No-std Only**: Cannot use std library features (by design)
2. **Static Data Only**: Embeddable system requires compile-time data
3. **Spinlocks**: Not ideal for long-held locks or contended scenarios
4. **No Async Locks**: Only synchronous primitives available
5. **Limited Error Handling**: Minimal error types (by design)

### Technical Debt
- **Sparse Documentation**: Some trait methods lack detailed examples
- **No Integration Tests**: Only unit tests present
- **External Code**: raw_parts.rs is adapted from external source (MIT licensed)

### Known Issues
- **RawParts u64 Size**: Uses u64 for length/capacity even on 32-bit systems (may waste space)
- **Panic on Overflow**: `into_vec()` panics if length/capacity don't fit in usize

## Future Improvements

### Planned Enhancements
- **Additional Examples**: More usage examples for embeddable traits
- **Error Types**: More structured error handling for embeddable operations
- **Async Spin Locks**: Async-aware locks when needed
- **const fn Expansion**: More const-evaluable functions

### Refactoring Opportunities
- **RawParts Optimization**: Use usize instead of u64 for native platform size
- **Trait Documentation**: Expand trait documentation with examples
- **Test Coverage**: Add integration tests for embeddable system

## Related Documentation

### Specifications
- No specific specifications currently documented
- Embeddable system design is implicit in trait definitions

### External Resources
- [Rust no_std Book](https://docs.rust-embedded.org/book/intro/no-std.html)
- [spin crate documentation](https://docs.rs/spin/)
- [Artichoke raw-parts source](https://github.com/artichoke/raw-parts)

### Related Modules
- `documentation/foundation_wasm/doc.md`
- `documentation/foundation_core/doc.md`
- `documentation/foundation_macros/doc.md`

## Version History

### [0.0.4] - Current
- Foundational no-std core implementation
- Spin lock primitives (Mutex, RwLock, SpinMutex)
- Embeddable file/directory trait system
- RawParts utility for Vec manipulation
- Zero-dependency core (except spin crate)

---
*Last Updated: 2026-01-14*
*Documentation Version: 1.0*
