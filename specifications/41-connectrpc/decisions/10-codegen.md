# Decision 10: Code Generation

## Context

connect-go generates service stubs as part of a protoc plugin (`protoc-gen-connect-go`). For each service in a `.proto` file, it generates:
- A handler constructor (`NewGreetServiceHandler`) that returns `(string, http.Handler)` — the path and the handler
- A client constructor (`NewGreetServiceClient`) that returns a typed client
- An interface for the service implementation

The generated code depends on the `connectrpc.com/connect` runtime library.

Our platform uses buffa for protobuf, which has its own code generation:
- `buffa-codegen` — code generation library (descriptors → Rust source)
- `buffa-build` — build.rs integration
- `protoc-gen-buffa` — protoc plugin

We need to generate ConnectRPC service stubs that:
1. Define a service trait with one method per RPC
2. Generate handler registration (Router integration)
3. Generate typed clients
4. Work with buffa-generated message types

## Decision

### Approach: Companion Code Generator

Create `connectrpc-codegen` as a companion to buffa's codegen. Two integration modes:

#### Mode 1: build.rs (Recommended)

```rust
// build.rs
fn main() {
    connectrpc_build::Config::new()
        .files(&["proto/greet.proto"])
        .includes(&["proto/"])
        .compile()
        .unwrap();
}
```

This invokes buffa-codegen internally (generating message types) and then generates ConnectRPC service stubs in the same output. Single `build.rs` call produces everything.

#### Mode 2: protoc plugin

```sh
protoc --buffa_out=. --connect-rust_out=. --plugin=protoc-gen-connect-rust service.proto
```

Separate plugin that generates only service stubs, referencing buffa-generated message types via `extern_path` mappings.

### Generated Output Structure

For a proto service:
```protobuf
package connectrpc.greet.v1;

service GreetService {
    rpc Greet(GreetRequest) returns (GreetResponse) {
        option idempotency_level = NO_SIDE_EFFECTS;
    }
    rpc GreetGroup(stream GreetRequest) returns (GreetGroupResponse) {}
    rpc GreetIndividuals(GreetRequest) returns (stream GreetResponse) {}
    rpc Converse(stream ConverseRequest) returns (stream ConverseResponse) {}
}
```

Generate:

```rust
// ============================================================
// Service trait
// ============================================================

/// Server-side implementation trait for GreetService.
/// Methods are **async fns / async Streams** (Decision 04); the seam/queues are internal
/// (Decision 11). `Req`/`Res` are owned-decoded by default; a buffa `OwnedView<…>` (proto)
/// or `RecordBatch` (Arrow) variant can be generated for the zero-copy path.
// Native async-fn-in-traits / RPITIT (static dispatch). Codegen emits the desugared
// `-> impl Future + Send` / `-> impl Stream + Send` form to pin the pool's Send bound.
pub trait GreetService: Send + Sync + 'static {
    /// Unary
    async fn greet(&self, ctx: &Ctx, request: Request<GreetRequest>)
        -> Result<Response<GreetResponse>, ConnectError>;

    /// Client streaming — consume an async Stream of requests, return one response.
    async fn greet_group(&self, ctx: &Ctx, requests: impl Stream<Item = Result<GreetRequest, ConnectError>>)
        -> Result<Response<GreetGroupResponse>, ConnectError>;

    /// Server streaming — return an async Stream of responses.
    async fn greet_individuals(&self, ctx: &Ctx, request: Request<GreetRequest>)
        -> Result<impl Stream<Item = Result<GreetResponse, ConnectError>>, ConnectError>;

    /// Bidi — async Stream in, async Stream out.
    async fn converse(&self, ctx: &Ctx, requests: impl Stream<Item = Result<ConverseRequest, ConnectError>>)
        -> Result<impl Stream<Item = Result<ConverseResponse, ConnectError>>, ConnectError>;
}

// ============================================================
// Service registration
// ============================================================

/// Procedure path for each RPC method.
pub mod procedure {
    pub const GREET: &str = "connectrpc.greet.v1.GreetService/Greet";
    pub const GREET_GROUP: &str = "connectrpc.greet.v1.GreetService/GreetGroup";
    pub const GREET_INDIVIDUALS: &str = "connectrpc.greet.v1.GreetService/GreetIndividuals";
    pub const CONVERSE: &str = "connectrpc.greet.v1.GreetService/Converse";
}

/// Register a GreetService implementation with a ConnectRPC router.
pub fn register_greet_service<S: GreetService>(
    router: &mut connectrpc::Router,
    service: std::sync::Arc<S>,
) {
    // Unary: Greet (idempotent → GET eligible)
    {
        let svc = service.clone();
        router.unary(
            procedure::GREET,
            connectrpc::unary_handler_fn(move |ctx, req| svc.greet(ctx, req)),
            connectrpc::HandlerOptions::new()
                .with_idempotency(connectrpc::IdempotencyLevel::NoSideEffects),
        );
    }

    // Client streaming: GreetGroup — closure forwards to the async method (2 args:
    // ctx + the request Stream; headers ride on the Stream's context, not a param).
    {
        let svc = service.clone();
        router.client_stream(
            procedure::GREET_GROUP,
            move |ctx, reqs| svc.greet_group(ctx, reqs),   // async; framework from_stream-bridges
            connectrpc::HandlerOptions::new(),
        );
    }

    // Server streaming: GreetIndividuals — returns an async Stream of responses.
    {
        let svc = service.clone();
        router.server_stream(
            procedure::GREET_INDIVIDUALS,
            move |ctx, req| svc.greet_individuals(ctx, req),
            connectrpc::HandlerOptions::new(),
        );
    }

    // Bidi streaming: Converse — async Stream in, async Stream out.
    {
        let svc = service.clone();
        router.bidi_stream(
            procedure::CONVERSE,
            move |ctx, reqs| svc.converse(ctx, reqs),
            connectrpc::HandlerOptions::new(),
        );
    }
}

// ============================================================
// Client
// ============================================================

/// Typed client for GreetService.
pub struct GreetServiceClient {
    greet: connectrpc::Client<GreetRequest, GreetResponse>,
    greet_group: connectrpc::Client<GreetRequest, GreetGroupResponse>,
    greet_individuals: connectrpc::Client<GreetRequest, GreetResponse>,
    converse: connectrpc::Client<ConverseRequest, ConverseResponse>,
}

impl GreetServiceClient {
    pub fn new(
        transport: std::sync::Arc<dyn connectrpc::Transport>,
        base_url: &str,
        options: connectrpc::ClientOptions,
    ) -> Result<Self, connectrpc::ConnectError> {
        Ok(Self {
            greet: connectrpc::Client::new(
                transport.clone(),
                &format!("{}/{}", base_url, procedure::GREET),
                options.clone().with_idempotency(connectrpc::IdempotencyLevel::NoSideEffects),
            )?,
            greet_group: connectrpc::Client::new(
                transport.clone(),
                &format!("{}/{}", base_url, procedure::GREET_GROUP),
                options.clone(),
            )?,
            greet_individuals: connectrpc::Client::new(
                transport.clone(),
                &format!("{}/{}", base_url, procedure::GREET_INDIVIDUALS),
                options.clone(),
            )?,
            converse: connectrpc::Client::new(
                transport.clone(),
                &format!("{}/{}", base_url, procedure::CONVERSE),
                options,
            )?,
        })
    }

    // Client methods are async, mirroring the server shapes (Decision 07's client types are
    // realized over the streaming `Transport`; no valtron types leak).

    /// Unary
    pub async fn greet(&self, ctx: &Ctx, request: Request<GreetRequest>)
        -> Result<Response<GreetResponse>, ConnectError> { self.greet.unary(ctx, request).await }

    /// Client streaming — send an async Stream of requests, await one response.
    pub async fn greet_group(&self, ctx: &Ctx, reqs: impl Stream<Item = GreetRequest>)
        -> Result<Response<GreetGroupResponse>, ConnectError> { self.greet_group.client_stream(ctx, reqs).await }

    /// Server streaming — await a response Stream.
    pub async fn greet_individuals(&self, ctx: &Ctx, request: Request<GreetRequest>)
        -> Result<impl Stream<Item = Result<GreetResponse, ConnectError>>, ConnectError> {
        self.greet_individuals.server_stream(ctx, request).await
    }

    /// Bidi — async Stream in, async Stream out.
    pub async fn converse(&self, ctx: &Ctx, reqs: impl Stream<Item = ConverseRequest>)
        -> Result<impl Stream<Item = Result<ConverseResponse, ConnectError>>, ConnectError> {
        self.converse.bidi_stream(ctx, reqs).await
    }
}
```

### Crate Structure

```
backends/foundation_connectrpc/          # Runtime library
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── codec.rs
│   ├── compression.rs
│   ├── envelope.rs
│   ├── error.rs
│   ├── handler.rs
│   ├── interceptor.rs
│   ├── router.rs
│   ├── client.rs
│   ├── protocol/
│   │   ├── mod.rs
│   │   ├── connect.rs
│   │   ├── grpc.rs
│   │   └── grpc_web.rs
│   └── auth/
│       ├── mod.rs
│       ├── middleware.rs
│       └── helpers.rs

backends/foundation_connectrpc_codegen/  # Code generation library
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── codegen.rs                       # Service stub generation
│   └── bin/
│       └── protoc-gen-connect-ewe.rs    # protoc plugin binary

backends/foundation_connectrpc_build/    # build.rs helper
├── Cargo.toml
├── src/
│   └── lib.rs
```

### Naming: `protoc-gen-connect-ewe`

The protoc plugin is named `protoc-gen-connect-ewe` to distinguish from the upstream `protoc-gen-connect-go` and third-party `protoc-gen-connect-rust`. This makes it clear it generates code for the ewe platform's ConnectRPC implementation.

### Method Name Conversion

Proto method names are PascalCase. Rust convention is snake_case:
- `Greet` → `greet`
- `GreetGroup` → `greet_group`
- `GetUser` → `get_user`

