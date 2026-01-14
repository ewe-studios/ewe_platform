---
module: foundation_runtimes
language: rust
status: active
last_updated: 2026-01-14
maintainer: ewe-platform team
related_specs: []
---

# foundation_runtimes (runtimes) - Documentation

## Overview
`foundation_runtimes` is a central crate that embeds runtime assets (JavaScript files, CSS, HTML, etc.) as static data within the binary using compile-time embedding. It serves as the single source of truth for all runtime assets that need to be accessible to the ewe-platform at runtime, particularly for WebAssembly and browser-based applications.

## Purpose and Responsibility
This crate serves as the embedded asset repository for the ewe-platform, providing:
- Compile-time embedding of JavaScript runtime files
- Compile-time embedding of reloader/hot-reload utilities
- Static access to runtime assets without filesystem I/O
- Compressed asset storage (optional gzip or brotli)
- Asset metadata (hashes, ETags, MIME types, timestamps)
- No-std compatible asset access
- Single centralized location for all runtime assets

## Module Location
- **Path**: `backends/runtimes/`
- **Entry Point**: `src/lib.rs`
- **Language**: Rust 2021 edition
- **Package Manager**: Cargo
- **Version**: 0.0.3

## What It Implements

### Core Modules

#### 1. **js_runtimes** (Lines 3-13 in lib.rs)
- **What**: Module containing embedded JavaScript runtime assets
- **Why**: Provides JavaScript runtime code for WASM/browser interop
- **How**: Uses `EmbedDirectoryAs` macro to embed directories at compile-time
- **Key Structs**:
  - **`AssetReloader`**: Embeds `sdk/web/reloader` directory
    - Source: `$CRATE/../../sdk/web/reloader`
    - Purpose: Hot-reload functionality for development
    - Contents: `reloader.js` and related files
    - Implements: `EmbeddableDirectory`, `DirectoryData`, `HasCompression`
  - **`AssetHostRuntimes`**: Embeds `sdk/web/jsruntime` directory
    - Source: `$CRATE/../../sdk/web/jsruntime`
    - Purpose: Core JavaScript runtime for WASM host environment
    - Contents: `megatron.js` and related files
    - Implements: `EmbeddableDirectory`, `DirectoryData`, `HasCompression`

## What It Imports

### Workspace Dependencies
- **foundation_nostd** (workspace): Provides embeddable traits
- **foundation_macros** (workspace): Provides `EmbedDirectoryAs` macro

### External Dependencies
- **brotli** (v8.0.2): Brotli compression support (used by macros)
- **flate2** (v1.1.5): Gzip/zlib compression support (used by macros)

Note: Compression dependencies are declared but only used if compression attributes are specified in the embed macros.

## Public API

### Re-exported Macro
```rust
use foundation_macros::EmbedDirectoryAs;
```

### Public Module
```rust
pub mod js_runtimes {
    pub struct AssetReloader;      // Implements EmbeddableDirectory
    pub struct AssetHostRuntimes;  // Implements EmbeddableDirectory
}
```

### Available Methods (via EmbeddableDirectory trait)

#### AssetReloader
```rust
impl EmbeddableDirectory for AssetReloader {
    // Iterate over all files in reloader directory
    fn info_iter(&self) -> core::slice::Iter<'_, FileInfo>;

    // Get file info by path
    fn info_for(&self, source: &str) -> Option<FileInfo>;

    // Get files under a subdirectory
    fn info_under_directory(&self, directory: &str) -> Vec<FileInfo>;

    // Read UTF-8 file data
    fn request_utf8(&self, source: &str) -> Option<(Vec<u8>, Option<FileInfo>)>;

    // Read UTF-16 file data
    fn request_utf16(&self, source: &str) -> Option<(Vec<u8>, Option<FileInfo>)>;
}

impl DirectoryData for AssetReloader {
    const FILES_DATA: &'static [StaticDirectoryData];
    const FILES_METADATA: &'static [FileInfo];

    // Get UTF-8 data by index
    fn get_utf8_for(&self, index: usize) -> Option<Vec<u8>>;

    // Read UTF-8 data by path
    fn read_utf8_for(&self, source: &str) -> Option<Vec<u8>>;

    // Get UTF-16 data by index
    fn get_utf16_for(&self, index: usize) -> Option<Vec<u8>>;

    // Read UTF-16 data by path
    fn read_utf16_for(&self, source: &str) -> Option<Vec<u8>>;
}

impl HasCompression for AssetReloader {
    fn compression(&self) -> DataCompression;
}
```

