---
module: foundation_macros
language: rust
status: active
last_updated: 2026-01-14
maintainer: ewe-platform team
related_specs: []
---

# foundation_macros - Documentation

## Overview
`foundation_macros` is a procedural macro crate that provides compile-time code generation for embedding files and directories into Rust binaries. It generates implementations of the embeddable traits from `foundation_nostd`, including automatic compression (gzip/brotli), hashing (SHA-256), ETag generation, MIME type detection, and timestamp extraction.

## Purpose and Responsibility
This crate serves as the compile-time asset embedding system for the ewe-platform, providing:
- Derive macros for embedding single files (`EmbedFileAs`)
- Derive macros for embedding entire directories (`EmbedDirectoryAs`)
- Automatic file compression (gzip or brotli)
- Compile-time hash generation (SHA-256)
- Automatic ETag generation (base85-encoded hash)
- MIME type detection based on file extensions
- Last-modified timestamp extraction
- Support for placeholder variables ($ROOT_CRATE, $CURRENT_CRATE, $CRATE)
- UTF-8 and optional UTF-16 encoding support
- Binary and text file support

## Module Location
- **Path**: `backends/foundation_macros/`
- **Entry Point**: `src/lib.rs`
- **Language**: Rust 2021 edition (proc-macro crate)
- **Package Manager**: Cargo
- **Version**: 0.0.4

## What It Implements

### Core Macros

#### 1. **EmbedFileAs** (Lines 50-108 in lib.rs)
- **What**: Procedural derive macro for embedding a single file
- **Why**: Allows compile-time inclusion of assets with metadata
- **How**: Reads file at compile-time, generates const arrays and impl blocks
- **Attributes**:
  - `#[source = "path"]`: File path to embed (required)
  - `#[gzip_compression]`: Apply gzip compression
  - `#[brottli_compression]`: Apply brotli compression
  - `#[with_utf16]`: Generate UTF-16 encoded version
  - `#[is_binary]`: Mark file as binary (no UTF-16 generation)
- **Placeholders**:
  - `$ROOT_CRATE`: Workspace root directory
  - `$CURRENT_CRATE`: Current crate directory
  - `$CRATE`: Alias for $CURRENT_CRATE
- **Generated Code**:
  - Static `&[u8]` arrays for UTF-8 and UTF-16 data
  - Implements `EmbeddableFile` trait from foundation_nostd
  - Implements `FileData` and `HasCompression` traits
  - Static `FileInfo` with metadata
- **Example**:
  ```rust
  #[derive(EmbedFileAs)]
  #[source = "$ROOT_CRATE/runtime/js/host.js"]
  #[gzip_compression]
  pub struct JSHostRuntime;
  ```

#### 2. **EmbedDirectoryAs** (Lines 41-76 in lib.rs)
- **What**: Procedural derive macro for embedding an entire directory
- **Why**: Allows batch embedding of multiple files with unified access
- **How**: Recursively reads directory, generates array of file data and metadata
- **Attributes**:
  - `#[source = "path"]`: Directory path to embed (required)
  - `#[gzip_compression]`: Apply gzip to all files
  - `#[brottli_compression]`: Apply brotli to all files
  - `#[with_utf16]`: Generate UTF-16 for all text files
- **Placeholders**: Same as EmbedFileAs
- **Generated Code**:
  - Static arrays of `FileInfo` metadata
  - Static arrays of file data tuples
  - Implements `EmbeddableDirectory` trait
  - Implements `DirectoryData` and `HasCompression` traits
  - Methods: `info_for()`, `info_under_directory()`, `request_utf8()`, `request_utf16()`
- **Example**:
  ```rust
  #[derive(EmbedDirectoryAs)]
  #[source = "$ROOT_CRATE/runtime/css"]
  #[brottli_compression]
  pub struct CSSAssets;
  ```

### Core Module

