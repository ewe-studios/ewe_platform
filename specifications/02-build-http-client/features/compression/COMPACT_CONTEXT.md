# COMPACT_CONTEXT: Compression Feature

> **CRITICAL**: This file contains ALL context needed. Main context SHOULD BE CLEARED after reading this.

---

## Current Task: Implement HTTP Compression/Decompression

**Feature**: compression (14 tasks, 0% complete)
**Status**: Ready to start
**Priority**: Medium
**Progress**: 7/13 features complete (54%), compression is next

---

## Feature Overview

Add automatic compression/decompression for HTTP bodies. Support gzip, deflate, brotli with transparent handling via content negotiation.

**Dependencies Met**:
- ✅ foundation complete (provides HttpClientError)

**Blocks**:
- public-api (needs compression configuration API)

---

## Implementation Requirements

### 1. ContentEncoding Enum

```rust
pub enum ContentEncoding {
    Identity,    // No compression
    Gzip,        // gzip (flate2)
    Deflate,     // deflate (flate2)
    Brotli,      // br (brotli crate)
    Unknown(String),
}

impl ContentEncoding {
    pub fn from_header(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "gzip" => Self::Gzip,
            "deflate" => Self::Deflate,
            "br" => Self::Brotli,
            "identity" => Self::Identity,
            other => Self::Unknown(other.to_string()),
        }
    }
}
```

### 2. DecompressingReader (Streaming)

```rust
pub struct DecompressingReader<R: Read> {
    inner: DecompressorKind<R>,
}

enum DecompressorKind<R: Read> {
    Identity(R),
    Gzip(flate2::read::GzDecoder<R>),
    Deflate(flate2::read::DeflateDecoder<R>),
    Brotli(brotli::Decompressor<R>),
}

impl<R: Read> Read for DecompressingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            DecompressorKind::Identity(r) => r.read(buf),
            DecompressorKind::Gzip(d) => d.read(buf),
            DecompressorKind::Deflate(d) => d.read(buf),
            DecompressorKind::Brotli(d) => d.read(buf),
        }
    }
}
```

### 3. CompressionConfig

```rust
pub struct CompressionConfig {
    pub add_accept_encoding: bool,    // Auto-add header
    pub auto_decompress: bool,        // Auto-decompress responses
    pub supported_encodings: Vec<ContentEncoding>,  // Preference order
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            add_accept_encoding: true,
            auto_decompress: true,
            supported_encodings: vec![
                ContentEncoding::Brotli,  // Best compression
                ContentEncoding::Gzip,
                ContentEncoding::Deflate,
            ],
        }
    }
}

impl CompressionConfig {
    pub fn accept_encoding_value(&self) -> String {
        self.supported_encodings
            .iter()
            .filter_map(|e| match e {
                ContentEncoding::Gzip => Some("gzip"),
                ContentEncoding::Deflate => Some("deflate"),
                ContentEncoding::Brotli => Some("br"),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}
```

### 4. Error Handling

Add to `HttpClientError`:
```rust
DecompressionFailed(String),
UnsupportedEncoding(String),
IoError(std::io::Error),  // via derive_more::From
```

### 5. Feature Gates

```toml
[dependencies]
flate2 = { version = "1.0", optional = true }
brotli = { version = "6.0", optional = true }

[features]
default = ["compression"]
compression = ["flate2", "brotli"]
gzip = ["flate2"]
brotli-support = ["brotli"]
```

---

## Tasks Breakdown (14 total)

1. [ ] Create `compression.rs` module
2. [ ] Implement `ContentEncoding` enum with from_header
3. [ ] Implement `CompressionConfig` with defaults
4. [ ] Implement `accept_encoding_value()` method
5. [ ] Create `DecompressorKind` enum (feature-gated)
6. [ ] Create `DecompressingReader` struct
7. [ ] Implement `Read` trait for `DecompressingReader`
8. [ ] Add compression error variants to `HttpClientError`
9. [ ] Add Accept-Encoding header logic (request building)
10. [ ] Add decompression logic (response reading)
11. [ ] Add `body()` and `raw_body()` methods
12. [ ] Add compression configuration API
13. [ ] Write comprehensive unit tests
14. [ ] Verify feature gates work

---

## Files to Create/Modify

**NEW**:
- `backends/foundation_core/src/wire/simple_http/client/compression.rs`

**MODIFY**:
- `backends/foundation_core/src/wire/simple_http/client/mod.rs` (add module, exports)
- `backends/foundation_core/src/wire/simple_http/client/errors.rs` (add error variants)
- `backends/foundation_core/src/wire/simple_http/client/request.rs` (Accept-Encoding header)
- `backends/foundation_core/src/wire/simple_http/client/intro.rs` (response body methods)
- `backends/foundation_core/Cargo.toml` (add dependencies, features)

---

## Integration Points

**Request Building** (PreparedRequest):
```rust
// If compression enabled and no Accept-Encoding header set
if config.add_accept_encoding && !headers.contains("Accept-Encoding") {
    headers.insert("Accept-Encoding", config.accept_encoding_value());
}
```

