---
feature: compression
completed: 0
uncompleted: 9
last_updated: 2026-01-19
tools:
  - Rust
  - cargo
---

# Compression - Tasks

## Task List

### Module Setup
- [ ] Create `client/compression.rs` - Compression/decompression module
- [ ] Update Cargo.toml with flate2 and brotli optional dependencies
- [ ] Add compression feature flags

### Core Types
- [ ] Implement `ContentEncoding` enum with from_header() parser
- [ ] Implement `CompressionConfig` with default encodings
- [ ] Implement `accept_encoding_value()` header generation

### Streaming Decompression
- [ ] Implement `DecompressorKind<R>` enum for different decompressors
- [ ] Implement `DecompressingReader<R>` with Read trait
- [ ] Add feature gates for gzip/deflate (flate2) and brotli

### Error Handling
- [ ] Add `DecompressionFailed` variant to HttpClientError
- [ ] Add `UnsupportedEncoding` variant to HttpClientError

### Integration
- [ ] Add Accept-Encoding header in request builder when compression enabled
- [ ] Wrap response body with DecompressingReader based on Content-Encoding
- [ ] Add `body()` method for decompressed content
- [ ] Add `raw_body()` method for original bytes
- [ ] Add `compression()` config method to SimpleHttpClient builder
- [ ] Add `no_compression()` method to request builder

## Implementation Order

1. **Cargo.toml** - Add optional dependencies and feature flags
2. **compression.rs** - Core types (ContentEncoding, CompressionConfig)
3. **compression.rs** - DecompressingReader with streaming decompression
4. **errors.rs** - Add new error variants
5. **Integration** - Connect to request builder and response handling

## Notes

### ContentEncoding Pattern
```rust
pub enum ContentEncoding {
    Identity,
    Gzip,
    Deflate,
    Brotli,
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

### DecompressingReader Pattern
```rust
pub struct DecompressingReader<R: Read> {
    inner: DecompressorKind<R>,
}

impl<R: Read> DecompressingReader<R> {
    pub fn new(reader: R, encoding: ContentEncoding) -> Result<Self, HttpClientError> {
        let inner = match encoding {
            ContentEncoding::Identity => DecompressorKind::Identity(reader),
            #[cfg(feature = "compression")]
            ContentEncoding::Gzip => DecompressorKind::Gzip(GzDecoder::new(reader)),
            // ... etc
        };
        Ok(Self { inner })
    }
}
```

### Feature Flags Pattern
```toml
[features]
default = ["compression"]
compression = ["flate2", "brotli"]
gzip = ["flate2"]
brotli = ["dep:brotli"]
```

---
*Last Updated: 2026-01-19*
