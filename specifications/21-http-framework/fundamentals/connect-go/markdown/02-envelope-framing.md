# connect-go — Envelope Framing

**Source:** `envelope.go` (388 LOC). Every streaming RPC message is wrapped in a 5-byte envelope — 1 byte of flags followed by a 4-byte big-endian length prefix. This framing is shared across all three protocols but interpreted differently.

## Envelope Structure

```
+--------+--------+--------+--------+--------+
| Flags  |         Length (uint32, BE)       |
| 1 byte |           4 bytes                 |
+--------+--------+--------+--------+--------+
|              Payload (variable)             |
|         Length bytes of data               |
+---------------------------------------------+
```

```go
// envelope.go:45
type envelope struct {
    Data   *bytes.Buffer  // Payload bytes
    Flags  uint8          // Bitwise flags
    offset int64          // Read/write cursor for io.Reader/io.WriterTo
}
```

## Envelope Flags

```go
// envelope.go:29
const flagEnvelopeCompressed = 0b00000001  // Shared across all protocols
```

Protocol-specific flags (defined in protocol files):

| Flag | Value | Protocol | Meaning |
|------|-------|----------|---------|
| `flagEnvelopeCompressed` | `0x01` | All | Payload is compressed |
| `connectFlagEnvelopeEndStream` | `0x02` | Connect | Final message with error + trailers |
| `grpcFlagEnvelopeTrailer` | `0x80` | gRPC-Web | Contains trailer headers block |

**Aha:** The flag `0x01` has the same meaning across all three protocols — compressed payload. This is intentional: it allows middleboxes that understand compression to handle any protocol without knowing the specifics. Protocol-specific flags (`0x02` for Connect end-stream, `0x80` for gRPC-Web trailers) occupy the upper bits and are mutually exclusive.

## Envelope Prefix Construction

```go
// envelope.go:377
func makeEnvelopePrefix(flags uint8, size int) ([5]byte, error) {
    size64 := int64(size)
    if size64 < 0 || size64 > math.MaxUint32 {
        return [5]byte{}, fmt.Errorf("size %d out of bounds", size)
    }
    prefix := [5]byte{}
    prefix[0] = flags
    binary.BigEndian.PutUint32(prefix[1:5], uint32(size64))
    return prefix, nil
}
```

The maximum payload size is `math.MaxUint32` (4,294,967,295 bytes ≈ 4 GiB).

## EnvelopeWriter

```go
// envelope.go:128
type envelopeWriter struct {
    ctx              context.Context
    sender           messageSender    // writes to underlying transport
    codec            Codec            // marshals messages
    compressMinBytes int              // min size before compression
    compressionPool  *compressionPool // gzip or other
    bufferPool       *bufferPool      // bytes.Buffer reuse
    sendMaxBytes     int              // max message size limit
}
```

The `Marshal()` method has two code paths:

```go
// envelope.go:138
func (w *envelopeWriter) Marshal(message any) *Error {
    if message == nil {
        // Send no-op to create request and send headers
        payload := nopPayload{}
        if _, err := w.sender.Send(payload); err != nil { ... }
        return nil
    }
    if appender, ok := w.codec.(marshalAppender); ok {
        return w.marshalAppend(message, appender)  // zero-copy path
    }
    return w.marshal(message)  // allocates []byte
}
```

### Compression Decision in `Write()`

```go
// envelope.go:158
func (w *envelopeWriter) Write(env *envelope) *Error {
    // Skip compression if: already compressed, no pool, or below threshold
    if env.IsSet(flagEnvelopeCompressed) ||
        w.compressionPool == nil ||
        env.Data.Len() < w.compressMinBytes {
        if w.sendMaxBytes > 0 && env.Data.Len() > w.sendMaxBytes {
            return errorf(CodeResourceExhausted, "message size %d exceeds sendMaxBytes %d", ...)
        }
        return w.write(env)
    }
    // Compress into pooled buffer
    data := w.bufferPool.Get()
    defer w.bufferPool.Put(data)
    if err := w.compressionPool.Compress(data, env.Data); err != nil { return err }
    if w.sendMaxBytes > 0 && data.Len() > w.sendMaxBytes {
        return errorf(CodeResourceExhausted, "compressed message size %d exceeds sendMaxBytes %d", ...)
    }
    return w.write(&envelope{
        Data:  data,
        Flags: env.Flags | flagEnvelopeCompressed,
    })
}
```

**Aha:** The compression check happens in `Write()`, not `Marshal()`. This means `marshalAppend()` can pre-populate the envelope with raw bytes, and `Write()` decides whether to compress based on the actual payload size vs `compressMinBytes`. The caller doesn't need to know about compression thresholds.

### marshalAppend — Zero-Copy Path

```go
// envelope.go:181
func (w *envelopeWriter) marshalAppend(message any, codec marshalAppender) *Error {
    buffer := w.bufferPool.Get()
    defer w.bufferPool.Put(buffer)
    raw, err := codec.MarshalAppend(buffer.Bytes(), message)
    if cap(raw) > buffer.Cap() {
        // Buffer was too small — replace it with the larger allocation
        *buffer = *bytes.NewBuffer(raw)
    } else {
        // Buffer was sufficient — just fix the internal state
        buffer.Write(raw)
    }
    envelope := &envelope{Data: buffer}
    return w.Write(envelope)
}
```