**Response Reading** (ResponseIntro):
```rust
// Parse Content-Encoding header
let encoding = ContentEncoding::from_header(
    headers.get("Content-Encoding").unwrap_or("identity")
);

// Wrap body reader if auto_decompress enabled
let body_reader = if config.auto_decompress {
    DecompressingReader::new(raw_reader, encoding)?
} else {
    raw_reader
};
```

---

## Critical Patterns (From Project)

### Use RawStream, Not Connection
**ALWAYS use RawStream abstraction** for connection handling:
```rust
// ✅ CORRECT - Use RawStream
use crate::netcap::RawStream;

let conn = Connection::with_timeout(addr, timeout)?;
let stream = RawStream::from_connection(conn)?;
// Automatic buffering, peek support, address tracking

// ❌ INCORRECT - Don't use raw Connection directly
use crate::netcap::Connection;
let conn = Connection::with_timeout(addr, timeout)?;
// Manual buffering needed, no peek, manual address tracking
```

### Visibility Policy
**ALL types must be public** - no `pub(crate)`:
```rust
pub struct CompressionConfig { ... }  // ✅ Always pub
pub enum ContentEncoding { ... }      // ✅ Always pub
pub fn accept_encoding_value() { ... } // ✅ Always pub
```

### Error Handling Pattern
Use `derive_more::From`:
```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    #[from]
    IoError(std::io::Error),  // Auto-converts

    #[from(ignore)]
    DecompressionFailed(String),  // Manual only
}
```

### Testing Pattern
TDD with WHY/WHAT documentation:
```rust
/// WHY: Verify ContentEncoding correctly parses gzip header
/// WHAT: Tests from_header with "gzip" input
#[test]
fn test_content_encoding_gzip() {
    let encoding = ContentEncoding::from_header("gzip");
    assert!(matches!(encoding, ContentEncoding::Gzip));
}
```

### Feature Gates
```rust
#[cfg(feature = "compression")]
use flate2::read::{GzDecoder, DeflateDecoder};

#[cfg(feature = "brotli-support")]
use brotli::Decompressor;

enum DecompressorKind<R: Read> {
    Identity(R),
    #[cfg(feature = "compression")]
    Gzip(GzDecoder<R>),
    #[cfg(feature = "compression")]
    Deflate(DeflateDecoder<R>),
    #[cfg(feature = "brotli-support")]
    Brotli(Decompressor<R>),
}
```

---

## Success Criteria

- [ ] All 14 tasks complete
- [ ] `compression.rs` compiles
- [ ] ContentEncoding parses all formats
- [ ] DecompressingReader streams correctly
- [ ] Gzip, deflate, brotli all work
- [ ] Accept-Encoding auto-added
- [ ] body() returns decompressed content
- [ ] raw_body() returns original bytes
- [ ] Feature gates functional
- [ ] All tests pass (unit + integration)
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy` passes (no warnings)

---

## Verification Commands

```bash
# Format check
cargo fmt --check

# Clippy check
cargo clippy --package foundation_core -- -D warnings

# Tests
cargo test --package foundation_core -- compression

# Build variations
cargo build --package foundation_core
cargo build --package foundation_core --features compression
cargo build --package foundation_core --no-default-features
```

---

## Before Starting - Retrieval Checklist

**MANDATORY**: Use retrieval-led reasoning:
1. [ ] Search for similar features (Grep "decompres\|compress\|encoding")
2. [ ] Read existing error handling patterns (check errors.rs)
3. [ ] Check how other features use Read trait (check io/ioutils)
4. [ ] Review testing patterns (read existing test files)
5. [ ] Check Cargo.toml for dependency patterns
6. [ ] Read intro.rs and request.rs for integration points

**Show evidence**: Reference specific files/patterns found.

---

## Notes

**Streaming NOT Buffering**:
- Use `Read` trait, NOT `read_to_end()`
- Decompress on-demand as data is read
- Memory-efficient for large responses

**Unknown Encodings**:
- Return raw body (Identity)
- Log warning if needed
- Don't fail request

**Dependencies**:
- flate2: Mature, well-tested gzip/deflate
- brotli: Larger dep, separate feature gate

**Configuration API**:
- Client-level default
- Per-request override
- Both body() and raw_body() access

---

## Related Documentation

**Rules to Load**:
- Rules 01-04 (mandatory)
- Rule 13 (implementation guide)
- Rule 14 (machine prompts)
- Rule 15 (context compaction)
- .agents/stacks/rust.md (Rust conventions)

**Feature Dependencies**:
- foundation: HttpClientError patterns
- request-response: PreparedRequest, ResponseIntro
- connection: Stream handling patterns

**Next Features After This**:
- public-api (will use CompressionConfig)
- Others: proxy-support, auth-helpers (parallel)

---

**Token Count**: ~800 tokens (97% reduction from full spec)
**Generated**: 2026-02-01
**Valid Until**: Compression feature complete
**Delete When**: Feature marked 100% complete

