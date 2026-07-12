# 04 — HTTP via DynNetClient + PreparedRequestBuilder

**Date:** 2026-07-10
**Updated:** 2026-07-12 (F51 alignment — Feature 01)
**Status:** Resolved

## Decision

All Docker Engine HTTP API calls use `foundation_netio::DynNetClient` and
`PreparedRequestBuilder` as the transport — the F51 cross-platform surface that
works on native (TCP/TLS/Unix sockets) and wasm32 (browser `fetch` / CF Workers).

This replaces the deprecated `SimpleHttpClient<R>` + `ClientRequestBuilder<R>` pattern.

## Architecture

```
DockerClient
  ├── http: DynNetClient               // Arc<dyn NetClient> — HTTP + WebSocket
  ├── socket_path: Option<PathBuf>     // for Unix socket transport
  ├── base_url: String                  // "http://localhost" or "unix://" scheme
  ├── api_version: ClientVersion        // e.g. 1.53
  └── auth: Option<DockerCredentials>   // registry auth header
```

## How DynNetClient works

`DynNetClient` (`Arc<dyn NetClient>`) is the platform-agnostic client handle:
- Built via `HttpClientBuilder::new().build()`
- Implements `HttpClient` (request-response) + `WebSocketConnector` (persistent connections)
- No resolver generic — `DynNetClient` is a concrete type, not `DynNetClient<R>`
- No DNS, no pool exposure — those are internal to the client
- Works on native (TCP/TLS) and wasm32 (browser `fetch`)
- `PreparedRequestBuilder` produces `PreparedRequest` values consumed by `HttpClient::open_exchange()`
- Returns `HttpExchangeClientTask` — a `TaskIterator` that yields `HttpExchange` variants

## The HTTP exchange pattern

Feature 01 establishes three layers for processing HTTP exchanges:

### 1. `split_exchange()` — split, don't send

Returns `(head, body, task)` — the caller sends the continuation to valtron:

```rust
let (head, body, task) = body_reader::split_exchange(&client, builder);
valtron::send(task.map_ready(|_| ()))?;

// Drain head — one (status, headers) then done
let (status, headers) = match head.next() { ... };

// Stream body chunks
while let Some(Stream::Next(Ok(chunk))) = body.next() {
    handle(chunk);
}
```

### 2. `send_and_split()` — split + send

Convenience: calls `split_exchange()` + `valtron::send()`, returns `(head, body)`.

### 3. `collect_exchange()` — non-streaming collect

Splits, sends, drains head, collects body chunks into `BytesMut`, returns
`SimpleResponse<SendSafeBody>`.

### Generated code

The code generator (`foundation_openapi`) produces functions that accept
`DynNetClient` (no `R` generic) and `&mut PreparedRequestBuilder` in closures:

```rust
pub fn create_container_request<F>(
    client: DynNetClient,
    args: &CreateContainerArgs,
    builder_mod: Option<F>,
) -> Result<
    impl TaskIterator<Ready = Result<ApiResponse<ContainerCreateResponse>, ApiError>, ...> + Send,
    ApiError,
>
where
    F: FnOnce(&mut PreparedRequestBuilder),
{
    let mut builder = PreparedRequestBuilder::post(&endpoint_url)?
        .header("Content-Type", "application/json")
        .query("name", args.name.as_deref());

    builder = builder.body_json(&args.body)
        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;

    if let Some(f) = builder_mod {
        f(&mut builder);
    }

    // Non-streaming: collect_exchange handles split → send → collect
    let result = body_reader::collect_exchange(&client, builder);
    // ... wrap in TaskIterator ...
}
```

### Docker-specific: streaming endpoints

For streaming endpoints (logs, events, stats), use `split_exchange()`:

```rust
pub fn container_logs_request<F>(
    client: DynNetClient,
    args: &ContainerLogsArgs,
    builder_mod: Option<F>,
) -> (
    CollectorStreamIterator<...>,  // head
    CollectorStreamIterator<...>,  // body
    Box<dyn TaskIterator<...> + Send>,  // task — caller sends to valtron
)
where
    F: FnOnce(&mut PreparedRequestBuilder),
{
    let mut builder = PreparedRequestBuilder::get(&endpoint_url)?
        .query("stdout", Some(&args.stdout.to_string()))
        .query("stderr", Some(&args.stderr.to_string()))
        .query("follow", Some(&args.follow.to_string()));

    if let Some(f) = builder_mod {
        f(&mut builder);
    }

    body_reader::split_exchange(&client, builder)
}
```

### Unix socket support

Docker's primary local transport is the Unix socket (`/var/run/docker.sock`).
`DynNetClient` supports Unix socket connections via the `HttpClientBuilder`.
This is covered in **[07 — Unix socket transport](07-unix-socket-transport.md)**.

### Error handling

Docker API errors follow a standard format:
```json
{"message": "No such container: abc123"}
```

We define:
```rust
#[derive(Debug, Display)]
pub enum DockerApiError {
    #[display("Docker API error ({status}): {message}")]
    Response { status: u16, message: String },
    #[display("HTTP transport error: {_0}")]
    Transport(String),
    #[display("JSON parse error: {_0}")]
    JsonParse(String),
    #[display("Docker not available: {_0}")]
    Unavailable(String),
}
```

### Registry authentication

For pull/push/build operations that need registry auth, set the `X-Registry-Auth`
header via the `builder_mod` closure:

```rust
fn auth_mod(credentials: &DockerCredentials) -> impl FnOnce(&mut PreparedRequestBuilder) + '_ {
    let json = serde_json::to_string(credentials).unwrap();
    let encoded = base64::prelude::BASE64_STANDARD.encode(&json);
    move |b: &mut PreparedRequestBuilder| {
        b.header(SimpleHeader::custom("X-Registry-Auth"), encoded);
    }
}
```

### API version negotiation

Docker supports version negotiation via `GET /version`. We implement:

```rust
impl DockerClient {
    pub fn negotiate_version(&mut self) -> Result<(), DockerApiError> {
        // GET /version → parse ServerVersion → extract ApiVersion field
        // Compare with our supported version (1.53)
        // Use min(client_version, server_version)
    }
}
```

Most users will use `DockerClient::connect_with_defaults()` which auto-negotiates.

## Related

- **[07 — Unix socket transport](07-unix-socket-transport.md)** — Transport layer (→ Feature 02)
- **[08 — Streaming responses](08-streaming-responses.md)** — Streaming endpoint handling (→ Feature 04)
- **[Feature 01 — DynNetClient + PreparedRequestBuilder alignment](../features/01-dynnetclient-codegen-alignment/feature.md)** — Full design
