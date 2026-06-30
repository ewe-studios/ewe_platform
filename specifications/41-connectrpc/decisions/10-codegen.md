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
pub trait GreetService: Send + Sync + 'static {
    /// Unary RPC: Greet
    fn greet(
        &self,
        ctx: &connectrpc::RequestContext,
        request: connectrpc::Request<GreetRequest>,
    ) -> Result<connectrpc::Response<GreetResponse>, connectrpc::ConnectError>;

    /// Client streaming RPC: GreetGroup (pull requests; headers via `requests.headers()`)
    fn greet_group(
        &self,
        ctx: &connectrpc::RequestContext,
        requests: connectrpc::MessageSource<GreetRequest>,
    ) -> Result<connectrpc::Response<GreetGroupResponse>, connectrpc::ConnectError>;

    /// Server streaming RPC: GreetIndividuals (push responses into the sink)
    fn greet_individuals(
        &self,
        ctx: &connectrpc::RequestContext,
        request: connectrpc::Request<GreetRequest>,
        responses: connectrpc::MessageSink<GreetResponse>,
    ) -> Result<(), connectrpc::ConnectError>;

    /// Bidirectional streaming RPC: Converse (interleave receive/send freely)
    fn converse(
        &self,
        ctx: &connectrpc::RequestContext,
        requests: connectrpc::MessageSource<ConverseRequest>,
        responses: connectrpc::MessageSink<ConverseResponse>,
    ) -> Result<(), connectrpc::ConnectError>;
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

    // Client streaming: GreetGroup
    {
        let svc = service.clone();
        router.client_stream(
            procedure::GREET_GROUP,
            connectrpc::client_stream_handler_fn(move |ctx, headers, reqs| svc.greet_group(ctx, headers, reqs)),
            connectrpc::HandlerOptions::new(),
        );
    }

    // Server streaming: GreetIndividuals
    {
        let svc = service.clone();
        router.server_stream(
            procedure::GREET_INDIVIDUALS,
            connectrpc::server_stream_handler_fn(move |ctx, req| svc.greet_individuals(ctx, req)),
            connectrpc::HandlerOptions::new(),
        );
    }

    // Bidi streaming: Converse
    {
        let svc = service.clone();
        router.bidi_stream(
            procedure::CONVERSE,
            connectrpc::bidi_stream_handler_fn(move |ctx, headers, reqs| svc.converse(ctx, headers, reqs)),
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

    /// Unary: Greet
    pub fn greet(
        &self,
        ctx: &mut connectrpc::RequestContext,
        request: connectrpc::Request<GreetRequest>,
    ) -> Result<connectrpc::Response<GreetResponse>, connectrpc::ConnectError> {
        self.greet.call_unary(ctx, request)
    }

    /// Client streaming: GreetGroup
    pub fn greet_group(
        &self,
        ctx: &mut connectrpc::RequestContext,
    ) -> connectrpc::ClientStream<GreetRequest, GreetGroupResponse> {
        self.greet_group.call_client_stream(ctx)
    }

    /// Server streaming: GreetIndividuals
    pub fn greet_individuals(
        &self,
        ctx: &mut connectrpc::RequestContext,
        request: connectrpc::Request<GreetRequest>,
    ) -> Result<connectrpc::ServerStream<GreetResponse>, connectrpc::ConnectError> {
        self.greet_individuals.call_server_stream(ctx, request)
    }

    /// Bidirectional streaming: Converse
    pub fn converse(
        &self,
        ctx: &mut connectrpc::RequestContext,
    ) -> connectrpc::BidiStream<ConverseRequest, ConverseResponse> {
        self.converse.call_bidi_stream(ctx)
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
- **Handler shape (H1 / H2 / H3):** generated server-streaming and bidi traits use
  Decision 11's push model (`MessageSink<Res>` and/or `MessageSource<Req>`), not returned
  iterators; client-streaming headers come from `MessageSource::headers()`, not a separate
  parameter.

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
- **View handlers (supported):** the byte-seam revision (Decision 11) removed the `dyn Any`
  boundary that previously blocked this (RS2), so the typed facade can decode a borrowed
  `MessageView<'a>` in place. Generate optional zero-copy "view" handler variants for
  read-heavy services (the owned-message variant remains the default).

## Open Questions

1. **buffa-codegen integration**: Should `foundation_connectrpc_codegen` invoke `buffa-codegen` internally (unified output), or require users to run both codegen steps? Unified is more ergonomic; separate is more flexible.
2. **Service trait object safety**: The generated service trait is not object-safe (methods have generic-like streaming types). This is fine for static dispatch but means you can't do `Box<dyn GreetService>`. Is this acceptable?
3. **Default implementations**: Should the generated service trait provide default `unimplemented` implementations for all methods (like tonic), so users can implement incrementally? connect-go requires all methods.
4. **View handlers**: buffa supports `MessageView<'a>` for zero-copy deserialization. Should we generate "view" variants of handlers that receive borrowed views instead of owned messages? This could be a significant performance advantage for read-heavy services.