#### AssetHostRuntimes
Implements the same trait methods as AssetReloader, but for the jsruntime directory.

### Usage Example
```rust
use foundation_runtimes::js_runtimes::{AssetReloader, AssetHostRuntimes};

// Access reloader assets
let reloader = AssetReloader::default();
if let Some((data, info)) = reloader.request_utf8("reloader.js") {
    let js_code = String::from_utf8(data).unwrap();
    println!("Reloader JS: {} bytes", js_code.len());
}

// Access host runtime assets
let host_runtime = AssetHostRuntimes::default();
if let Some((data, info)) = host_runtime.request_utf8("megatron.js") {
    let runtime_code = String::from_utf8(data).unwrap();
    println!("Runtime JS: {} bytes", runtime_code.len());
}

// Iterate over all reloader files
for file_info in reloader.info_iter() {
    println!("File: {}", file_info.source_name);
    println!("  Path: {}", file_info.source_path);
    println!("  Hash: {}", file_info.hash);
    println!("  MIME: {:?}", file_info.mime_type);
}
```

## Feature Flags

This crate has no feature flags - it provides a fixed set of embedded assets.

## Architecture

### Design Patterns Used
- **Repository Pattern**: Centralized asset storage
- **Facade Pattern**: Simple interface over complex embedding mechanism
- **Compile-Time Evaluation**: All embedding happens at compile-time
- **Static Data Pattern**: Assets stored as `&'static` data

### Module Structure
```
foundation_runtimes/
├── src/
│   └── lib.rs                    # Crate entry point (14 lines)
│       ├── use foundation_macros::EmbedDirectoryAs
│       └── pub mod js_runtimes
│           ├── AssetReloader (embeds sdk/web/reloader/)
│           └── AssetHostRuntimes (embeds sdk/web/jsruntime/)
├── Cargo.toml
└── Referenced Assets (not in crate):
    └── ../../sdk/web/
        ├── reloader/
        │   └── reloader.js         # Hot-reload functionality
        └── jsruntime/
            └── megatron.js          # Core JS runtime
```

## Key Implementation Details

### Compile-Time Embedding
1. **Build Time**: During compilation, `EmbedDirectoryAs` macro:
   - Resolves `$CRATE` placeholder to crate directory
   - Constructs absolute paths to `sdk/web/reloader` and `sdk/web/jsruntime`
   - Recursively reads all files in these directories
   - Generates static byte arrays with file contents
   - Generates FileInfo metadata (hash, ETag, MIME type, timestamps)
   - Generates trait implementations for data access

2. **Generated Code**: For each directory, generates:
   - `const FILES_DATA: &'static [StaticDirectoryData]`
   - `const FILES_METADATA: &'static [FileInfo]`
   - Implementations of HasCompression, DirectoryData, EmbeddableDirectory

3. **Runtime Access**: Applications access embedded data via trait methods:
   - Zero-cost abstraction - just pointer dereferencing
   - No file I/O at runtime
   - No dynamic allocation for file data (uses static slices)

### Asset Organization
- **Reloader Assets** (`AssetReloader`):
  - Development-time utilities
  - Hot-reload functionality
  - Browser refresh coordination
  - Typically used in development builds

- **Host Runtime Assets** (`AssetHostRuntimes`):
  - Core JavaScript runtime (`megatron.js`)
  - WASM-to-JS bridge functionality
  - Browser API wrappers
  - Memory management helpers
  - Used in both development and production

### Performance Considerations
- **Zero Runtime Cost**: All data is static, no allocation/deallocation
- **Compile-Time Overhead**: Embedding large files increases compile time
- **Binary Size**: Embedded assets increase binary size
- **No I/O Latency**: Instant access without filesystem operations
- **Cache-Friendly**: Static data is optimally cached by CPU

### Security Considerations
- **Immutable Assets**: Embedded data cannot be modified at runtime
- **Hash Verification**: SHA-256 hashes for integrity checking
- **Controlled Embedding**: Only explicitly embedded directories are included
- **No Path Traversal**: Assets are embedded, not accessed via filesystem

### Compression Strategy
Currently, no compression attributes are specified in the macros, meaning:
- **DataCompression::NONE**: Assets stored uncompressed
- **Trade-off**: Larger binary size but no decompression overhead at runtime
- **Future**: Can add `#[gzip_compression]` or `#[brottli_compression]` to reduce binary size

## Dependencies and Relationships

