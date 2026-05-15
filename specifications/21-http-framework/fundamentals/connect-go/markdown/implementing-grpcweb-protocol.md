# Implementing the gRPC-Web Protocol (Native, Over HTTP/1.1)

**Reference:** `connect-go/protocol_grpc.go` (~1010 LOC), `connect-go/envelope.go` (388 LOC), `connect-go/codec.go` (260 LOC), `connect-go/error.go` (472 LOC). This document provides a complete blueprint for implementing gRPC-Web natively — not via a proxy or Envoy translation layer — following the connect-go reference implementation's approach. The key insight: gRPC-Web encodes trailers as the final envelope in the response body because HTTP/1.1 lacks native trailer support.

## Protocol Philosophy

gRPC-Web was designed to bring gRPC to browsers and HTTP/1.1 environments. Since HTTP/1.1 doesn't support native trailers, gRPC-Web encodes trailers as the final message envelope in the response body — flagged with `0x80` and containing an HTTP/1-style header block. All messages (including data and trailers) use the same 5-byte envelope framing. This makes gRPC-Web compatible with any HTTP/1.1 proxy, CDN, or load balancer, at the cost of requiring the client to parse the full response body before accessing trailers.

## Content-Type Routing

```
gRPC-Web proto:  application/grpc-web
gRPC-Web codec:  application/grpc-web+{codec}   e.g., application/grpc-web+proto, application/grpc-web+json
gRPC-Web text:   application/grpc-web-text       (base64-encoded entire body)
```

The server derives the codec name from the content type:
- If content-type is exactly `application/grpc-web` → codec is `"proto"` (implicit bare mapping)
- If content-type starts with `application/grpc-web+` → codec is `strings.TrimPrefix(contentType, "application/grpc-web+")`
- If content-type is `application/grpc-web-text` → codec is `"proto"`, body is base64-encoded

**Aha:** The `application/grpc-web-text` content type is unique to gRPC-Web. It base64-encodes the entire response body (including the trailer envelope), enabling gRPC-Web to work with HTTP/1.1 intermediaries that might corrupt binary data. The client must base64-decode the entire body before parsing envelopes.

## HTTP Method and Version

| RPC Kind | Method | HTTP Version |
|----------|--------|-------------|
| Unary | POST | HTTP/1.1 or HTTP/2 |
| Client Stream | POST | HTTP/1.1 or HTTP/2 |
| Server Stream | POST | HTTP/1.1 or HTTP/2 (requires chunked transfer or streaming) |
| Bidi Stream | POST | HTTP/2 recommended (HTTP/1.1 half-duplex limits true bidi) |

**Key advantage over gRPC:** gRPC-Web works over HTTP/1.1, making it compatible with browsers, older proxies, and CDNs that don't support HTTP/2. However, true bidirectional streaming is limited on HTTP/1.1 due to half-duplex constraints.

## Required Headers

### Client → Server

| Header | Value | Source |
|--------|-------|--------|
| `Content-Type` | `application/grpc-web` or `application/grpc-web+{codec}` | Derived from codec name |
| `User-Agent` | `connect-go/{version} ({goVersion})` or custom | `protocol_grpc.go:237` |
| `X-User-Agent` | Same as User-Agent | Only for gRPC-Web (`protocol_grpc.go:300`) |
| `Grpc-Timeout` | `<digits><unit>` | Optional — same format as gRPC |
| `Grpc-Encoding` | Compression name | Only if request is compressed |
| `Grpc-Accept-Encoding` | Comma-separated compression names | From registered pools |

**Aha:** gRPC-Web sets both `User-Agent` and `X-User-Agent` headers. The `X-User-Agent` is a legacy requirement from the original gRPC-Web spec — some gRPC-Web proxies (like Envoy) inspect this header. Even when implementing natively without a proxy, setting both headers ensures compatibility with the broader gRPC-Web ecosystem.

### Server → Client

| Header | Value | Condition |
|--------|-------|-----------|
| `Content-Type` | `application/grpc-web` or `application/grpc-web+{codec}` | Always |
| `Grpc-Encoding` | Compression name | Only if response compressed |
| `Grpc-Accept-Encoding` | Comma-separated names | Always |

Unlike gRPC over HTTP/2, gRPC-Web does NOT send trailers as HTTP headers. All trailers go in the response body.

## Envelope Framing

Every gRPC-Web message (data and trailers) uses the 5-byte envelope format:

```
+--------+--------+--------+--------+--------+
| Flags  |         Length (uint32, BE)       |
| 1 byte |           4 bytes                 |
+--------+--------+--------+--------+--------+
|              Payload (Length bytes)         |
+---------------------------------------------+
```

### Flags

| Flag | Value | Meaning |
|------|-------|---------|
| Compressed | `0x01` | Payload is compressed |
| Trailer | `0x80` | Final message containing trailers (gRPC-Web specific) |

**Aha:** The gRPC-Web trailer flag (`0x80`) is completely different from Connect's end-stream flag (`0x02`). Connect's `0x02` envelope carries a JSON object with both error and metadata. gRPC-Web's `0x80` envelope carries a raw HTTP/1-style header block (key: value lines) that must be parsed with a MIME header reader. This is the single most important difference between the two protocols.

## Streaming Server → Client Flow

```mermaid
flowchart TD
    H1["Handler calls Send(msg)"]
    H2["Marshal msg → bytes"]
    H3["Compress if > compressMinBytes"]
    H4["Write 5-byte prefix (flag 0x00 or 0x01) + payload"]
    H5["Flush (http.Flusher required)"]

    E1["Handler returns (or errors)"]
    E2["Build trailer headers: grpc-status, grpc-message, grpc-status-details-bin"]
    E3["Format as HTTP/1 header block (key: value\\n)"]
    E4["Write envelope with flag 0x80"]
    E5["Flush and close"]

    H1 --> H2 --> H3 --> H4 --> H5
    E1 --> E2 --> E3 --> E4 --> E5
```

## Trailer Handling: Body-Encoded

gRPC-Web encodes trailers as the final envelope in the response body:

```go
// protocol_grpc.go:579
func (m *grpcMarshaler) MarshalWebTrailers(trailer http.Header) *Error {
    // Lowercase all header keys (gRPC-Web spec requires lowercase keys)
    for key, values := range trailer {
        lower := strings.ToLower(key)
        if key != lower {
            delete(trailer, key)
            trailer[lower] = values
        }
    }
    // Write as HTTP/1 headers block (without terminating newline)
    trailer.Write(raw)
    return m.Write(&envelope{
        Data:  raw,
        Flags: grpcFlagEnvelopeTrailer,  // 0x80
    })
}
```

**Key considerations:**
1. All trailer header keys must be lowercased. This differs from gRPC over HTTP/2 which preserves canonical case.
2. The header block is written without a terminating newline (per the gRPC-Web spec).
3. The header block format is `key: value\r\n` per line, like HTTP/1 headers.

### Client-Side Parsing

```go
// protocol_grpc.go:609
func (u *grpcUnmarshaler) Unmarshal(message any) *Error {
    if !u.web || !env.IsSet(grpcFlagEnvelopeTrailer) {
        return errorf(CodeInternal, "protocol error: invalid envelope flags %d", env.Flags)
    }
    // Add newline to make it parseable by textproto
    data.WriteByte('\n')
    mimeHeader, _ := textproto.NewReader(bufio.NewReader(data)).ReadMIMEHeader()
    u.webTrailer = http.Header(mimeHeader)
}
```

**Aha:** The gRPC-Web trailer encoding adds a `'\n'` byte because `textproto.Reader.ReadMIMEHeader()` expects a blank line terminating the header block. The original trailer data doesn't include the terminating newline (per the gRPC-Web spec), so the reader must add it. Without this newline, the MIME header parser hangs waiting for the terminator.

## Trailers-Only Optimization

```go
// protocol_grpc.go:537
if hc.web && !hc.wroteToBody && len(hc.responseHeader) == 0 {
    // gRPC-Web: send trailers as HTTP headers instead of body
    mergeHeaders(hc.responseWriter.Header(), mergedTrailers)
    return nil
}
```

For gRPC-Web, if no body has been written and there are no custom response headers, trailers are sent as regular HTTP headers instead of a body envelope. This is the "trailers-only" response — useful for unary RPCs that return an error without any response body.

**Aha:** This optimization means a client must check both HTTP headers (for trailers-only) AND the response body (for body-encoded trailers) to find the `grpc-status`. A robust client implementation checks headers first, then falls back to parsing the body envelope if no status header is found.

## Error Serialization in Trailers

