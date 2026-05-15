# connect-go — Compression and Buffer Pools

**Source:** `compression.go` (225 LOC), `buffer_pool.go` (55 LOC). ConnectRPC uses `sync.Pool` for both compression pools and byte buffer pools to avoid allocation overhead in high-throughput RPC scenarios. The compression system supports asymmetric compression — a client may send compressed requests while accepting uncompressed responses, and vice versa.

## Buffer Pool

```go
// buffer_pool.go:22
const (
    initialBufferSize    = 512
    maxRecycleBufferSize = 8 * 1024 * 1024 // 8MiB
)

type bufferPool struct {
    sync.Pool
}

func newBufferPool() *bufferPool {
    return &bufferPool{
        Pool: sync.Pool{
            New: func() any {
                return bytes.NewBuffer(make([]byte, 0, initialBufferSize))
            },
        },
    }
}
```

### Get/Put Lifecycle

```go
func (b *bufferPool) Get() *bytes.Buffer {
    if buf, ok := b.Pool.Get().(*bytes.Buffer); ok {
        return buf
    }
    return bytes.NewBuffer(make([]byte, 0, initialBufferSize))
}

func (b *bufferPool) Put(buffer *bytes.Buffer) {
    if buffer.Cap() > maxRecycleBufferSize {
        return  // discard — too large
    }
    buffer.Reset()
    b.Pool.Put(buffer)
}
```

**Aha:** The 8MiB recycle limit prevents memory leaks. If a buffer grew to handle a 50MB message, recycling it would hold 50MB of memory in the pool indefinitely — a classic Go memory leak pattern. By discarding oversized buffers, the pool self-regulates based on actual workload. The initial 512-byte capacity is chosen because most RPC messages are small — a tiny initial buffer avoids wasting memory on unused capacity.

### Where Buffers Are Used

Buffers from the pool are used throughout the protocol stack:
- Codec marshaling/unmarshaling intermediate storage
- Compression/decompression scratch space
- Envelope reader/writer temporary storage

Each protocol handler and client has its own `BufferPool` reference, passed through `protocolHandlerParams` and `protocolClientParams`.

## Compression Interfaces

```go
// compression.go:34
type Decompressor interface {
    io.Reader
    Close() error
    Reset(io.Reader) error
}

// compression.go:48
type Compressor interface {
    io.Writer
    Close() error
    Reset(io.Writer)
}
```

The standard library's `*gzip.Reader` and `*gzip.Writer` implement these interfaces directly. The `Reset` method is critical — it allows reusing the same compressor/decompressor instance for different data streams without reallocating internal buffers.

## Compression Pool

```go
// compression.go:60
type compressionPool struct {
    decompressors sync.Pool
    compressors   sync.Pool
}

func newCompressionPool(
    newDecompressor func() Decompressor,
    newCompressor func() Compressor,
) *compressionPool {
    return &compressionPool{
        decompressors: sync.Pool{New: func() any { return newDecompressor() }},
        compressors:   sync.Pool{New: func() any { return newCompressor() }},
    }
}
```

### Decompress Method

```go
// compression.go:82
func (c *compressionPool) Decompress(dst *bytes.Buffer, src *bytes.Buffer, readMaxBytes int64) *Error {
    decompressor, err := c.getDecompressor(src)
    if err != nil {
        return errorf(CodeInvalidArgument, "get decompressor: %w", err)
    }
    reader := io.Reader(decompressor)
    if readMaxBytes > 0 && readMaxBytes < math.MaxInt64 {
        reader = io.LimitReader(decompressor, readMaxBytes+1)
    }
    bytesRead, err := dst.ReadFrom(reader)
    if err != nil {
        _ = c.putDecompressor(decompressor)
        err = wrapIfContextError(err)
        if connectErr, ok := asError(err); ok {
            return connectErr
        }
        return errorf(CodeInvalidArgument, "decompress: %w", err)
    }
    if readMaxBytes > 0 && bytesRead > readMaxBytes {
        // Drain remaining to safely recycle decompressor
        discardedBytes, err := io.Copy(io.Discard, decompressor)
        _ = c.putDecompressor(decompressor)
        return errorf(CodeResourceExhausted, "message size %d is larger than configured max %d",
            bytesRead+discardedBytes, readMaxBytes)
    }
    if err := c.putDecompressor(decompressor); err != nil {
        return errorf(CodeUnknown, "recycle decompressor: %w", err)
    }
    return nil
}
```

**Aha:** When a message exceeds `readMaxBytes`, the decompressor must be fully drained before recycling. If the decompressor is recycled while still holding buffered data from the oversized message, the next use would read garbage from the previous stream. The `io.Copy(io.Discard, decompressor)` ensures the decompressor is in a clean state.