### Depends On
- **foundation_nostd** (workspace): Embeddable traits (HasCompression, DirectoryData, etc.)
- **foundation_macros** (workspace): `EmbedDirectoryAs` procedural macro
- **brotli** (v8.0.2): Compression support (if enabled)
- **flate2** (v1.1.5): Compression support (if enabled)

### Used By
- **Web-based applications**: Any application needing JS runtime or reloader
- **WASM modules**: Access embedded JS for browser interop
- **Development tools**: Hot-reload infrastructure

### Sibling Modules
- **foundation_macros**: Provides the embedding mechanism
- **foundation_nostd**: Provides trait definitions
- **foundation_wasm**: May use runtime assets for JS interop

## Configuration

### Feature Flags
None currently defined.

### Build Configuration
- **Edition**: Rust 2021
- **Asset Paths**: Hardcoded relative to crate location
  - `$CRATE/../../sdk/web/reloader`
  - `$CRATE/../../sdk/web/jsruntime`

### Compression Configuration
To enable compression, modify the struct attributes in `lib.rs`:
```rust
// Example: Add gzip compression
#[derive(EmbedDirectoryAs, Default)]
#[source = "$CRATE/../../sdk/web/reloader"]
#[gzip_compression]  // Add this line
pub struct AssetReloader;
```

## Known Issues and Limitations

### Current Limitations
1. **No Compression**: Assets embedded uncompressed (increases binary size)
2. **Static Asset List**: Cannot dynamically add/remove assets at runtime
3. **No Hot Reload of Embedded**: Changes to JS files require recompilation
4. **Fixed Paths**: Asset paths hardcoded, cannot be configured
5. **No Filtering**: All files in directories are embedded
6. **No UTF-16**: No `#[with_utf16]` attribute specified

### Technical Debt
- **Missing Compression**: Should evaluate gzip vs brotli for binary size reduction
- **No Documentation**: Structs lack doc comments explaining their purpose
- **Single Module**: Could organize assets into more granular modules
- **No Asset Registry**: No central list of available assets

## Future Improvements

### Planned Enhancements
- **Compression**: Add `#[gzip_compression]` or `#[brottli_compression]` to reduce binary size
- **More Asset Types**: Embed CSS, HTML, WASM modules, images, etc.
- **Configurable Paths**: Allow asset paths to be configured via features or env vars
- **Asset Registry**: Provide a registry of all available assets
- **Lazy Decompression**: Decompress on first access, cache results
- **Conditional Embedding**: Embed different assets based on build profiles

### Refactoring Opportunities
- **Documentation**: Add comprehensive doc comments with usage examples
- **Module Organization**: Split into development vs production asset modules
- **Versioning**: Track runtime asset versions
- **Testing**: Add tests verifying embedded assets can be accessed

## Testing

### Current Tests
None - this is a pure data crate with no logic to test.

### Future Testing Strategy
- **Integration Tests**: Verify assets can be accessed and have correct metadata
- **Size Tests**: Ensure binary size impact is reasonable
- **Compression Tests**: Verify compressed assets decompress correctly
- **Checksum Tests**: Validate embedded hashes match actual content

## Related Documentation

### Specifications
- No specific specifications currently documented
- Asset embedding behavior documented in foundation_macros

### External Resources
- [Foundation Macros Documentation](./foundation_macros/doc.md)
- [Foundation Nostd Documentation](./foundation_nostd/doc.md)

### Related Modules
- `documentation/foundation_macros/doc.md` - Embedding mechanism
- `documentation/foundation_nostd/doc.md` - Trait definitions
- `documentation/foundation_wasm/doc.md` - JS runtime consumer

## Embedded Assets

### AssetReloader (sdk/web/reloader/)
- **reloader.js**: Hot-reload functionality for development
- Purpose: Enables live reload of code changes without full page refresh
- Usage: Development builds only

### AssetHostRuntimes (sdk/web/jsruntime/)
- **megatron.js**: Core JavaScript runtime for WASM host environment
- Purpose: Provides JS bridge for WASM modules to interact with browser APIs
- Features:
  - WASM memory management
  - Function call marshalling
  - Event handling
  - DOM manipulation wrappers
  - Browser API abstractions
- Usage: Production and development builds

## Version History

### [0.0.3] - Current
- Central crate for runtime assets
- Embeds reloader directory (AssetReloader)
- Embeds jsruntime directory (AssetHostRuntimes)
- No compression enabled
- Uses EmbedDirectoryAs macro
- Provides EmbeddableDirectory interface

---
*Last Updated: 2026-01-14*
*Documentation Version: 1.0*
