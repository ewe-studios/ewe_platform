# Learnings — Spec 54

## From Spec 53 (docker-container-testbed)

### Bollard is deeply tokio-bound
- `hyper 1.x` requires tokio executor — no way to use bollard without a tokio runtime
- `tokio-util` codecs power all streaming (logs, events, stats)
- `tonic` for BuildKit gRPC is even more tokio-dependent
- The `block_on` workaround from spec 53 works but is fragile — executors don't compose

### Type generation pitfalls
- OpenAPI → code generates types that "work in some ways but need ironing"
- Generated types use `Option<Box<T>>` for nullable fields — unnecessary indirection
- `$ref` chains produce deeply nested wrappers
- `allOf` compositions don't translate to idiomatic Rust
- Manual types guided by the spec (not generated) are cleaner

### Bollard 0.21 API specifics
- Builder pattern everywhere: `CreateContainerOptionsBuilder::default().name("x").build()`
- `ContainerCreateBody` replaces old `Config` struct
- `create_container` takes owned `Option<CreateContainerOptions>` (not by ref)
- Models in `bollard::models`, query params in `bollard::query_parameters`
- `NetworkCreateResponse.id: String` differs from `Network.id: Option<String>`

### Error handling
- `derive_more::From` conflicts when multiple `String` variants exist in an enum
- `derive_more::Error` requires inner types to impl `std::error::Error` (String doesn't)
- `ErrorTrace<DockerError>` provides the `Error` impl; `DockerError` only needs `Display + Debug`
- `?` doesn't auto-convert to `ErrorTrace` — needs explicit `docker_err(...)?` wrapping

## From ballard source study

### Code generation pipeline
- Swagger codegen (Maven/Java) → `bollard-stubs` crate → `bollard::models` + `bollard::query_parameters`
- Tonic/prost → `bollard-buildkit-proto` crate → `bollard::grpc` + `bollard::fsutil` + etc.
- Custom `BollardCodegen.java` templates for idiomatic Rust (not raw swagger output)

### Transport abstraction
- `Transport` enum: Http, Https, Unix, NamedPipe, Ssh, Mock, Custom
- `CustomTransport` trait: `fn request(&self, req: BollardRequest) -> TransportReturnTy`
- `Docker` struct holds `Arc<Transport>` + `RequestModifier` for auth injection

### Response processing
- `process_into_value<T>()` — single JSON response
- `process_into_stream<T>()` — JSON line stream
- `process_into_stream_string()` — multiplexed log stream
- `process_into_body()` — raw byte stream
- `process_upgraded()` — HTTP upgrade (attach, exec, gRPC h2c)
- `process_websocket()` — WebSocket attach

### API version
- Default: 1.53 (Docker Engine v29.4.1)
- Negotiation via `GET /version` — uses `min(client, server)`

## Platform patterns to follow

### foundation_deployment_cloudflare
- Hand-written types in `types.rs` (not generated)
- `CloudflareClient` wraps `SimpleHttpClient` + auth
- Each API operation returns `TaskIterator`
- Error enum with `Display` derive + `ErrorTrace` wrapper

### foundation_deployment (abstractions)
- `Deployable` trait: `deploy()` and `destroy()` return `impl TaskIterator`
- `ProviderClient<S, R>` wraps `StateStore` + `SimpleHttpClient`
- `UpdateTask<T, S>` wraps TaskIterator to persist results
- `Deploying` enum: `Init → Processing → Done → Failed`

### foundation_openapi
- **UnifiedGenerator** produces real code from OpenAPI specs: types from schemas, Args types, `{op}_request()` functions returning `impl TaskIterator`
- Generates `shared/mod.rs` (ApiError/ApiResponse re-exports), per-group `mod.rs` files, provider `mod.rs` with feature guards
- Auto-updates Cargo.toml with feature flags
- Types: proper structs with `Box<>` for recursive types, `#[serde(flatten)] HashMap` fallback for unknown schemas
- Client functions: build URL with path/query params, send via SimpleHttpClient, parse JSON → ApiResponse<T>
- **Limitation**: streaming endpoints get standard JSON handling (needs manual replacement with LogFrameDecoder/JsonLineDecoder)
- `SpecProcessor` parses OpenAPI 3.0 specs
- `NormalizedSpec` has `endpoints`, `types`, `metadata`
- Use generator for initial surface, refine where spec is lossy

### foundation_connectrpc
- `ProtoCodec` for protobuf encoding
- `Client` with `ClientStream`, `ServerStream`, `BidiStream`
- `H1Transport` / `H2Transport` for HTTP transport
- Can implement Unix socket transport for BuildKit gRPC
