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

**This is the single normative definition** (Decision 05's earlier duplicate sketch was
removed — its protocols map their header names onto this function). From connect-go's
`negotiateCompression`:

```rust
pub fn negotiate_compression(
    registry: &CompressionRegistry,
    request_encoding: Option<&str>,
    accept_encoding: Option<&str>,
) -> ConnectResult<NegotiatedCompression> {
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
    /// Compressor for outgoing messages (None = identity).
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
    /// Default: **unlimited (0)** to match connect-go (P17/Q7); a cap is opt-in.
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
            read_max_bytes: 0,   // unlimited by default (connect-go parity, P17/Q7); cap is opt-in
            send_max_bytes: 0,
            compress_min_bytes: 0,
        }
    }
}
```

### Buffer Pool — ownership model (decided; RS6 expanded)

connect-go uses `sync.Pool`; ours is **per worker thread**. Terms first, because "worker"
is load-bearing: a *worker* is **one OS thread of the valtron pool**, running a
`LocalExecutorEngine` that drives *many* tasks by polling them one at a time. Tasks are not
workers — a thread hosts hundreds of tasks, but polls on a thread are serialized, so a
thread-local pool is **uncontended by construction** (no `Mutex`, RS6). Per-task pools were
rejected (thousands of idle buffers, no cross-task reuse); a process-global pool was
rejected (the `Mutex`).

```rust
pub struct BufferPool {
    pool: Vec<Vec<u8>>,   // recycled buffers (LIFO, hot path — zero sync)
    /// Lock-free return lane: final drops of frozen frames, from ANY thread, come home.
    /// Bounded — overflow (or an oversized buffer) is simply freed, so an idle/dead
    /// thread can never hoard memory.
    return_lane: Arc<ConcurrentQueue<Vec<u8>>>,
    initial_capacity: usize,
    max_recycle_size: usize,
}

impl BufferPool {
    pub fn new() -> Self {
        Self {
            pool: Vec::new(),
            return_lane: Arc::new(ConcurrentQueue::bounded(RETURN_LANE_DEPTH)),
            initial_capacity: 512,
            max_recycle_size: 8 * 1024 * 1024,  // 8 MiB, matching connect-go
        }
    }

    pub fn get(&mut self) -> Vec<u8>;      // pop local LIFO; when empty, drain the return lane
    pub fn put(&mut self, mut buf: Vec<u8>);
    /// Convert filled scratch into a pipe-safe `Bytes` that RECYCLES on final drop —
    /// this IS the "convert" arm of invariant 1 below.
    pub fn freeze(&self, buf: Vec<u8>) -> Bytes;
}

/// Owner behind `Bytes::from_owner` (or the equivalent vtable on our own Bytes type) —
/// returns its allocation to the ORIGIN pool on FINAL drop. Clones/slices of the `Bytes`
/// only bump refcounts; the owner drops exactly once. Recycling is invisible to every
/// consumer: the pipe, facade, and transport see a normal `Bytes`.
struct PooledFrame {
    buf: Vec<u8>,                          // the encoded frame
    home: Arc<ConcurrentQueue<Vec<u8>>>,   // origin pool's return lane
}
impl AsRef<[u8]> for PooledFrame { fn as_ref(&self) -> &[u8] { &self.buf } }
impl Drop for PooledFrame {
    fn drop(&mut self) {
        let buf = mem::take(&mut self.buf);
        if buf.capacity() <= MAX_RECYCLE { let _ = self.home.push(buf); } // lane full → free
    }
}
```

**The two byte paths own their memory differently:**

```
READ PATH  — task-owned, NOT pooled
socket ─read─► IncrementalDecoder accumulating buffer (BytesMut, OWNED BY THE READER TASK —
               it must survive parks/yields, and the task's next poll may run on another
               thread, so it cannot be thread-local)
        └─ completed frame = split/freeze ─► Bytes (zero-copy) ─► pipe ─► facade

WRITE PATH — thread-local pool scratch
handler msg ─marshal_append─► pooled Vec ─compress─► pooled Vec
    ├─ consumed within the SAME poll (writer flushes to socket / renders the chunk)
    │      → put() back                                          ✓ recycled
    └─ must OUTLIVE the poll (frame parked in a pipe)
           → pool.freeze(buf) → Bytes with a PooledFrame owner: on FINAL drop the
             allocation returns to its ORIGIN pool via the lock-free return lane
                                                                 ✓ recycled (decided)
```

