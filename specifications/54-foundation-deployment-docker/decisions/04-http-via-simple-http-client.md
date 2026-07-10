# 04 — HTTP via SimpleHttpClient

**Date:** 2026-07-10
**Status:** Resolved

## Decision

All Docker Engine HTTP API calls use `foundation_netio::SimpleHttpClient<R>` as the transport,
matching the pattern used by `foundation_deployment_cloudflare` and the auto-generated Stripe/API
providers.

## Architecture

```
DockerClient
  ├── http_client: SimpleHttpClient<R>
  ├── socket_path: Option<PathBuf>    // for Unix socket transport
  ├── base_url: String                 // "http://localhost" or "unix://" scheme
  ├── api_version: ClientVersion       // e.g. 1.53
  └── auth: Option<DockerCredentials>  // registry auth header
```

### How SimpleHttpClient works

`SimpleHttpClient<R>` is a generic HTTP client built on our own HTTP stack:
- Generic over `R: DnsResolver` (defaults to `SystemDnsResolver`)
- Connection pooling via `HttpConnectionPool<R>`
- TLS via `SSLConnector`
- Configurable timeouts (connect, read, write)
- Redirect handling
- Middleware chain support
- Builder pattern: `.get(url).header(...).body(...).build_send_request()`

Returns a `TaskIterator` — no `async fn`, no `Future`, no tokio.

### Docker-specific request building

Docker's API uses JSON request/response bodies with some special cases:

1. **JSON body endpoints** (most CRUD operations):
   ```rust
   // POST /containers/create
   fn create_container(
       client: &SimpleHttpClient<R>,
       options: Option<&CreateContainerOptions>,
       config: ContainerCreateBody,
   ) -> Result<impl TaskIterator<Ready = Result<ContainerCreateResponse, DockerApiError>, ...>, DockerApiError> {
       let url = format!("{}{}/containers/create?{}", base_url, api_version, query_string(options));
       client.post(&url)
           .header("Content-Type", "application/json")
           .json_body(&config)
           .build_send_request()?
           .map_ready(|intro| match intro {
               RequestIntro::Success { stream, .. } => parse_json_body::<ContainerCreateResponse>(stream),
               RequestIntro::Failed(e) => Err(DockerApiError::RequestFailed(e)),
           })
   }
   ```

2. **Streaming endpoints** (logs, events, stats):
   ```rust
   // GET /containers/{id}/logs?stdout=1&stderr=1&follow=1
   // Returns a stream of LogOutput via SharedByteBufferStream
   ```

3. **Tarball endpoints** (export, download/archive):
   ```rust
   // GET /containers/{id}/archive?path=/app
   // Returns raw byte stream (tar archive)
   ```

4. **Image build** (POST /build):
   ```rust
   // multipart/form-data with Dockerfile tarball context
   ```

### Unix socket support

Docker's primary local transport is the Unix socket (`/var/run/docker.sock`).
`SimpleHttpClient` needs to support Unix socket connections. This is covered in
**[07 — Unix socket transport](07-unix-socket-transport.md)**.

### Error handling

Docker API errors follow a standard format:
```json
{"message": "No such container: abc123"}
```

We define:
```rust
/// Error from the Docker Engine API.
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

For pull/push/build operations that need registry auth, we set the `X-Registry-Auth` header:
```rust
fn x_registry_auth_header(credentials: &DockerCredentials) -> Result<String, DockerApiError> {
    let json = serde_json::to_string(credentials)?;
    Ok(base64::Engine::encode(&base64::prelude::BASE64_STANDARD, json))
}
```

### API version negotiation

Docker supports version negotiation via `GET /version`. We implement:
```rust
impl DockerClient {
    pub fn negotiate_version(&mut self) -> Result<(), DockerApiError> {
        // GET /version -> parse ServerVersion -> extract ApiVersion field
        // Compare with our supported version (1.53)
        // Use min(client_version, server_version)
    }
}
```

Most users will use `DockerClient::connect_with_defaults()` which auto-negotiates.