When `MarshalAppend` grows the slice beyond the pool buffer's capacity, the new larger buffer replaces the pooled one. This is a pessimistic resize strategy — the larger buffer is kept for future use.

## EnvelopeReader

```go
// envelope.go:230
type envelopeReader struct {
    ctx             context.Context
    reader          io.Reader
    bytesRead       int64  // tracks bytes read (detects trailers-only gRPC)
    codec           Codec
    last            envelope  // stores special envelope data
    compressionPool *compressionPool
    bufferPool      *bufferPool
    readMaxBytes    int
}
```

### Read() — Prefix + Payload

```go
// envelope.go:317
func (r *envelopeReader) Read(env *envelope) *Error {
    prefixes := [5]byte{}
    n, err := io.ReadFull(r.reader, prefixes[:])
    r.bytesRead += int64(n)
    if err != nil {
        if errors.Is(err, io.EOF) {
            return NewError(CodeUnknown, err)  // clean stream end
        }
        err = wrapIfMaxBytesError(err, "read 5 byte message prefix")
        err = wrapIfContextDone(r.ctx, err)
        return errorf(CodeInvalidArgument, "protocol error: incomplete envelope: %w", err)
    }
    size := int64(binary.BigEndian.Uint32(prefixes[1:5]))
    if r.readMaxBytes > 0 && size > int64(r.readMaxBytes) {
        n, err := io.CopyN(io.Discard, r.reader, size)  // drain to allow reuse
        r.bytesRead += n
        return errorf(CodeResourceExhausted, "message size %d is larger than configured max %d", size, r.readMaxBytes)
    }
    readN, err := io.CopyN(env.Data, r.reader, size)
    r.bytesRead += readN
    if err != nil {
        if errors.Is(err, io.EOF) {
            return errorf(CodeInvalidArgument, "protocol error: promised %d bytes in enveloped message, got %d bytes", size, readN)
        }
        return errorf(CodeUnknown, "read enveloped message: %w", err)
    }
    env.Flags = prefixes[0]
    return nil
}
```

**Aha:** When `readMaxBytes` is exceeded, the reader drains the full message from the stream (`io.CopyN(io.Discard, ...)`) rather than leaving it partially consumed. This is critical for HTTP connection reuse — an unconsumed message body would corrupt the next request on the same connection.

### Unmarshal() — Full Message Processing

```go
// envelope.go:241
func (r *envelopeReader) Unmarshal(message any) *Error {
    buffer := r.bufferPool.Get()
    var dontRelease *bytes.Buffer
    defer func() {
        if buffer != dontRelease {
            r.bufferPool.Put(buffer)
        }
    }()

    env := &envelope{Data: buffer}
    err := r.Read(env)

    // Handle clean stream end
    case err == nil && env.Flags == 0 && env.Data.Len() == 0:
        return nil  // zero value is correct for empty message

    // Handle protocol-specific end-stream
    if env.Flags != 0 && env.Flags != flagEnvelopeCompressed {
        // Drain rest of stream to ensure no extra data
        numBytes, err := discard(r.reader)
        r.bytesRead += numBytes
        if numBytes > 0 {
            return errorf(CodeInternal, "corrupt response: %d extra bytes after end of stream", numBytes)
        }
        r.last = envelope{Data: data, Flags: env.Flags}
        dontRelease = data  // don't return to pool
        return errSpecialEnvelope
    }

    // Normal message: decompress if needed
    if data.Len() > 0 && env.IsSet(flagEnvelopeCompressed) {
        decompressed := r.bufferPool.Get()
        if err := r.compressionPool.Decompress(decompressed, data, int64(r.readMaxBytes)); err != nil { return err }
        data = decompressed
    }

    // Unmarshal via codec
    if err := r.codec.Unmarshal(data.Bytes(), message); err != nil {
        return errorf(CodeInvalidArgument, "unmarshal message: %w", err)
    }
    return nil
}
```

## The Special Envelope Sentinel

```go
// envelope.go:31
var errSpecialEnvelope = errorf(
    CodeUnknown,
    "final message has protocol-specific flags: %w",
    io.EOF,
)
```

This sentinel error wraps `io.EOF` and signals to protocol-specific code that the final envelope has protocol-specific flags (Connect's `0x02` end-stream, gRPC-Web's `0x80` trailers). User code checks `errors.Is(err, io.EOF)` to detect stream end. Protocol wrappers inspect `reader.last` to extract error details and trailers.

## The discard() Function

```go
// protocol.go:289
const discardLimit = 1024 * 1024 * 4 // 4MiB

func discard(reader io.Reader) (int64, error) {
    if lr, ok := reader.(*io.LimitedReader); ok {
        return io.Copy(io.Discard, lr)
    }
    lr := &io.LimitedReader{R: reader, N: discardLimit}
    return io.Copy(io.Discard, lr)
}
```

Limits draining to 4 MiB to prevent a malicious server from keeping the connection open indefinitely.

## Next

[03-codec-system.md](03-codec-system.md) — The `Codec` interface and its extensions (`marshalAppender`, `stableCodec`).