#### **embedders** (Line 3 in lib.rs)
- **What**: Implementation module for the proc macros
- **Why**: Separates macro entry points from implementation logic
- **How**: Parsing, file I/O, compression, hashing, code generation
- **Key Functions**:
  - `embed_directory_on_struct()`: Implementation for EmbedDirectoryAs
  - `embed_file_on_struct()`: Implementation for EmbedFileAs
  - `has_attr()`: Check for attribute presence
  - `get_attr()`: Extract attribute value
  - `find_root_cargo()`: Locate workspace root
  - `get_target_source_path()`: Resolve path placeholders
  - `impl_embeddable_directory()`: Generate directory embedding code
  - `impl_embeddable_file()`: Generate file embedding code
- **Key Error Types**:
  - `GenError::UnableToGetModifiedDate`
  - `GenError::UnableToGetMimeType`
  - `GenError::Any(Box<dyn error::Error>)`

## What It Imports

### Workspace Dependencies
- **foundation_nostd** (workspace): Uses embeddable types for code generation

### External Dependencies (Proc Macro Tools)
- **syn** (≥2.0): Parse Rust code and derive input
- **quote** (≥1.0): Generate Rust code as token streams
- **proc-macro2** (≥1.0): TokenStream manipulation
- **proc-macro-crate** (v3.3.0): Resolve crate names in proc macros

### External Dependencies (Asset Processing)
- **brotli** (v8.0.2): Brotli compression algorithm
- **flate2** (v1.1.5): Gzip/zlib compression
- **sha2** (v0.10.8): SHA-256 hashing
- **base85rs** (v0.1): Base85 encoding for ETags
- **chrono** (v0.4): Timestamp handling (std features)
- **new_mime_guess** (v4.0): MIME type detection from file extensions

## Public API

### Derive Macros
- **`#[derive(EmbedFileAs)]`**: Embed a single file with attributes
- **`#[derive(EmbedDirectoryAs)]`**: Embed a directory recursively

### Supported Attributes
- **`source`**: Required - file or directory path
- **`gzip_compression`**: Optional - apply gzip compression
- **`brottli_compression`**: Optional - apply brotli compression
- **`with_utf16`**: Optional - generate UTF-16 encoding
- **`is_binary`**: Optional - mark as binary file (file only)

### Path Placeholders
- **`$ROOT_CRATE`**: Replaced with workspace root directory path
- **`$CURRENT_CRATE`**: Replaced with current crate directory path
- **`$CRATE`**: Alias for `$CURRENT_CRATE`

### Generated Trait Implementations
- **`HasCompression`**: Returns compression type used
- **`FileData`**: Provides `read_utf8()` and `read_utf16()` methods
- **`EmbeddableFile`**: Provides `info()` method returning `&FileInfo`
- **`DirectoryData`**: Provides file access by index or path
- **`EmbeddableDirectory`**: Provides directory iteration and lookup

## Feature Flags

This crate has no feature flags - it provides a fixed proc macro interface.

## Architecture

### Design Patterns Used
- **Procedural Macro Pattern**: Compile-time code generation
- **Builder Pattern**: Accumulate file metadata before generation
- **Factory Pattern**: Create trait implementations from file data
- **Template Method**: Fixed generation algorithm with pluggable compression
- **Strategy Pattern**: Different compression algorithms (gzip, brotli, none)

### Module Structure
```
foundation_macros/
├── src/
│   ├── lib.rs                    # Public macro definitions (109 lines)
│   │   ├── #[proc_macro_derive(EmbedDirectoryAs, ...)]
│   │   ├── embed_directory_as() entry point
│   │   ├── #[proc_macro_derive(EmbedFileAs, ...)]
│   │   └── embed_file_as() entry point
│   └── embedders.rs              # Implementation (large file)
│       ├── GenError enum
│       ├── embed_directory_on_struct()
│       ├── embed_file_on_struct()
│       ├── has_attr() - attribute checking
│       ├── get_attr() - attribute extraction
│       ├── find_root_cargo() - workspace root discovery
│       ├── get_target_source_path() - placeholder resolution
│       ├── impl_embeddable_directory() - directory code gen
│       ├── impl_embeddable_file() - file code gen
│       ├── File I/O operations
│       ├── Compression logic (gzip, brotli)
│       ├── SHA-256 hashing
│       ├── Base85 encoding
│       ├── MIME type detection
│       └── TokenStream generation with quote!
└── Cargo.toml
```

