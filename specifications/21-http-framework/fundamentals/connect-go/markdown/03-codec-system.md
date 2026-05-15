# connect-go — Codec System

**Source:** `codec.go` (260 LOC). The `Codec` interface abstracts message serialization with two concrete implementations (`protoBinaryCodec`, `protoJSONCodec`) and two extension interfaces (`marshalAppender`, `stableCodec`) for performance and GET request support.

## Core Codec Interface

```go
// codec.go:34
type Codec interface {
    // Name returns the codec name, used in Content-Type subtypes
    // e.g., "application/grpc+proto" → codec name "proto"
    Name() string

    // Marshal marshals a message to bytes
    Marshal(any) ([]byte, error)

    // Unmarshal unmarshals bytes into a message
    Unmarshal([]byte, any) error
}
```

The `Name()` value appears in HTTP `Content-Type` subtypes. `application/grpc+proto` maps to a codec named `"proto"`.

## Extension Interfaces

```go
// codec.go:56 — zero-copy append path
type marshalAppender interface {
    Codec
    MarshalAppend([]byte, any) ([]byte, error)
}

// codec.go:68 — deterministic output for GET caching
type stableCodec interface {
    Codec
    MarshalStable(any) ([]byte, error)
    IsBinary() bool
}
```

**Aha:** Go's interface embedding makes these "optional capabilities" — a codec can implement `Codec` alone, or opt into `marshalAppender` for zero-copy serialization, or `stableCodec` for deterministic output. The `envelopeWriter` checks `if appender, ok := w.codec.(marshalAppender)` at runtime and uses the optimized path when available. This is more flexible than a monolithic interface with optional methods that return "not implemented" errors.

## protoBinaryCodec

```go
// codec.go:94
type protoBinaryCodec struct{}

func (c *protoBinaryCodec) Name() string { return codecNameProto }  // "proto"

func (c *protoBinaryCodec) Marshal(message any) ([]byte, error) {
    protoMessage, ok := message.(proto.Message)
    if !ok { return nil, errNotProto(message) }
    return proto.Marshal(protoMessage)
}

func (c *protoBinaryCodec) MarshalAppend(dst []byte, message any) ([]byte, error) {
    protoMessage, ok := message.(proto.Message)
    if !ok { return nil, errNotProto(message) }
    return proto.MarshalOptions{}.MarshalAppend(dst, protoMessage)
}

func (c *protoBinaryCodec) Unmarshal(data []byte, message any) error {
    protoMessage, ok := message.(proto.Message)
    if !ok { return errNotProto(message) }
    return proto.Unmarshal(data, protoMessage)
}

func (c *protoBinaryCodec) MarshalStable(message any) ([]byte, error) {
    // Deterministic: field ordering by tag number, not map iteration
    options := proto.MarshalOptions{Deterministic: true}
    return options.Marshal(protoMessage)
}

func (c *protoBinaryCodec) IsBinary() bool { return true }
```

Implements all three interfaces: `Codec`, `marshalAppender`, and `stableCodec`. `MarshalStable` uses `Deterministic: true` for consistent field ordering (needed for GET request caching).

## protoJSONCodec

```go
// codec.go:146
type protoJSONCodec struct {
    name string  // "json" or "json; charset=utf-8"
}

func (c *protoJSONCodec) Name() string { return c.name }

func (c *protoJSONCodec) Marshal(message any) ([]byte, error) {
    return protojson.MarshalOptions{}.Marshal(protoMessage)
}

func (c *protoJSONCodec) MarshalAppend(dst []byte, message any) ([]byte, error) {
    return protojson.MarshalOptions{}.MarshalAppend(dst, protoMessage)
}

func (c *protoJSONCodec) Unmarshal(binary []byte, message any) error {
    if len(binary) == 0 {
        return errors.New("zero-length payload is not a valid JSON object")
    }
    options := protojson.UnmarshalOptions{DiscardUnknown: true}
    return options.Unmarshal(binary, protoMessage)
}

func (c *protoJSONCodec) MarshalStable(message any) ([]byte, error) {
    // Compact JSON for stable whitespace output
    messageJSON, err := c.Marshal(message)
    compactedJSON := bytes.NewBuffer(messageJSON[:0])
    json.Compact(compactedJSON, messageJSON)
    return compactedJSON.Bytes(), nil
}

func (c *protoJSONCodec) IsBinary() bool { return false }
```

**Key design choice:** `DiscardUnknown: true` on unmarshal. This means clients and servers don't need to use exactly the same version of the `.proto` schema — new fields added server-side are silently ignored by older clients. This is critical for rolling deployments.

## Codec Constants

```go
// codec.go:28
const (
    codecNameProto           = "proto"
    codecNameJSON            = "json"
    codecNameJSONCharsetUTF8 = codecNameJSON + "; charset=utf-8"
)
```

Three codec names are registered by default. The `json` and `json; charset=utf-8` variants are treated as equivalent for routing purposes.

## readOnlyCodecs Interface

```go
// codec.go:209
type readOnlyCodecs interface {
    Get(name string) Codec           // Get codec by name
    Protobuf() Codec                 // Fallback protobuf codec (always available)
    Names() []string                 // List all registered codec names
}
```

The `codecMap` implementation (`codec.go:231`) wraps a `map[string]Codec`. `Protobuf()` returns the user-supplied `proto` codec if registered, falling back to a default `protoBinaryCodec`.

```go
// codec.go:239
func (m *codecMap) Protobuf() Codec {
    if pb, ok := m.nameToCodec[codecNameProto]; ok {
        return pb
    }
    return &protoBinaryCodec{}
}
```

This fallback is critical for gRPC — the wire protocol always needs access to a protobuf codec for error serialization (gRPC errors use protobuf `Status` messages, even when the main codec is JSON).

## errNotProto — ProtoV1 Detection

```go
// codec.go:254
func errNotProto(message any) error {
    if _, ok := message.(protoiface.MessageV1); ok {
        return fmt.Errorf("%T uses github.com/golang/protobuf, but connect-go only supports google.golang.org/protobuf: see https://go.dev/blog/protobuf-apiv2", message)
    }
    return fmt.Errorf("%T doesn't implement proto.Message", message)
}
```

Detects the legacy `github.com/golang/protobuf` (ProtoV1) and provides a clear error message pointing users to the V2 migration guide.

## Codec Usage in Protocol Handlers

The handler config registers codecs via options:

```go
// handler.go:366
withProtoBinaryCodec().applyToHandler(&config)
withProtoJSONCodecs().applyToHandler(&config)
```

Both proto and JSON codecs are registered by default. Each protocol handler then builds a `readOnlyCodecs` view:

```go
// handler.go:391
codecs := newReadOnlyCodecs(c.Codecs)
```

The codec is selected per-request based on the `Content-Type` header.

## Next

[04-error-handling.md](04-error-handling.md) — Error wrapping chains, RST_STREAM mapping, wire errors, and error details.
