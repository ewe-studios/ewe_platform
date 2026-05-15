# connect-go — Interceptor Architecture

**Source:** `interceptor.go` (141 LOC). Interceptors are middleware that wrap unary, streaming client, and streaming handler RPCs. They can mutate requests/responses, handle errors, retry, recover from panics, emit logs and metrics — anything that needs to happen before or after an RPC call.

## Interceptor Interface

```go
// interceptor.go:52
type Interceptor interface {
    WrapUnary(UnaryFunc) UnaryFunc
    WrapStreamingClient(StreamingClientFunc) StreamingClientFunc
    WrapStreamingHandler(StreamingHandlerFunc) StreamingHandlerFunc
}

// The function types being wrapped:
type UnaryFunc func(context.Context, AnyRequest) (AnyResponse, error)
type StreamingClientFunc func(context.Context, Spec) StreamingClientConn
type StreamingHandlerFunc func(context.Context, StreamingHandlerConn) error
```

Each method takes the "next" function in the chain and returns a wrapped version. This is the classic decorator pattern — each interceptor wraps the next one, like onion layers.

## UnaryInterceptorFunc — Simple Implementation

```go
// interceptor.go:58
type UnaryInterceptorFunc func(UnaryFunc) UnaryFunc

func (f UnaryInterceptorFunc) WrapUnary(next UnaryFunc) UnaryFunc { return f(next) }
func (f UnaryInterceptorFunc) WrapStreamingClient(next StreamingClientFunc) StreamingClientFunc { return next }
func (f UnaryInterceptorFunc) WrapStreamingHandler(next StreamingHandlerFunc) StreamingHandlerFunc { return next }
```

A `UnaryInterceptorFunc` is a function that only wraps unary RPCs. Its `WrapStreamingClient` and `WrapStreamingHandler` methods are no-ops — it passes streaming RPCs through unchanged. This is convenient for interceptors that only care about request-response calls (e.g., a metrics interceptor that only tracks unary latencies).

## Chain Composition

```go
// interceptor.go:76
type chain struct {
    interceptors []Interceptor
}

// interceptor.go:81
func newChain(interceptors []Interceptor) *chain {
    var chain chain
    // Reverse order: first interceptor in slice acts first
    for i := len(interceptors) - 1; i >= 0; i-- {
        if interceptor := interceptors[i]; interceptor != nil {
            chain.interceptors = append(chain.interceptors, interceptor)
        }
    }
    return &chain
}
```

**Aha:** Interceptors are composed in **reverse order**. If you register `[A, B, C]`, the chain reverses them to `[C, B, A]`. When `WrapUnary` iterates through the chain, each interceptor wraps the result of the previous wrap. The net effect is: `C(B(A(next)))` — meaning `A` executes first (outermost), then `B`, then `C` (innermost, closest to the actual RPC). This matches the intuitive "first registered, first executed" model.

## WrapUnary — With Sentinel Check

```go
// interceptor.go:94
func (c *chain) WrapUnary(next UnaryFunc) UnaryFunc {
    for _, interceptor := range c.interceptors {
        next = unaryThunk(next)           // add sentinel check
        next = interceptor.WrapUnary(next)  // wrap with interceptor
    }
    return next
}
```

Each iteration:
1. Wraps `next` with `unaryThunk` (sentinel context check).
2. Passes the wrapped function to the interceptor's `WrapUnary`.

The result is a deeply nested chain where each layer has both the sentinel check and the interceptor logic.

## Sentinel Context Checks

```go
// interceptor.go:117
func unaryThunk(next UnaryFunc) UnaryFunc {
    return func(ctx context.Context, req AnyRequest) (AnyResponse, error) {
        if err := checkSentinel(ctx); err != nil {
            return nil, err
        }
        return next(ctx, req)
    }
}

// interceptor.go:135
func checkSentinel(ctx context.Context) error {
    if ctx.Value(clientCallInfoContextKey{}) != ctx.Value(sentinelContextKey{}) {
        return errNewClientContextProhibited
    }
    return nil
}
```

**Aha:** The sentinel context check prevents interceptors from replacing the context. Here's how it works:

1. Before calling the interceptor chain, the client stores a `callInfo` struct in the context via `context.WithValue(ctx, sentinelContextKey{}, callInfo)`.
2. The `sentinelContextKey{}` and `clientCallInfoContextKey{}` are distinct types — so `ctx.Value(sentinelContextKey{})` returns the `callInfo` value.
3. Each `unaryThunk` checks that these two values are still equal.
4. If an interceptor creates a new context (e.g., `context.WithTimeout` or `context.WithValue`), the sentinel value is lost — the check fails.

This protects against subtle bugs where an interceptor's context replacement would discard the client's call info (peer, spec, headers). Interceptors that need to modify the context must do so through the `CallInfo` mechanism rather than replacing the context entirely.

## Where Interceptors Are Applied

### Server-Side: Handler Creation

```go
// handler.go:64 (unary)
if interceptor := config.Interceptor; interceptor != nil {
    untyped = interceptor.WrapUnary(untyped)
}

// handler.go:416 (streaming)
if ic := config.Interceptor; ic != nil {
    implementation = ic.WrapStreamingHandler(implementation)
}
```

### Client-Side: Client Creation

```go
// client.go:107 (unary)
if interceptor := config.Interceptor; interceptor != nil {
    unaryFunc = interceptor.WrapUnary(unaryFunc)
}

// client.go:285 (streaming client)
if interceptor := c.config.Interceptor; interceptor != nil {
    newConn = interceptor.WrapStreamingClient(newConn)
}
```

The interceptor chain is applied once at creation time, not on every call. This avoids the overhead of rebuilding the chain for each RPC.

## streamingClientThunk

```go
// interceptor.go:126
func streamingClientThunk(next StreamingClientFunc) StreamingClientFunc {
    return func(ctx context.Context, spec Spec) StreamingClientConn {
        if err := checkSentinel(ctx); err != nil {
            return &errStreamingClientConn{err: err}
        }
        return next(ctx, spec)
    }
}
```

For streaming client RPCs, the sentinel check returns an `errStreamingClientConn` — a dummy connection that immediately returns the error on every operation. This ensures the sentinel failure propagates through the streaming API rather than panicking.

## No Sentinel on Handler Side

Note that `WrapStreamingHandler` (`interceptor.go:110`) does **not** use a thunk:

```go
func (c *chain) WrapStreamingHandler(next StreamingHandlerFunc) StreamingHandlerFunc {
    for _, interceptor := range next {
        next = interceptor.WrapStreamingHandler(next)
    }
    return next
}
```

This is because server-side handlers receive the context from the HTTP request — they don't need the same protection as the client-side where the context carries call info set up during client creation.

## Next

[08-compression-buffers.md](08-compression-buffers.md) — Compression pools, buffer pooling, and compression negotiation.