Use the `heck` crate for conversion (already in buffa's dependencies).

## Consequences

- Two crates for codegen: library (`foundation_connectrpc_codegen`) and build helper (`foundation_connectrpc_build`)
- Generated service traits use foundation_connectrpc's handler types
- Generated clients are typed wrappers around `connectrpc::Client`
- Procedure paths follow protobuf convention: `package.Service/Method`
- Idempotency level from proto `option idempotency_level` flows through to handler options
- Codegen depends on buffa-codegen for message type generation

## Review-Gap Coverage

- **R1 — leading slash:** generated procedure constants include the leading slash
  (`/package.Service/Method`).
- **R2 — Unimplemented handler:** generate `Unimplemented<Service>Handler` returning
  `unimplemented` for every method.
- **R3 — service-name constant:** generate a fully-qualified `<Service>Name` constant.
- **R4 — client trait:** generate a `<Service>Client` trait alongside the struct (for
  mocking/testing).
- **R5 — `WithSchema`:** propagate the method descriptor/schema to generated handler and
  client constructors.
- **R14 — `ClientOptions: Clone`:** provide an explicit `Clone` impl (it holds
  `Vec<Arc<dyn Interceptor>>`), since generated constructors clone it.
- **Handler shape (H1 / H2 / H3):** generated methods are **async fns / async `Stream`s**
  (Decision 04): unary `async fn(Req) -> Res`; server-stream `async fn(Req) -> impl Stream`;
  client-stream `async fn(impl Stream<Req>) -> Res`; bidi `async fn(impl Stream<Req>) -> impl
  Stream`. The seam/queues are internal (Decision 11); no `MessageSink`/`MessageSource` in
  generated signatures.
- **Async-trait lowering (decided):** generate **native async-fn-in-traits / RPITIT**, with
  the codegen emitting explicit `-> impl Future<…> + Send` / `-> impl Stream<…> + Send`
  returns so the pool's `Send` bound is satisfied. No `#[async_trait]` boxing — the service
  trait is static-dispatch (we don't need `dyn GreetService`; the generated `<Service>Client`
  trait, R4, covers mocking). Fall back to `#[async_trait]` (boxed futures, object-safe) only
  if a `dyn`-dispatch need appears.
- **`'static` returned streams (S2):** a returned `-> impl Stream + Send` from an
  `async fn(&self, ctx: &Ctx, …)` would capture `&self`/`&ctx` and be **non-`'static`**, so
  `from_stream` (which needs `Send + 'static`) couldn't drive it. Therefore the request
  context is passed **owned / `Arc<Ctx>`** (not `&Ctx`) into streaming handlers, the handler
  produces an **owning** stream (captures `Arc<Self>`/`Arc<Ctx>` clones in an `async move`),
  and codegen emits the RPITIT return as `+ Send + 'static` with `use<>`-style capture
  control. Unary is unaffected (its future is awaited and completes in place).

**Codegen scope (decided — include it, fully built and ready):**
- **One unified generator, no split tooling, no separate codegen crate.** We learn from
  buffa-codegen and connect-go's protoc plugin and build our own single generator that
  emits **everything** — message types + service traits + clients — in one pass (no second
  manual codegen step). Placement:
  - **generation logic** lives in **`foundation_macros`** (the repo's central home for all
    proc/derive/codegen logic);
  - **a binary in `foundation_netio`** exposes that capability as the CLI / protoc plugin.

  This supersedes the `foundation_connectrpc_codegen` / `foundation_connectrpc_build`
  crates sketched in the plan — codegen is not its own crate.
- **Default `unimplemented` impls:** the generated service trait provides default methods
  returning `unimplemented`, so a service can be implemented incrementally (tonic-style),
  rather than connect-go's all-methods-required.
- **Object-safety:** the generated service *handler* trait stays statically dispatched
  (not object-safe — acceptable); the separately generated `<Service>Client` trait (R4)
  covers mocking/testing where a trait object is wanted.
- **Zero-copy variants (supported):** owned-decode is the default. For read-heavy services,
  generate a zero-copy variant whose request type is `buffa::OwnedView<…>` (proto;
  `'static + Send + Sync`, survives `.await`) or an Arc-backed `RecordBatch` (Arrow). A naked
  borrowed `MessageView<'a>` is **not** generated (can't cross `.await`).

## Open Questions

**TODO**: Looks outdated

1. **buffa-codegen integration**: Should `foundation_connectrpc_codegen` invoke `buffa-codegen` internally (unified output), or require users to run both codegen steps? Unified is more ergonomic; separate is more flexible.
2. **Service trait object safety**: The generated service trait is not object-safe (methods have generic-like streaming types). This is fine for static dispatch but means you can't do `Box<dyn GreetService>`. Is this acceptable?
   **TODO**: did we not resolve this to support static dispatching ?

3. **Default implementations**: Should the generated service trait provide default `unimplemented` implementations for all methods (like tonic), so users can implement incrementally? connect-go requires all methods.
    **TODO**: interesting question, sure i guess, since they will edit the code after generation.

4. **View handlers**: buffa supports `MessageView<'a>` for zero-copy deserialization. Should we generate "view" variants of handlers that receive borrowed views instead of owned messages? This could be a significant performance advantage for read-heavy services.
   **TODO**: Looks outdated, correct me
