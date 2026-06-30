# Decision 06: Compression System

## Context

connect-go defines pluggable compression via:

```go
type Decompressor interface {
    io.Reader
    Close() error
    Reset(io.Reader) error
}

type Compressor interface {
    io.Writer
    Close() error
    Reset(io.Writer)
}
```

With a `compressionPool` using `sync.Pool` for reuse, and negotiation via `Accept-Encoding` / `Content-Encoding` headers.

Key behaviors:
- Default: gzip (via `compress/gzip`) at default level
- Compression is per-message for streaming (each envelope independently compressed)
- `CompressMinBytes` — don't compress messages smaller than this (default 0 = always compress if compression configured)
- `ReadMaxBytes` — reject messages larger than this after decompression (DoS protection)
- `SendMaxBytes` — reject messages larger than this before sending

foundation_http already has `CompressionMiddleware` with `CompressionAlgorithm` enum supporting gzip and brotli. foundation_netio has flate2 (gzip) and brotli dependencies.

## Decision

### Compression Traits

```rust
/// Synchronous compressor for unary messages.
pub trait Compressor: Send + Sync + 'static {
    /// Algorithm name used in headers ("gzip", "zstd", "br", etc.)
    fn name(&self) -> &str;

    /// Compress data.
    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Decompress data. Returns error if decompressed size exceeds limit.
    fn decompress(&self, input: &[u8], max_bytes: usize) -> Result<Vec<u8>, CompressionError>;
}
```

### Built-in Compressors

```rust
/// Gzip compressor using flate2 (already in foundation_netio dependencies).
pub struct GzipCompressor {
    level: u32,  // flate2 compression level (default: 6)
}

/// Zstandard compressor (behind "zstd" feature flag).
#[cfg(feature = "zstd")]
pub struct ZstdCompressor {
    level: i32,  // zstd compression level (default: 3)
}

/// Brotli compressor (behind "brotli" feature flag, already in foundation_netio).
#[cfg(feature = "brotli")]
pub struct BrotliCompressor {
    quality: u32,  // brotli quality level (default: 4)
}
```

### Compression Registry

```rust
pub struct CompressionRegistry {
    compressors: HashMap<String, Arc<dyn Compressor>>,
    /// Ordered list of supported compression names for Accept-Encoding header.
    supported_names: Vec<String>,
}

impl CompressionRegistry {
    /// Create with gzip registered by default.
    pub fn new() -> Self;

    /// Register a compressor.
    pub fn register(&mut self, compressor: Arc<dyn Compressor>);

    /// Remove a compressor by name.
    pub fn remove(&mut self, name: &str);

    /// Look up a compressor by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Compressor>>;

    /// Format the Accept-Encoding / Connect-Accept-Encoding header value.
    pub fn accept_encoding_header(&self) -> &str;
}
```

### Compression Negotiation

From connect-go's `negotiateCompression`:

```rust
pub fn negotiate_compression(
    registry: &CompressionRegistry,
    request_encoding: Option<&str>,
    accept_encoding: Option<&str>,
) -> Result<NegotiatedCompression, ConnectError> {
    // 1. If request_encoding is set and not "identity", validate it's supported
    //    → Error CodeUnimplemented if not ("unsupported encoding X, supported: [gzip, ...]")
    // 2. If accept_encoding is set, find the first supported algorithm
    //    → Fall back to request_encoding if accept_encoding lists nothing we support
    // 3. Default: identity (no compression)
    // 4. Always accept "identity" as the least preferred
}

pub struct NegotiatedCompression {
    /// Compressor to decompress incoming messages (None = identity).
    pub request_decompressor: Option<Arc<dyn Compressor>>,
    /// Compressor name to compress outgoing messages (None = identity).
    pub response_compressor: Option<Arc<dyn Compressor>>,
}
```

### Per-Message Compression for Streaming

Each envelope in a streaming RPC is independently compressed:
- Writer checks `compress_min_bytes` per message
- If message is large enough and compression is negotiated, compress and set flag bit 0
- If message is too small, send uncompressed (flag bit 0 clear)

This is already handled by `EnvelopeWriter` in Decision 05.

### Size Limits

```rust
pub struct SizeLimits {
    /// Maximum decompressed message size to accept (DoS protection).
    /// Default: 4 MiB. Set to 0 for unlimited.
    pub read_max_bytes: usize,

    /// Maximum message size to send. Default: unlimited (0).
    pub send_max_bytes: usize,

    /// Minimum message size before compression is applied.
    /// Default: 0 (always compress if compression is configured).
    pub compress_min_bytes: usize,
}

impl Default for SizeLimits {
    fn default() -> Self {
        Self {
            read_max_bytes: 4 * 1024 * 1024,  // 4 MiB
            send_max_bytes: 0,
            compress_min_bytes: 0,
        }
    }
}
```

### Buffer Pool

connect-go uses `sync.Pool` for buffer reuse. We use a similar approach:

```rust
pub struct BufferPool {
    pool: Vec<Vec<u8>>,   // recycled buffers
    initial_capacity: usize,
    max_recycle_size: usize,
}

impl BufferPool {
    pub fn new() -> Self {
        Self {
            pool: Vec::new(),
            initial_capacity: 512,
            max_recycle_size: 8 * 1024 * 1024,  // 8 MiB, matching connect-go
        }
    }

    pub fn get(&mut self) -> Vec<u8>;
    pub fn put(&mut self, mut buf: Vec<u8>);
}
```

In production, wrap in `Mutex` or use thread-local pools. The buffer pool reduces allocation pressure during message marshaling/unmarshaling.

## Consequences

- Gzip available by default (leveraging existing flate2 in foundation_netio)
- Zstd and brotli behind feature flags
- Custom compressors plug in via `Compressor` trait + `CompressionRegistry`
- Per-message compression for streaming (each envelope independent)
- Size-limited decompression prevents compression bomb DoS
- Buffer pool reduces GC/allocation pressure

## Review-Gap Coverage

- **P17 / Q7 — `read_max_bytes` default (decided):** default to **unlimited (0)** to match
  connect-go; the 4 MiB cap becomes opt-in. We own the new http2 layer, so nothing forces
  a cap.
- **H10 — per-message checks:** apply `compress_min_bytes` per envelope (small messages
  sent uncompressed even when compression is negotiated); check `send_max_bytes` **after**
  compression.
- **RS6 — buffer pool (decided):** use thread-local pools in the valtron worker model to
  avoid `Mutex` contention; buffers do not migrate between workers.
- **RS10 — streaming compressor:** provide reset-able streaming compressor/decompressor
  (not only the batch API) with pooling, for large/streamed messages.

## Open Questions

1. **foundation_http CompressionMiddleware overlap**: foundation_http already has compression middleware. Should connectrpc bypass it and handle compression internally (as connect-go does), or can we reuse the middleware? connect-go handles compression per-message, not per-HTTP-response, so the middleware approach doesn't apply to streaming.
2. **Thread-safety for BufferPool**: In valtron's worker pool model, each worker thread could have its own buffer pool (thread-local) to avoid contention. Is this preferable to a shared `Mutex<BufferPool>`?
3. **Streaming compression context**: The protocol spec says "Compression contexts are not maintained over message boundaries." This means each message is compressed independently (no dictionary sharing). Confirm this is what we implement — it's simpler but slightly less efficient than context-preserving compression.