## Key Implementation Details

### Compilation Process
1. **Macro Invocation**: User adds `#[derive(EmbedFileAs)]` to struct
2. **Parsing**: syn parses the derive input and attributes
3. **Path Resolution**: Resolve placeholders ($ROOT_CRATE, etc.)
4. **File I/O**: Read file contents at compile-time
5. **Compression**: Optionally compress with gzip or brotli
6. **Hashing**: Compute SHA-256 hash
7. **ETag Generation**: Encode hash with base85
8. **MIME Detection**: Guess MIME type from file extension
9. **Timestamp Extraction**: Get last-modified time
10. **Code Generation**: Generate const arrays and trait impls with quote!
11. **Token Stream**: Return generated code to compiler

### Compression Strategy
- **None**: Data stored as-is in static arrays
- **Gzip**: Uses flate2 with default compression level
- **Brotli**: Uses brotli with default compression level
- **Mutual Exclusion**: Cannot use both gzip and brotli (enforced with assert!)
- **Decompression**: Consumer must decompress at runtime

### Path Resolution Algorithm
1. Check for `$CURRENT_CRATE` or `$CRATE` in path
2. Replace with `CARGO_MANIFEST_DIR` environment variable
3. Check for `$ROOT_CRATE` in path
4. Traverse up directory tree to find workspace root (Cargo.toml without parent Cargo.toml)
5. Replace with workspace root path
6. Handle both `Cargo.toml` and `cargo.toml` (case-insensitive)

### Metadata Extraction
- **File Name**: Extract from path
- **File Path**: Absolute and relative-to-parent variants
- **Package Directory**: Crate directory
- **Hash**: SHA-256 digest of file contents (hex-encoded)
- **ETag**: Base85-encoded hash for HTTP caching
- **MIME Type**: Detected via new_mime_guess crate
- **Last Modified**: System time since Unix epoch
- **Is Directory**: Boolean flag

### Performance Considerations
- **Compile-Time Work**: All processing done at compile-time
- **Runtime Zero-Cost**: Embedded data is static, no runtime overhead
- **Compression Trade-off**: Smaller binary vs runtime decompression cost
- **Base85 Encoding**: Compact ETag representation

### Security Considerations
- **Compile-Time Safety**: No runtime file access
- **Hash Verification**: SHA-256 ensures data integrity
- **Path Validation**: Validates paths exist at compile-time
- **No Arbitrary Code**: Only generates safe trait implementations

### Code Generation Details
The generated code includes:
- Static byte arrays with `&'static [u8]` type
- Optional UTF-16 byte arrays for text files
- FileInfo struct with all metadata
- Trait implementations (HasCompression, FileData, EmbeddableFile/Directory)
- Efficient data structures (arrays, not collections)

## Dependencies and Relationships

### Depends On
- **foundation_nostd** (workspace): Defines embeddable traits
- **syn, quote, proc-macro2**: Proc macro infrastructure
- **brotli, flate2**: Compression algorithms
- **sha2, base85rs**: Hashing and encoding
- **chrono**: Timestamp handling
- **new_mime_guess**: MIME type detection

### Used By
- **foundation_wasm**: Uses macros to embed JS runtime files
- **foundation_runtimes**: Uses macros extensively for runtime assets
- **Any crate needing embedded assets**: Public API for asset embedding

