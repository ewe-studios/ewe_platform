---
feature: compression
description: Automatic compression/decompression of HTTP bodies with gzip, deflate, and brotli support
status: pending
depends_on:
  - foundation
estimated_effort: small
created: 2026-01-19
last_updated: 2026-01-19
---

# Compression Feature

## Overview

Add automatic compression and decompression support for HTTP request/response bodies. This feature enables transparent handling of gzip, deflate, and brotli compressed content, following HTTP content negotiation standards.

## Dependencies

This feature depends on:
- `foundation` - Uses HttpClientError for error handling

This feature is required by:
- `public-api` - Exposes compression configuration to users

## Requirements

### Auto Accept-Encoding Header

Automatically add `Accept-Encoding` header to requests:

```rust
// When compression is enabled (default)
Accept-Encoding: gzip, deflate, br

// Header is added automatically unless:
// 1. Compression is disabled
// 2. User has already set Accept-Encoding header
```

### Automatic Decompression

Decompress response bodies based on `Content-Encoding` header:

```rust
pub enum ContentEncoding {
    Identity,    // No compression
    Gzip,        // gzip compression
    Deflate,     // deflate compression
    Brotli,      // br (brotli) compression
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

### Streaming Decompression

Decompression MUST be iterator-based, not buffered:

```rust
pub struct DecompressingReader<R: Read> {
    inner: DecompressorKind<R>,
}

enum DecompressorKind<R: Read> {
    Identity(R),
    Gzip(GzDecoder<R>),
    Deflate(DeflateDecoder<R>),
    Brotli(BrotliDecoder<R>),
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

### Feature-Gated Dependencies

Compression libraries are optional:

```toml
[dependencies]
flate2 = { version = "1.0", optional = true }
brotli = { version = "6.0", optional = true }

[features]
default = ["compression"]
compression = ["flate2", "brotli"]
gzip = ["flate2"]
brotli = ["brotli"]
```

### Configuration API

```rust
// Client-level configuration
let client = SimpleHttpClient::new()
    .compression(true)  // Enable compression (default)
    .compression(false); // Disable compression

// Per-request override
let response = client.get(url)
    .no_compression()  // Disable for this request
    .send()?;

// Access both raw and decompressed body
let response = client.get(url).send()?;
response.body();      // Auto-decompressed
response.raw_body();  // Original compressed bytes
```

### Error Handling

Add compression-related error variants:

```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    // ... existing variants ...

    #[from(ignore)]
    DecompressionFailed(String),

    #[from(ignore)]
    UnsupportedEncoding(String),

    #[from]
    IoError(std::io::Error),
}
```

## Implementation Details

### File Structure

```
client/
├── compression.rs    (NEW - Compression/decompression logic)
└── ...
```

### CompressionConfig

```rust
#[derive(Clone, Debug)]
pub struct CompressionConfig {
    /// Enable automatic Accept-Encoding header
    pub add_accept_encoding: bool,

    /// Enable automatic decompression
    pub auto_decompress: bool,

    /// Supported encodings (in preference order)
    pub supported_encodings: Vec<ContentEncoding>,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            add_accept_encoding: true,
            auto_decompress: true,
            supported_encodings: vec![
                ContentEncoding::Brotli,
                ContentEncoding::Gzip,
                ContentEncoding::Deflate,
            ],
        }
    }
}
```

### Accept-Encoding Header Generation

```rust
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

### Integration Points

1. **Request Building**: Add Accept-Encoding header if compression enabled
2. **Response Reading**: Wrap response body reader with DecompressingReader
3. **ClientResponse**: Expose both `body()` and `raw_body()` methods

## Success Criteria

- [ ] `compression.rs` exists and compiles
- [ ] `ContentEncoding` enum correctly parses header values
- [ ] `DecompressingReader` implements streaming decompression
- [ ] Gzip decompression works correctly
- [ ] Deflate decompression works correctly
- [ ] Brotli decompression works correctly (feature-gated)
- [ ] Accept-Encoding header is auto-added when compression enabled
- [ ] Per-request compression override works
- [ ] `response.body()` returns decompressed content
- [ ] `response.raw_body()` returns original bytes
- [ ] Compression can be disabled client-wide
- [ ] Feature gates work correctly
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- compression
cargo build --package foundation_core
cargo build --package foundation_core --features compression
cargo build --package foundation_core --no-default-features
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** foundation feature is complete
- **MUST READ** flate2 documentation for GzDecoder, DeflateDecoder
- **MUST READ** brotli crate documentation for BrotliDecoder
- **MUST CHECK** existing simple_http structures for integration points

### Implementation Guidelines
- Use streaming (Read trait) for decompression, NOT buffering
- Feature gate brotli separately (larger dependency)
- Handle unknown encodings gracefully (return raw body)
- Preserve original body access via `raw_body()`
- Follow existing error patterns with derive_more::From

### Testing Considerations
- Test with real compressed content
- Test streaming behavior (not just final result)
- Test fallback when compression dependencies not available
- Test per-request override behavior

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
