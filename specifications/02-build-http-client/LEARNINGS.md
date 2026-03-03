# Learnings: 02-build-http-client

## Critical Implementation Details
- Do NOT use async/await, tokio, or async runtimes for this client code or testing. Follow Rust Clean Code guidance for synchronous implementations only (see .agents/skills/rust-clean-code/implementation/skill.md).
- Always prefer project and stdlib building blocks before adding any external crate.

## Common Failures and Fixes
- Including async or tokio primitives leads to spec non-compliance; remove and refactor to sync patterns if found.
- Forgetting panics documentation: all public functions must specify # Panics if needed.
- Trying to enable a nonexistent “sync” feature or run with --features sync is incorrect; project is strictly synchronous by default, no gate or feature toggle. Synchronous is the enforced norm.
- The foundational Cargo feature in this project is “std.” If you need to pass a --features flag, it should be --features std, not sync.

## Testing Insights
- TestHttpServer::redirect_chain helper makes writing sequential redirect tests much simpler and reusable. Use it for any test exercising redirect chains with different codes or locations.
- Key redirect cases to cover in integration tests:
  - Mix of relative/absolute URLs in Location
  - Host changes: ensure sensitive headers (e.g., Authorization, Cookie) are stripped after cross-origin
  - POST→GET method switch for 303 (and some 302)
  - Method/body preservation for 307/308
  - Redirect loop detection and error if chain exceeds max_redirects
  - Non-redirect 3xx (e.g., 305, 306, 304) should NOT trigger another request
  - Edge cases: empty Location, invalid schemes, large chains, chunked encoding

- This project uses a dedicated test crate: ewe_platform_tests (defined in /tests/Cargo.toml). The crate aggregates and executes integration test modules, including http_redirect_integration.rs, from the /tests hierarchy.
- To run integration tests correctly, invoke cargo test --package ewe_platform_tests (add --features std if needed), ensuring the test crate discovers and runs all properly registered test modules.
- Test module discovery relies on mod.rs files registering test modules within the crate hierarchy. Verify mod.rs includes mod http_redirect_integration; or similar for all intended test files.
- Integration tests should not rely on async test setups. Use only synchronous test helpers from test infrastructure.
- Test runners should be invoked as-is; do not attempt to run or toggle a “sync” feature for this project—there is none. Only “std” (for standard library support) and other actual features in Cargo.toml are permitted.

- Compression feature gates: flate2 provides both gzip and deflate decoders. Use #[cfg(any(feature = “gzip”, feature = “deflate”))] for DeflateDecoder imports and usage to avoid unused import warnings when only one feature enabled.
- Brotli decompressor needs buffer size parameter (4096 works well): BrotliDecoder::new(inner, 4096)

## Dependencies and Interactions
- This project is strictly synchronous per stack standards; async/await dependencies must be disallowed at review.

## Future Considerations
- If async HTTP support is needed, a separate feature and architectural track must be proposed and accepted before any work.

---

## HTTP Client Proxy Support Implementation (2026-03-03)

### 1. Multiple Connection Paths - Check ALL Code Paths

**Key Insight**: When implementing infrastructure features (proxy, auth, timeouts), always trace through ALL connection creation paths, not just the obvious ones.

**What Happened**:
- Initial implementation added proxy support to `request_stream.rs` (simple request path)
- User correctly identified missing support in `request_redirect.rs` (redirect path)
- This would have caused redirected requests to bypass proxy configuration

**Critical Discovery**: HTTP client has multiple connection creation paths:
1. **Direct requests**: `GetHttpRequestStreamTask` → uses pool connection
2. **Redirect requests**: `GetHttpRequestRedirectTask` → creates new connections for redirects
3. Both paths must support the same features (proxy, timeouts, auth, etc.)

**Architecture Pattern**:
```rust
// Task state tuples must carry ClientConfig through entire chain:
type InitData = Box<(Request, Timeout, Pool, MaxRedirects, Config)>;
type TryingData = Box<(Request, Timeout, Pool, Descriptor, Redirects, Config)>;

// Both simple and redirect tasks need config parameter:
GetHttpRequestStreamTask::new(request, timeout, pool, config)
GetHttpRequestRedirectTask::new(request, timeout, pool, redirects, config)
```