### Sibling Modules
- **foundation_nostd**: Defines traits that this crate implements
- **foundation_core**: Higher-level framework built on this foundation
- **foundation_wasm**: Uses embedded files for WASM/JS runtime

## Configuration

### Feature Flags
None - proc macros provide a fixed interface.

### Environment Variables
- **`CARGO_MANIFEST_DIR`**: Used to resolve `$CURRENT_CRATE` placeholder
- Workspace root discovered by traversing filesystem

### Build Configuration
- **proc-macro = true**: Required in Cargo.toml for proc macro crates
- **Edition**: Rust 2021

## Known Issues and Limitations

### Current Limitations
1. **Compile-Time Overhead**: Large files/directories slow compilation
2. **Binary Size**: Embedded data increases binary size
3. **No Hot Reloading**: Changes require recompilation
4. **No Lazy Loading**: All data embedded in binary
5. **Compression Choice**: Cannot choose compression level
6. **No Filtering**: Cannot exclude files from directory embedding
7. **Case-Sensitive Paths**: Placeholder matching is case-sensitive
8. **No Streaming**: Entire file loaded into memory at compile-time

### Technical Debt
- **Limited Error Messages**: Panics with basic messages
- **No Path Normalization**: Relies on OS path handling
- **Hardcoded Placeholders**: Fixed set of placeholder strings
- **No Conditional Embedding**: Cannot conditionally embed based on features

### Known Issues
- **find_root_cargo()**: Recursive directory traversal may be slow on deep hierarchies
- **Panic on Missing Files**: No graceful error handling, just panics
- **No Validation**: Does not validate file types before processing
- **UTF-16 for Binary**: Attempting UTF-16 on binary files wastes space

## Future Improvements

### Planned Enhancements
- **Compression Level Control**: Attribute to specify compression level
- **File Filtering**: Glob patterns to include/exclude files
- **Better Error Messages**: Detailed error reporting with file context
- **Lazy Embedding**: Conditional embedding based on features
- **Directory Options**: Recursive depth control, symlink handling
- **Custom Hashing**: Support for other hash algorithms
- **Incremental Compilation**: Better caching for faster rebuilds

### Refactoring Opportunities
- **Error Handling**: Replace panics with proper error types
- **Path Normalization**: Canonical path handling
- **Modular Generation**: Split code generation into smaller functions
- **Test Coverage**: Add integration tests for macros
- **Documentation**: More examples in doc comments

## Testing

### Test Strategy
- **No Direct Tests**: Proc macro crates are hard to test directly
- **Integration Testing**: Tested via dependent crates (foundation_wasm, runtimes)
- **Manual Testing**: Verify generated code in expansion

### Testing Challenges
- Proc macros require special test infrastructure
- Generated code must be inspected manually
- Compilation failures are the primary feedback mechanism

## Related Documentation

### Specifications
- No specific specifications currently documented
- Macro behavior documented in doc comments

### External Resources
- [The Rust Procedural Macro Book](https://doc.rust-lang.org/reference/procedural-macros.html)
- [syn documentation](https://docs.rs/syn/)
- [quote documentation](https://docs.rs/quote/)
- [Brotli Compression](https://github.com/google/brotli)
- [Gzip Format](https://www.gnu.org/software/gzip/)

### Related Modules
- `documentation/foundation_nostd/doc.md` - Defines embeddable traits
- `documentation/foundation_wasm/doc.md` - Uses these macros
- `documentation/runtimes/doc.md` - Heavy user of these macros

## Version History

### [0.0.4] - Current
- EmbedFileAs and EmbedDirectoryAs derive macros
- Compression support (gzip, brotli)
- SHA-256 hashing and base85 ETags
- MIME type detection
- Placeholder support ($ROOT_CRATE, $CURRENT_CRATE, $CRATE)
- UTF-8 and UTF-16 encoding support
- Binary file support

---
*Last Updated: 2026-01-14*
*Documentation Version: 1.0*