```go
// protocol_grpc.go:841
func grpcErrorToTrailer(trailer http.Header, protobuf Codec, err error) {
    if err == nil {
        setHeaderCanonical(trailer, grpcHeaderStatus, "0")  // OK
        return
    }
    // Merge custom metadata (unless wire error)
    if connectErr, ok := asError(err); ok && !connectErr.wireErr {
        mergeNonProtocolHeaders(trailer, connectErr.meta)
    }
    status := grpcStatusForError(err)
    code := status.GetCode()
    message := status.GetMessage()

    // Serialize details as protobuf Status
    if len(status.Details) > 0 {
        bin, _ := protobuf.Marshal(status)
        setHeaderCanonical(trailer, grpcHeaderDetails, EncodeBinaryHeader(bin))
    }
    setHeaderCanonical(trailer, grpcHeaderStatus, strconv.Itoa(int(code)))
    setHeaderCanonical(trailer, grpcHeaderMessage, grpcPercentEncode(message))
}
```

The error serialization is identical to gRPC over HTTP/2 — the same `grpc-status`, `grpc-message`, and `grpc-status-details-bin` headers are built, then encoded into the body envelope via `MarshalWebTrailers`.

## Error Parsing from Trailers (Client Side)

```go
// protocol_grpc.go:692
func grpcErrorForTrailer(protobuf Codec, trailer http.Header) *Error {
    codeHeader := getHeaderCanonical(trailer, grpcHeaderStatus)
    if codeHeader == "" {
        code := CodeInternal
        if len(trailer) == 0 { code = CodeUnknown }
        return NewError(code, errTrailersWithoutGRPCStatus)
    }
    if codeHeader == "0" { return nil }  // OK

    code, _ := strconv.ParseUint(codeHeader, 10, 32)
    message, _ := grpcPercentDecode(getHeaderCanonical(trailer, grpcHeaderMessage))
    retErr := NewWireError(Code(code), errors.New(message))

    // Parse protobuf error details from grpc-status-details-bin
    detailsBinaryEncoded := getHeaderCanonical(trailer, grpcHeaderDetails)
    if len(detailsBinaryEncoded) > 0 {
        detailsBinary, _ := DecodeBinaryHeader(detailsBinaryEncoded)
        var status statusv1.Status
        protobuf.Unmarshal(detailsBinary, &status)
        for _, d := range status.GetDetails() {
            retErr.details = append(retErr.details, &ErrorDetail{pbAny: d})
        }
        // Prefer protobuf data over header values
        retErr.code = Code(status.GetCode())
        retErr.err = errors.New(status.GetMessage())
    }
    return retErr
}
```

## Percent Encoding for Grpc-Message

```go
// protocol_grpc.go:895
func grpcPercentEncode(msg string) string {
    // Characters that need escaping: control chars (< ' ' or > '~') and '%'
    func grpcShouldEscape(char byte) bool {
        return char < ' ' || char > '~' || char == '%'
    }
    // Two-pass: count escapes first, then encode with uppercase hex
    for i := range len(msg) {
        if grpcShouldEscape(msg[i]) { hexCount++ }
    }
    // Encode with uppercase hex (A-F, not a-f)
    out.WriteByte('%')
    out.WriteByte(upperhex[char>>4])  // "0123456789ABCDEF"
    out.WriteByte(upperhex[char&15])
}
```

**Aha:** gRPC-Web uses the same custom percent-encoding as gRPC over HTTP/2. Only control characters (ASCII < 32 or > 126) and `%` itself are escaped. Hex digits use uppercase. This is shared code between the two protocols — implementing one means you already have the encoding for the other.

## Timeout Encoding

gRPC-Web uses the same timeout format as gRPC:

```
Grpc-Timeout: <digits><unit>  // max 8 digits, units: H/M/S/m/u/n
```

See the gRPC protocol implementation guide for the full parsing and encoding logic. The key difference from Connect: gRPC-Web uses the unit-suffixed format (max 8 digits), not plain milliseconds (max 10 digits).

## Compression Negotiation

gRPC-Web uses the same compression negotiation as gRPC over HTTP/2:

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

Both gRPC and gRPC-Web use `Grpc-Encoding` and `Grpc-Accept-Encoding` headers — not the standard HTTP `Content-Encoding`/`Accept-Encoding`.

## Response Validation