**Best Practice**: When adding infrastructure features:
1. Grep for ALL connection creation points (`create_http_connection`, `create_https_connection`)
2. Check both simple request AND redirect task implementations
3. Verify state tuples carry necessary context through entire state machine
4. Run validation to confirm completeness across all code paths

### 2. Working with Existing Patterns - Don't Fight the Architecture

**Key Insight**: When the architecture uses specific patterns (Connection → RawStream → SharedByteBufferStream), work WITH those patterns, not against them.

**What Happened**:
- Initially tried complex workarounds to extract `Connection` from wrapped types
- User repeatedly said “don't be stupid” - signal to reconsider approach
- Solution: Use `Connection` directly from start, wrap only when needed

**Wrong Approach**:
```rust
// Trying to extract Connection from HttpClientConnection
let connection = http_conn.stream.into_inner().into_connection()  // Doesn't exist!
```

**Right Approach**:
```rust
// Work with Connection directly
let connection = Connection::with_timeout(addr, timeout)?;
// Wrap temporarily for I/O operations
let stream = SharedByteBufferStream::rwrite(RawStream::from_connection(connection.try_clone()?)?);
// Return the original Connection
Ok(connection)
```

**Lesson**: When you're fighting the type system or architecture:
1. Step back and look at existing patterns
2. Check what other similar code does (e.g., `upgrade_to_tls` takes Connection, not HttpClientConnection)
3. Ask “what does the caller actually need?” (Connection for TLS upgrade, not a wrapper)

### 3. Proxy Connection Return Types - Different Protocols Need Different Approaches

**Key Insight**: HTTP proxy and HTTPS proxy have fundamentally different return types based on their security models.

**Architecture Decision**:
```rust
// HTTP proxy: Returns plain Connection (TCP tunnel)
fn connect_via_http_proxy() -> Result<Connection, HttpClientError>

// HTTPS proxy: Returns HttpClientConnection (TLS to proxy)
fn connect_via_https_proxy() -> Result<HttpClientConnection, HttpClientError>
```

**Rationale**:
- **HTTP proxy**: Plain TCP tunnel → can be upgraded to TLS for HTTPS targets → return `Connection`
- **HTTPS proxy**: TLS to proxy already established → can't extract `Connection` → return wrapped `HttpClientConnection`
- **HTTPS target through HTTPS proxy**: Would need double-TLS (not yet supported)

**Pattern for Handling**:
```rust
match proxy_config.protocol {
    ProxyProtocol::Http => {
        let conn = pool.connect_via_http_proxy(...)?;
        if target_is_https {
            HttpClientConnection::upgrade_to_tls(conn, host, port)  // Upgrade tunnel
        } else {
            wrap_connection(conn)  // Wrap plain tunnel
        }
    }
    ProxyProtocol::Https => {
        let http_conn = pool.connect_via_https_proxy(...)?;
        if target_is_https {
            return Err(NotSupported);  // Double-TLS not supported
        }
        Ok(http_conn)  // Already wrapped with TLS to proxy
    }
}
```

### 4. Feature Completeness Validation - Always Run Comprehensive Checks

**Key Insight**: Features may LOOK complete but have subtle gaps. Validation agent provides comprehensive verification.

**Validation Results** (65 tests across 3 features):
- ✅ Cookie-Jar: 12 tests - RFC 6265 compliance verified
- ✅ Middleware: 18 tests - Thread safety (Send + Sync) verified
- ✅ Proxy-Support: 35 tests - Environment detection, NO_PROXY bypass verified

**What Validation Caught**:
- All implementations complete with no TODOs/stubs
- Documentation complete (WHY/WHAT/HOW patterns throughout)
- Code quality verified (fmt, clippy passing)
- Integration verified (all exports present in mod.rs)
- Feature specifications needed status updates to “completed”

