# Decision 10: Code Generation

**TODO**: I would also like to add the capability to generate a service and client definition from a trait with the only requirement that the inputs structs and outputs must all support ToArrow/FromArrow and/or serde or buffer serializaiton for json since protobuf requires the protobuf definition, this flexibility allows users decide if they want a protobuf or code driven definition, our macro does not care, it generates the needed code as necessary. This is really nice because i can have a crate that just defines my service traits and generate the actual implementation modules for them in another module if we want. I wonder if we can even support importing other service trait definitions and generating for them in a new crate - if not directly - maybe by adding a declaration on a empty struct or module (as I am not sure rust allows us define a macro without it being on a struct, module or function) but lets explore.

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

### Approach: one unified generator (no separate codegen crate)

> **Decided (see "Codegen scope" below).** Generation logic lives in **`foundation_macros`**
> (the repo's central home for all proc/derive/codegen logic — see the macros-location rule);
> a **binary in `foundation_netio`** exposes it as the CLI / protoc plugin. There is **no**
> separate `connectrpc-codegen` / `connectrpc_build` crate, and the generator emits
> **everything** (message types + service traits + clients) in **one pass** — no second manual
> codegen step. The two integration modes below are surfaces over that single generator.

#### Mode 1: build.rs (Recommended)

**Yes add**

```rust
// build.rs — calls the foundation_macros generator via the foundation_netio build helper API
fn main() {
    foundation_netio::connectrpc_build::Config::new()
        .files(&["proto/greet.proto"])
        .includes(&["proto/"])
        .compile()
        .unwrap();
}
```

Produces message types **and** ConnectRPC service stubs in one output — a single `build.rs`
call, no separate buffa step.

#### Mode 2: protoc plugin

**Yes add**


```sh
protoc --connect-ewe_out=. --plugin=protoc-gen-connect-ewe service.proto
```

The `protoc-gen-connect-ewe` **binary lives in `foundation_netio`** and drives the same
`foundation_macros` generator; it emits message types + stubs together.

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
    /// Unary — ctx BY VALUE (`Ctx` is owned + cheap-`Clone`, Arc-backed internals,
    /// Decision 04 §Ctx); errors are `ConnectResult` (Decision 03).
    async fn greet(&self, ctx: Ctx, request: Request<GreetRequest>)
        -> ConnectResult<Response<GreetResponse>>;

    /// Client streaming — consume an async Stream of requests, return one response.
    async fn greet_group(&self, ctx: Ctx, requests: impl Stream<Item = ConnectResult<GreetRequest>>)
        -> ConnectResult<Response<GreetGroupResponse>>;

    /// Server streaming — return an async Stream of responses.
    async fn greet_individuals(&self, ctx: Ctx, request: Request<GreetRequest>)
        -> ConnectResult<impl Stream<Item = ConnectResult<GreetResponse>>>;

    /// Bidi — async Stream in, async Stream out.
    async fn converse(&self, ctx: Ctx, requests: impl Stream<Item = ConnectResult<ConverseRequest>>)
        -> ConnectResult<impl Stream<Item = ConnectResult<ConverseResponse>>>;
}

// ============================================================
// Service registration
// ============================================================

/// Procedure path for each RPC method — leading slash included (R1): the full path
/// standard clients send, matching the router's key format (Decision 08).
pub mod procedure {
    pub const GREET: &str = "/connectrpc.greet.v1.GreetService/Greet";
    pub const GREET_GROUP: &str = "/connectrpc.greet.v1.GreetService/GreetGroup";
    pub const GREET_INDIVIDUALS: &str = "/connectrpc.greet.v1.GreetService/GreetIndividuals";
    pub const CONVERSE: &str = "/connectrpc.greet.v1.GreetService/Converse";
}