### Compress Method

```go
// compression.go:114
func (c *compressionPool) Compress(dst *bytes.Buffer, src *bytes.Buffer) *Error {
    compressor, err := c.getCompressor(dst)
    if err != nil {
        return errorf(CodeUnknown, "get compressor: %w", err)
    }
    if _, err := src.WriteTo(compressor); err != nil {
        _ = c.putCompressor(compressor)
        err = wrapIfContextError(err)
        if connectErr, ok := asError(err); ok {
            return connectErr
        }
        return errorf(CodeInternal, "compress: %w", err)
    }
    if err := c.putCompressor(compressor); err != nil {
        return errorf(CodeInternal, "recycle compressor: %w", err)
    }
    return nil
}
```

### Get/Put Lifecycle

```go
// compression.go:133
func (c *compressionPool) getDecompressor(reader io.Reader) (Decompressor, error) {
    decompressor := c.decompressors.Get().(Decompressor)
    return decompressor, decompressor.Reset(reader)
}

// compression.go:141
func (c *compressionPool) putDecompressor(decompressor Decompressor) error {
    _ = decompressor.Close()
    _ = decompressor.Reset(http.NoBody)  // clear internal reader reference
    c.decompressors.Put(decompressor)
    return nil
}

// compression.go:156
func (c *compressionPool) getCompressor(writer io.Writer) (Compressor, error) {
    compressor := c.compressors.Get().(Compressor)
    compressor.Reset(writer)
    return compressor, nil
}

// compression.go:165
func (c *compressionPool) putCompressor(compressor Compressor) error {
    if err := compressor.Close(); err != nil {
        return err
    }
    compressor.Reset(io.Discard)  // clear internal writer reference
    c.compressors.Put(compressor)
    return nil
}
```

**Aha:** Decompressors are reset with `http.NoBody` when returned to the pool; compressors are reset with `io.Discard`. This clears internal references to the underlying reader/writer, preventing memory leaks from retained references. The reset on put also ensures the next get starts in a clean state — some decompressors (like gzip) need to read header data on reset, and starting with an empty body avoids errors.

## Read-Only Compression Pools

```go
// compression.go:176
type readOnlyCompressionPools interface {
    Get(string) *compressionPool
    Contains(string) bool
    CommaSeparatedNames() string
}
```

The `newReadOnlyCompressionPools()` function (`compression.go:183`) reverses the registration order for preference:

```go
func newReadOnlyCompressionPools(
    nameToPool map[string]*compressionPool,
    reversedNames []string,
) readOnlyCompressionPools {
    names := make([]string, 0, len(reversedNames))
    seen := make(map[string]struct{}, len(reversedNames))
    for i := len(reversedNames) - 1; i >= 0; i-- {
        name := reversedNames[i]
        if _, ok := seen[name]; continue
        seen[name] = struct{}{}
        names = append(names, name)
    }
    return &namedCompressionPools{
        nameToPool:          nameToPool,
        commaSeparatedNames: strings.Join(names, ","),
    }
}
```

**Aha:** The last registered compression algorithm is the most preferred. If you register `[gzip, zstd]`, the `Accept-Encoding` header will be `zstd,gzip` — zstd first because it was registered last. This allows users to express preference by ordering their `WithCompression` calls. Duplicates are deduplicated, keeping only the last registration.

## Compression Negotiation (Recap)

From `protocol.go:302`:

```go
func negotiateCompression(availableCompressors, sent, accept string) (reqComp, respComp string, err *Error) {
    requestCompression = sent
    responseCompression = requestCompression
    if responseCompression == identity && accept != "" {
        for _, name := range strings.FieldsFunc(accept, isCommaOrSpace) {
            if availableCompressors.Contains(name) {
                responseCompression = name
                break
            }
        }
    }
}
```

The asymmetric negotiation means:
- If the client compressed its request with `gzip`, the server responds with `gzip` (matching request compression).
- If the client didn't compress but accepts `zstd,gzip`, the server picks the first mutually supported one.
- Unknown compression on the request side returns `CodeUnimplemented` with acceptable encodings.

## Default Gzip Registration

```go
// option.go:614
func withGzip() Option {
    return &compressionOption{
        Name: compressionGzip,
        CompressionPool: newCompressionPool(
            func() Decompressor { return &gzip.Reader{} },
            func() Compressor { return gzip.NewWriter(io.Discard) },
        ),
    }
}
```

Both clients and handlers register gzip by default, using the standard library's `compress/gzip` package at the default compression level. This means gzip is available out of the box without any configuration.

## Next

[09-handler-lifecycle.md](09-handler-lifecycle.md) — The `Handler` struct, `ServeHTTP` dispatch flow, handler constructor variants, and request/response header merging.