**Best Practice**: After implementing a feature:
1. Run comprehensive test suite (not just unit tests)
2. Check for TODOs, FIXMEs, `unimplemented!()`, `todo!()`
3. Verify documentation completeness
4. Run `cargo fmt --check` and `cargo clippy`
5. Update feature specifications to reflect completion status
6. Use validation agent for final sign-off

### 5. TLS Upgrade Pattern - Reuse Existing Infrastructure

**Key Insight**: Don't reinvent TLS upgrade logic - reuse the existing `upgrade_to_tls` method.

**What We Had**:
```rust
// Existing method that does TLS handshake
impl HttpClientConnection {
    fn upgrade_to_tls(
        connection: Connection,  // Takes plain Connection!
        host: &str,
        port: u16,
    ) -> Result<Self, HttpClientError>
}
```

**What We Did**:
```rust
// HTTP proxy returns Connection → perfect for upgrade_to_tls!
let tunnel_connection = pool.connect_via_http_proxy(...)?;
if url.is_https() {
    HttpClientConnection::upgrade_to_tls(tunnel_connection, host, port)
} else {
    wrap_plain_connection(tunnel_connection)
}
```

**Lesson**: Before implementing complex logic:
1. Grep for similar functionality (`upgrade_to_tls`, `create_https_connection`)
2. Check what parameters existing methods take (guides your return types)
3. Reuse existing infrastructure rather than duplicating logic

### 6. Environment Variable Patterns - Case-Insensitive with Fallback

**Key Insight**: Follow Unix conventions for environment variables (uppercase preferred, lowercase fallback).

**Implementation Pattern**:
```rust
pub fn from_env(scheme: &Scheme) -> Option<Self> {
    if scheme.is_http() {
        Self::from_env_var(“HTTP_PROXY”)
            .or_else(|| Self::from_env_var(“http_proxy”))  // Fallback
    } else {
        Self::from_env_var(“HTTPS_PROXY”)
            .or_else(|| Self::from_env_var(“https_proxy”))
    }
}
```

**NO_PROXY Bypass Logic**:
```rust
// Support multiple patterns:
- Wildcard: “*” matches everything
- Exact match: “localhost” matches “localhost”
- Suffix with dot: “.example.com” matches “api.example.com”
- Suffix without dot: “example.com” matches “api.example.com” AND “example.com”
```

**Tests**: Comprehensive environment variable tests with `#[serial]` attribute to prevent race conditions.

### 7. State Machine Configuration - Carry Context Through All States

**Key Insight**: State machines need configuration available in ALL states, not just init.

**Problem**: Initially only had config in Init state:
```rust
enum State {
    Init(Request, Config),
    Trying(Request),  // Config lost!
}
```

**Solution**: Add config to ALL state tuples:
```rust
type InitData = Box<(Request, Timeout, Pool, Redirects, Config)>;
type TryingData = Box<(Request, Timeout, Pool, Descriptor, Redirects, Config)>;

// Pass config through transitions:
State::Init((req, timeout, pool, redirects, config)) => {
    State::Trying((req, timeout, pool, descriptor, redirects, config))
}
```

**Rationale**: Features like proxy need config when creating connections in ANY state (Init, Trying, Retrying, etc.).

---

## WebSocket Feature Specification (2026-03-03)

### 8. Request Builder Selection - Use the Right Abstraction Level

**Key Insight**: When building HTTP requests for established connections, use the low-level `SimpleIncomingRequestBuilder` instead of the high-level `ClientRequestBuilder`.

**What Happened**:
- Initial WebSocket specification used `ClientRequestBuilder` for upgrade handshake
- User questioned: “There is a SimpleIncomingRequestBuilder so why go use ClientRequestBuilder?”
- This revealed incorrect abstraction level in the spec

**Problem with ClientRequestBuilder**:
```rust
// WRONG: ClientRequestBuilder does URL parsing, DNS resolution, connection management
let request = ClientRequestBuilder::<SystemDnsResolver>::get(url)?
    .header(SimpleHeader::UPGRADE, “websocket”)
    .build()?;
let simple_request = request.into_simple_incoming_request()?; // Unnecessary conversion
```