```go
// protocol_grpc.go:649
func grpcValidateResponse(response *http.Response, header http.Header,
    availableCompressors readOnlyCompressionPools, web bool, codecName string) *Error {
    if response.StatusCode != http.StatusOK {
        return errorf(httpToCode(response.StatusCode), "HTTP status %v", response.Status)
    }
    // Validate content-type matches request codec
    if err := grpcValidateResponseContentType(web, codecName, contentType); err != nil {
        return err
    }
    // Validate compression
    if compression != "" && compression != compressionIdentity &&
        !availableCompressors.Contains(compression) {
        return errorf(CodeInternal, "unknown encoding %q: accepted encodings are %v", compression, ...)
    }
}
```

gRPC-Web expects HTTP 200 for all responses, just like gRPC over HTTP/2. Non-200 indicates a transport-level failure.

## Text Protocol Mode (base64 Body)

When the client requests `application/grpc-web-text`, the entire response body is base64-encoded:

```go
// Server side: encode the entire body (all envelopes) as base64
// Client side: decode the body before parsing envelopes
```

This mode exists because some HTTP/1.1 intermediaries corrupt binary data. Base64 encoding ensures the envelope framing bytes survive intact through any HTTP/1.1 proxy. The tradeoff is a 33% size increase.

**Key consideration:** The base64 encoding applies to the ENTIRE response body — all data envelopes plus the final trailer envelope. The client must decode the full body, then parse envelopes normally.

## Client Request Flow (Browser Compatible)

```mermaid
sequenceDiagram
    participant B as Browser
    participant S as Server

    B->>S: POST /service/Method HTTP/1.1
    Note over B,S: Content-Type: application/grpc-web+proto
    Note over B,S: X-User-Agent: grpc-web-javascript/0.1
    Note over B,S: Grpc-Timeout: 500m
    Note over B,S: [body: 5-byte envelope + message]

    S->>S: Parse timeout → context.WithTimeout
    S->>S: Decompress if Grpc-Encoding present
    S->>S: Unmarshal request via codec

    S->>S: Execute handler logic

    alt Success (streaming)
        S->>B: HTTP 200 OK
        Note over S,B: Content-Type: application/grpc-web+proto
        Note over S,B: [body: 5-byte envelope + message]
        Note over S,B: [body: 5-byte envelope + message]
        Note over S,B: [body: 5-byte envelope (flag 0x80) + trailer block]
    else Error (trailers-only)
        S->>B: HTTP 200 OK
        Note over S,B: Content-Type: application/grpc-web+proto
        Note over S,B: [HTTP headers]
        Note over S,B: grpc-status: 5
        Note over S,B: grpc-message: user%20not%20found
        Note over S,B: [body: empty]
    end
```

## Compatibility Validation Checklist

1. **Content-Type**: Server must return `application/grpc-web` or `application/grpc-web+{codec}` matching the request codec. Bare `application/grpc-web` maps to proto.
2. **HTTP Status**: All responses must be HTTP 200. Non-200 indicates transport-level failure.
3. **Trailers in body**: Response must end with an envelope flagged `0x80` containing the trailer header block. Exception: trailers-only responses send trailers as HTTP headers.
4. **Trailer header keys**: Must be lowercased in the body envelope (gRPC-Web spec requirement).
5. **Trailer parsing**: Client must add `'\n'` before parsing with `textproto.Reader.ReadMIMEHeader()`.
6. **grpc-status**: Must always be present (either in HTTP headers for trailers-only, or in body envelope). Missing → `CodeInternal`.
7. **grpc-message encoding**: Must use custom percent-encoding (control chars + `%`, uppercase hex).
8. **grpc-status-details-bin**: Must be base64-encoded protobuf `google.rpc.Status` message.
9. **X-User-Agent**: Client should set both `User-Agent` and `X-User-Agent` for ecosystem compatibility.
10. **Timeout format**: Must be `<digits><unit>` with max 8 digits. Units: H/M/S/m/u/n.
11. **Compression headers**: Must use `Grpc-Encoding`/`Grpc-Accept-Encoding`, not standard HTTP headers.
12. **Flush**: Server must flush after each streaming message (`http.Flusher`).
13. **Text mode**: When content-type is `application/grpc-web-text`, the entire body must be base64-encoded.
14. **HTTP/1.1 compatibility**: Must work without HTTP/2 features (no native trailers, no multiplexing).

## Conformance Testing

The Go implementation includes a conformance test harness (`conformance/` directory) that exercises:
- All three protocols (Connect, gRPC, gRPC-Web)
- All four RPC kinds (unary, client stream, server stream, bidi)
- All error codes
- Compression (gzip)
- Timeout/deadline semantics
- Cancellation

An implementation should pass the connectrpc/conformance test suite to claim compatibility.