/// Register a GreetService implementation with a ConnectRPC router.
pub fn register_greet_service<S: GreetService>(
    router: &mut connectrpc::Router,
    service: std::sync::Arc<S>,
) {
    // Closures clone the Arc'd service into `async move` so the returned futures/streams
    // are `'static` (S2; `ctx` is already by-value). The generated fn also builds each
    // procedure's typed codec table (`ProcedureCodecs`, Decision 02).

    // Unary: Greet (idempotent → GET eligible)
    {
        let svc = service.clone();
        router.unary(
            procedure::GREET,
            connectrpc::ProcedureCodecs::<GreetRequest, GreetResponse>::defaults(),
            move |ctx, req| { let svc = svc.clone(); async move { svc.greet(ctx, req).await } },
            connectrpc::HandlerOptions::new()
                .with_idempotency(connectrpc::IdempotencyLevel::NoSideEffects),
        );
    }

    // Client streaming: GreetGroup — 2 args: ctx + the request Stream (headers ride on
    // the Stream's context, not a param).
    {
        let svc = service.clone();
        router.client_stream(
            procedure::GREET_GROUP,
            connectrpc::ProcedureCodecs::<GreetRequest, GreetGroupResponse>::defaults(),
            move |ctx, reqs| { let svc = svc.clone(); async move { svc.greet_group(ctx, reqs).await } },
            connectrpc::HandlerOptions::new(),
        );
    }

    // Server streaming: GreetIndividuals — returns an async Stream of responses.
    {
        let svc = service.clone();
        router.server_stream(
            procedure::GREET_INDIVIDUALS,
            connectrpc::ProcedureCodecs::<GreetRequest, GreetResponse>::defaults(),
            move |ctx, req| { let svc = svc.clone(); async move { svc.greet_individuals(ctx, req).await } },
            connectrpc::HandlerOptions::new(),
        );
    }

    // Bidi streaming: Converse — async Stream in, async Stream out.
    {
        let svc = service.clone();
        router.bidi_stream(
            procedure::CONVERSE,
            connectrpc::ProcedureCodecs::<ConverseRequest, ConverseResponse>::defaults(),
            move |ctx, reqs| { let svc = svc.clone(); async move { svc.converse(ctx, reqs).await } },
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
    ) -> connectrpc::ConnectResult<Self> {
        // Constants carry the leading slash (R1), so paths concatenate without a separator.
        // Each Client::new also receives the procedure's ProcedureCodecs table (Decision 02).
        Ok(Self {
            greet: connectrpc::Client::new(
                transport.clone(),
                &format!("{}{}", base_url, procedure::GREET),
                connectrpc::ProcedureCodecs::defaults(),
                options.clone().with_idempotency(connectrpc::IdempotencyLevel::NoSideEffects),
            )?,
            greet_group: connectrpc::Client::new(
                transport.clone(),
                &format!("{}{}", base_url, procedure::GREET_GROUP),
                connectrpc::ProcedureCodecs::defaults(),
                options.clone(),
            )?,
            greet_individuals: connectrpc::Client::new(
                transport.clone(),
                &format!("{}{}", base_url, procedure::GREET_INDIVIDUALS),
                connectrpc::ProcedureCodecs::defaults(),
                options.clone(),
            )?,
            converse: connectrpc::Client::new(
                transport.clone(),
                &format!("{}{}", base_url, procedure::CONVERSE),
                connectrpc::ProcedureCodecs::defaults(),
                options,
            )?,
        })
    }

    // Client methods are async, mirroring the server shapes (Decision 07's client types are
    // realized over the streaming `Transport`; no valtron types leak).

    /// Unary
    pub async fn greet(&self, ctx: Ctx, request: Request<GreetRequest>)
        -> ConnectResult<Response<GreetResponse>> { self.greet.unary(ctx, request).await }

    /// Client streaming — send an async Stream of requests, await one response.
    pub async fn greet_group(&self, ctx: Ctx, reqs: impl Stream<Item = GreetRequest>)
        -> ConnectResult<Response<GreetGroupResponse>> { self.greet_group.client_stream(ctx, reqs).await }

    /// Server streaming — await a response Stream.
    pub async fn greet_individuals(&self, ctx: Ctx, request: Request<GreetRequest>)
        -> ConnectResult<impl Stream<Item = ConnectResult<GreetResponse>>> {
        self.greet_individuals.server_stream(ctx, request).await
    }

    /// Bidi — async Stream in, async Stream out.
    pub async fn converse(&self, ctx: Ctx, reqs: impl Stream<Item = ConverseRequest>)
        -> ConnectResult<impl Stream<Item = ConnectResult<ConverseResponse>>> {
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

# Codegen is NOT a separate crate. Generation logic lives in foundation_macros;
# the CLI / protoc-plugin binary lives in foundation_netio (see "Codegen scope" below).
backends/foundation_macros/
├── src/
│   └── connectrpc/                      # unified generator: messages + service traits + clients

backends/foundation_netio/
├── src/
│   └── connectrpc_build.rs              # build.rs helper API (Mode 1)
└── src/bin/
    └── protoc-gen-connect-ewe.rs        # protoc plugin binary (Mode 2)
```

### Naming: `protoc-gen-connect-ewe`

The protoc plugin binary (in `foundation_netio`) is named `protoc-gen-connect-ewe` to distinguish from the upstream `protoc-gen-connect-go` and third-party `protoc-gen-connect-rust`. This makes it clear it generates code for the ewe platform's ConnectRPC implementation.

### Method Name Conversion

Proto method names are PascalCase. Rust convention is snake_case:
- `Greet` → `greet`
- `GreetGroup` → `greet_group`
- `GetUser` → `get_user`

Use the `heck` crate for conversion (already in buffa's dependencies).

## Consequences

- **No separate codegen crate**: generation logic is in `foundation_macros`; the CLI / protoc-plugin binary is in `foundation_netio`. One generator emits message types **and** service/client stubs in a single pass (supersedes the `foundation_connectrpc_codegen`/`_build` sketch).
- Generated service traits use foundation_connectrpc's handler types
- Generated clients are typed wrappers around `connectrpc::Client`
- Procedure paths follow protobuf convention: `package.Service/Method`
- Idempotency level from proto `option idempotency_level` flows through to handler options
- The unified generator **learns from** buffa-codegen but emits message types itself (one pass); it does not depend on a separate buffa-codegen invocation

## Decided Details

- **R1 — leading slash:** generated procedure constants include the leading slash
  (`/package.Service/Method`).
- **R2 — Unimplemented handler:** generate `Unimplemented<Service>Handler` returning
  `unimplemented` for every method.
- **R3 — service-name constant:** generate a fully-qualified `<Service>Name` constant.
- **R4 — client trait:** generate a `<Service>Client` trait alongside the struct (for
  mocking/testing).
- **R5 — `WithSchema`:** propagate the method descriptor/schema to generated handler and
  client constructors.
- **R19 — `ClientOptions: Clone`:** provide an explicit `Clone` impl (it holds
  `Vec<Arc<dyn Interceptor>>`), since generated constructors clone it. *(Relabeled from
  R14 — that label belongs to Decision 09's Cedar authorization; R-labels are global.)*
- **Custom-codec entry points:** codegen also emits
  `register_<service>_with_codec::<S, C>(router, service, codec)` and
  `<Service>Client::new_with_codec::<C>(…)`, expanding the bound `C: CodecFor<M>` over
  **every** request/response type in the service — the registration-time installation point
  the Decision 02 `ProcedureCodecs` table requires. Options (`with_codec(name)`) only
  *select* among installed entries; they never install. The macro hides the bound expansion,
  so the user supplies one codec value.
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
- **Default-body hidden types (codegen template constraint):** a defaulted stream-returning
  method (`-> ConnectResult<impl Stream<…>>`) cannot have a body of bare `unimplemented!()`
  or `Err(…)` — RPITIT needs a **nameable hidden type** inferred from the body, and a
  diverging/`Err`-only body provides none. The generator therefore emits a typed error
  return that pins one without boxing, e.g.
  `let r: ConnectResult<futures::stream::Empty<ConnectResult<Res>>> =
  Err(ConnectError::unimplemented(procedure::…).into()); r` (hidden type =
  `stream::Empty<_>`). Unary defaults need no such pinning (`Response<Res>` is concrete).
- **`'static` returned streams (S2):** a returned `-> impl Stream + Send` from an
  `async fn(&self, ctx: &Ctx, …)` would capture `&self`/`&ctx` and be **non-`'static`**, so
  `from_stream` (which needs `Send + 'static`) couldn't drive it. Resolved by the **by-value
  `Ctx`** (Decision 04 §Ctx — owned + cheap-`Clone` with Arc-backed internals, passed owned
  in **all four kinds**, uniformly): the handler produces an **owning** stream (captures `Arc<Self>`/`Ctx` clones in
  an `async move`), and codegen emits the RPITIT return as `+ Send + 'static` with
  `use<>`-style capture control.

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
- **Zero-copy variants (supported; single-codec by construction — fresh-review A5):**
  owned-decode is the default. For read-heavy services, generate a zero-copy variant whose
  request type is `buffa::OwnedView<…>` (proto; `'static + Send + Sync`, survives `.await`)
  or an Arc-backed `RecordBatch` (Arrow). Because those types satisfy no `CodecFor` family
  bound, the variant has **no generic `ProcedureCodecs` table**: codegen fixes one concrete
  codec and dispatches its inherent view method statically; other content-types → 415
  (documented non-conformant extension endpoint, like `only(...)`). A naked borrowed
  `MessageView<'a>` is **not** generated (can't cross `.await`).