**Correct with SimpleIncomingRequestBuilder**:
```rust
// RIGHT: Direct HTTP request building for established connection
let request = SimpleIncomingRequestBuilder::get(uri.path_and_query())
    .header(SimpleHeader::HOST, uri.host_with_port())
    .header(SimpleHeader::UPGRADE, “websocket”)
    .build()?;
// No conversion needed, ready for Http11::request(&request).http_render()
```

**Key Differences**:

| Aspect | ClientRequestBuilder | SimpleIncomingRequestBuilder |
|--------|---------------------|------------------------------|
| **Purpose** | High-level client requests | Low-level HTTP message building |
| **Connection** | Creates new connections | Works with existing connection |
| **DNS Resolution** | Handles DNS lookup | No DNS involvement |
| **URL Handling** | Parses full URLs | Takes path/query directly |
| **Output** | `Request` (needs conversion) | `SimpleIncomingRequest` (direct) |
| **Use Case** | New HTTP requests | Protocol upgrades, tunneling |

**When to Use Each**:

**Use ClientRequestBuilder**:
- Making new HTTP client requests (GET, POST, etc.)
- Need automatic connection management
- Need DNS resolution
- Building complete request from URL

**Use SimpleIncomingRequestBuilder**:
- WebSocket upgrade handshake (connection already established)
- HTTP CONNECT proxy tunneling (building CONNECT request)
- Protocol switching scenarios
- Direct HTTP message construction
- Server-side request building

**Additional Details**:
- `SimpleIncomingRequestBuilder` requires explicit `HOST` header for HTTP/1.1
- Direct integration with `Http11::request().http_render()` (no conversion)
- Part of `simple_http/impls.rs` core types
- Follows the principle: use lowest abstraction level that works

**Pattern for WebSocket Handshake**:
```rust
// 1. Connection already established
let mut conn = HttpClientConnection::connect(&uri, &resolver, timeout)?;

// 2. Build upgrade request at low level
let request = SimpleIncomingRequestBuilder::get(uri.path_and_query())
    .header(SimpleHeader::HOST, uri.host_with_port())
    .header(SimpleHeader::UPGRADE, “websocket”)
    .header(SimpleHeader::CONNECTION, “Upgrade”)
    .header(SimpleHeader::SEC_WEBSOCKET_KEY, &key)
    .header(SimpleHeader::SEC_WEBSOCKET_VERSION, “13”)
    .build()?;

// 3. Render using RenderHttp
let request_bytes = Http11::request(&request).http_render()?;

// 4. Write to existing connection
for chunk in request_bytes {
    conn.write_all(&chunk?)?;
}
```

**Lesson**: Always choose the request builder that matches your abstraction level:
- Already have a connection? Use `SimpleIncomingRequestBuilder`
- Need to create a connection? Use `ClientRequestBuilder`
- Building responses? Use `SimpleIncomingResponse`

This applies to other protocol upgrade scenarios (HTTP/2 upgrade, tunneling, etc.).

---

*2026-02-28: Added a reminder from .agents/skills/rust-clean-code/implementation/skill.md—SYNC ONLY: Do not use async/await or tokio in client or test code for this spec. All patterns must follow synchronous design and Rust Clean Code rules only.*

*2026-02-28: “sync” is not a Cargo feature for this project. The codebase is synchronous by design and standards; never attempt to toggle or test with a sync feature. All code, tests, and runners assume sync-by-default.*

*2026-02-28: The correct Cargo feature to pass (where needed) is “std” for standard library support, not “sync.”*

*2026-03-03: Added comprehensive HTTP proxy support learnings covering multiple connection paths, architecture patterns, TLS upgrade reuse, environment variables, and state machine configuration.*

*2026-03-03: Added WebSocket request builder selection learning covering the distinction between ClientRequestBuilder (high-level, connection management) and SimpleIncomingRequestBuilder (low-level, message building).*