**Origin-return semantics (decided — option A):** the frame is produced on thread A but its
last `Bytes` reference usually drops on thread B (the writer task's thread), so `PooledFrame`
carries an `Arc` to its **origin** pool's return lane and buffers always come home. The hot
path stays the plain local LIFO (zero sync); the only synchronization is a lock-free push on
final drop plus an occasional lane-drain in `get()` — no `Mutex`, so RS6's property holds,
and pool sizes stay matched to each thread's *production* rate. (Dropping into the
*dropping* thread's pool was rejected: buffers would systematically drain toward consumer
threads while producer threads keep allocating fresh, and a final drop on a non-worker
thread has no pool at all.) Scope: `freeze` serves **write-path encoded frames** (and
decompression outputs); read-path frames are already zero-copy slices of the reader task's
`BytesMut` and were never pool candidates. This settles the frame-recycling half of the old buffer-reuse question; only
decode-scratch reuse remains an implementation-time tunable (plan.md).

**Invariants (normative):**
1. **A pooled buffer never crosses a yield point as a borrow** — acquire → fill →
   `put()`-back or `freeze()` within one poll. Thread-locality demands it (the next poll
   may run elsewhere) and pool starvation punishes violations. `freeze` is the sanctioned
   way for pooled memory to outlive the poll: ownership transfers to the `Bytes`.
2. **Anything entering a pipe owns its memory** (`Bytes`) — never a pooled borrow. A frozen
   frame satisfies this: it is a normal owned `Bytes`; recycling on final drop is invisible
   to consumers.

**Reaching the pool:** code already running on a worker uses the thread-local accessor
(`with_worker_buffer(|buf| …)`); work packaged *before* being submitted to the pool may
instead carry the pool handle in its **internal task context** (the transport/task-side
context — NOT the handler `Ctx`, which never exposes framework plumbing). Either way, the
pool a buffer returns to is the thread it was used on. Shared framework objects (e.g.
`ErrorWriter`, Decision 03) never **own** a pool — one instance is shared across threads,
so owning a `&mut`-API pool would force exactly the `Mutex` RS6 rejects; they borrow
through the accessor. Sizing matches connect-go (LIFO free-list, 8 MiB recycle cap);
revisit size classes only if profiling demands it.

## Consequences

- Gzip available by default (leveraging existing flate2 in foundation_netio)
- Zstd and brotli behind feature flags
- Custom compressors plug in via `Compressor` trait + `CompressionRegistry`
- Per-message compression for streaming (each envelope independent)
- Size-limited decompression prevents compression bomb DoS
- Buffer pool reduces GC/allocation pressure

## Decided Details

- **P17 / Q7 — `read_max_bytes` default (decided):** default to **unlimited (0)** to match
  connect-go; the 4 MiB cap becomes opt-in. We own the new http2 layer, so nothing forces
  a cap.
- **H10 — per-message checks:** apply `compress_min_bytes` per envelope (small messages
  sent uncompressed even when compression is negotiated); check `send_max_bytes` **after**
  compression.
- **RS6 — buffer pool (decided):** use thread-local pools in the valtron worker model to
  avoid `Mutex` contention; buffers never land in a **foreign** pool — final drops of
  frozen frames return to the **origin** pool via its lock-free return lane (§Buffer Pool).
  Expanded in §Buffer Pool: worker = executor **thread** (tasks share their thread's pool);
  the read path is task-owned `BytesMut` (survives parks), the pool serves write-path
  scratch; pipe-bound frames are recycled via `freeze`/`PooledFrame`; two normative
  invariants (never across a yield point as a borrow; pipes carry owned `Bytes`); shared
  objects borrow via `with_worker_buffer`, never own a pool.
- **RS10 — streaming compressor:** provide reset-able streaming compressor/decompressor
  (not only the batch API) with pooling, for large/streamed messages.


## Cross-References

- Envelope framing / per-message compression flags: Decision 05. Middleware ordering (HTTP vs
  seam): Decision 08. Size limits also referenced by client/server configs (Decisions 07/12).
