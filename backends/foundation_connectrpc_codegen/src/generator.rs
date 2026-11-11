//! Proto-to-Rust code generation for `ConnectRPC` services (Decision 10).
//!
//! WHY: Every `ConnectRPC` service needs a server trait, handler registration, and
//! typed client. Rather than hand-writing these per service, a code generator
//! reads [`FileDescriptorProto`] values (from protobuf file descriptors) and emits
//! compilable Rust source.
//!
//! WHAT: This module provides the single public entry point [`generate_services`],
//! which processes a slice of `FileDescriptorProto` and returns the Rust source
//! code for all services found.
//!
//! For each service it generates:
//! - A `procedure` module with path constants (leading slash per R1).
//! - A fully-qualified `<Service>Name` constant (R3).
//! - An async RPITIT service trait with default unimplemented bodies.
//! - A `register_<service>` registration function.
//! - An `Unimplemented<Service>Handler` struct (R2).
//! - A typed `<Service>Client` struct and trait (R4).
//!
//! HOW: Iterates file descriptors → service descriptors → methods. Method
//! streaming attributes determine the four RPC shapes (unary, `client_stream`,
//! `server_stream`, `bidi_stream`). `PascalCase` method names are lowered to `snake_case`.
//! Idempotency levels are extracted from proto method options (field 34 on
//! `MethodOptions`).

use std::fmt::Write as _;

use prost_types::{
    method_options, FileDescriptorProto, MethodDescriptorProto, MethodOptions,
    ServiceDescriptorProto,
};

/// Message for a failed write to a `String`. A `String` write cannot fail, so
/// this message is only a guard.
const WRITE_ERR: &str = "writing to a String cannot fail";

// ── Public entry point ───────────────────────────────────────────────────

/// Generate `ConnectRPC` service code from proto file descriptors.
/// Returns Rust source code as a `String`.
///
/// Each file descriptor contributes its services. The output is ready to be
/// written to a `.rs` file included via `include!` or a protoc plugin response.
#[must_use]
pub fn generate_services(files: &[FileDescriptorProto]) -> String {
    let mut out = String::with_capacity(4096);
    for file in files {
        let package = file.package.as_deref().unwrap_or("");
        for service in &file.service {
            generate_service(package, service, &mut out);
        }
    }
    out
}

// ── Per-service generation ───────────────────────────────────────────────

fn generate_service(package: &str, service: &ServiceDescriptorProto, out: &mut String) {
    let svc_name = service.name.as_deref().unwrap_or("Unknown");
    let methods: Vec<MethodInfo> = service.method.iter().map(MethodInfo::from).collect();

    // Separator + heading
    out.push_str("// ============================================================\n");
    writeln!(out, "// {svc_name}").expect(WRITE_ERR);
    out.push_str("// ============================================================\n\n");

    // 1. Service name constant (R3)
    generate_name_constant(package, svc_name, out);

    // 2. Procedure path constants (R1)
    generate_procedure_module(package, svc_name, &methods, out);

    // 3. Service trait
    generate_service_trait(svc_name, &methods, out);

    // 4. Registration function
    generate_registration(svc_name, &methods, out);

    // 5. Unimplemented handler (R2)
    generate_unimplemented_handler(svc_name, out);

    // 6. Typed client (R4)
    generate_client(svc_name, &methods, out);
}

// ── Method info helper ───────────────────────────────────────────────────

struct MethodInfo {
    pascal: String,
    snake: String,
    input_type: String,
    output_type: String,
    kind: MethodKind,
    idempotency_expr: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MethodKind {
    Unary,
    ServerStream,
    ClientStream,
    BidiStream,
}

impl MethodInfo {
    fn from(m: &MethodDescriptorProto) -> Self {
        let pascal = m.name.as_deref().unwrap_or("Unknown").to_string();
        let snake = escape_keyword(to_snake_case(&pascal));
        let input_type = extract_type_name(m.input_type.as_deref().unwrap_or("UnknownRequest"));
        let output_type = extract_type_name(m.output_type.as_deref().unwrap_or("UnknownResponse"));
        let client_stream = m.client_streaming.unwrap_or(false);
        let server_stream = m.server_streaming.unwrap_or(false);

        let kind = match (client_stream, server_stream) {
            (false, false) => MethodKind::Unary,
            (false, true) => MethodKind::ServerStream,
            (true, false) => MethodKind::ClientStream,
            (true, true) => MethodKind::BidiStream,
        };

        let idempotency_expr = extract_idempotency(&m.options);

        Self {
            pascal,
            snake,
            input_type,
            output_type,
            kind,
            idempotency_expr,
        }
    }
}

// ── Name helpers ─────────────────────────────────────────────────────────

/// Escape a generated identifier that collides with a Rust keyword by turning
/// it into a raw identifier (`Return` → `r#return`). Raw identifiers are valid
/// everywhere the generator uses method names: `fn` names, struct fields,
/// and call sites (`BuildKit`'s `LLBBridge.Return` is what first hit this).
fn escape_keyword(name: String) -> String {
    // Keywords that can appear from snake_casing a proto RPC name and that
    // Rust permits as raw identifiers.
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn", "else",
        "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in", "let", "loop",
        "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return", "static",
        "struct", "trait", "true", "try", "type", "unsafe", "use", "virtual", "where", "while",
        "yield",
    ];
    if KEYWORDS.contains(&name.as_str()) {
        format!("r#{name}")
    } else {
        name
    }
}

/// Convert `PascalCase` to `snake_case`.
///
/// `GreetGroup` → `greet_group`, `GetURL` → `get_url`, `Greet` → `greet`.
#[must_use]
pub fn to_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                let next = chars.get(i + 1).copied();
                let prev_lower = prev.is_lowercase() || prev.is_ascii_digit();
                let next_lower = next.is_some_and(char::is_lowercase);
                if (prev_lower || (next_lower && i + 1 < len))
                    && result.as_bytes().last() != Some(&b'_')
                {
                    result.push('_');
                }
            }
            result.push(c.to_ascii_lowercase());
        } else if c == '-' {
            result.push('_');
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a proto fully-qualified type name to the short Rust type name.
///
/// `.connectrpc.greet.v1.GreetRequest` → `GreetRequest`
fn extract_type_name(fqn: &str) -> String {
    fqn.rsplit('.').next().unwrap_or(fqn).to_string()
}

// ── Idempotency ──────────────────────────────────────────────────────────

/// Extract the idempotency level from method options, returning the Rust
/// expression that represents it.
#[must_use]
pub fn extract_idempotency(options: &Option<MethodOptions>) -> String {
    match options {
        Some(opts) => match opts.idempotency_level {
            Some(level) if level == method_options::IdempotencyLevel::NoSideEffects as i32 => {
                "foundation_connectrpc::IdempotencyLevel::NoSideEffects".to_string()
            }
            Some(level) if level == method_options::IdempotencyLevel::Idempotent as i32 => {
                "foundation_connectrpc::IdempotencyLevel::Idempotent".to_string()
            }
            _ => "foundation_connectrpc::IdempotencyLevel::Unknown".to_string(),
        },
        None => "foundation_connectrpc::IdempotencyLevel::Unknown".to_string(),
    }
}

/// Get the handler-options suffix for the given idempotency expression.
/// Returns `.with_idempotency(…)` if not Unknown, else empty.
fn idempotency_suffix(expr: &str) -> String {
    if expr == "foundation_connectrpc::IdempotencyLevel::Unknown" {
        String::new()
    } else {
        format!(".with_idempotency({expr})")
    }
}

// ── 1. Service name constant (R3) ───────────────────────────────────────

fn generate_name_constant(package: &str, svc_name: &str, out: &mut String) {
    let const_name = format!("{}_NAME", svc_name.to_uppercase());
    let qualified = format!("{package}.{svc_name}");
    writeln!(
        out,
        "/// Fully-qualified service name.\npub const {const_name}: &str = \"{qualified}\";\n"
    )
    .expect(WRITE_ERR);
}

// ── 2. Procedure path constants (R1) ─────────────────────────────────────

fn generate_procedure_module(
    package: &str,
    svc_name: &str,
    methods: &[MethodInfo],
    out: &mut String,
) {
    out.push_str("/// Procedure path constants — leading slash included (R1).\n");
    out.push_str("pub mod procedure {\n");
    for m in methods {
        let const_name = m.snake.trim_start_matches("r#").to_uppercase();
        let path = format!("/{package}.{svc_name}/{}", m.pascal);
        writeln!(out, "    /// Procedure path for \"{path}\".").expect(WRITE_ERR);
        writeln!(out, "    pub const {const_name}: &str = \"{path}\";").expect(WRITE_ERR);
    }
    out.push_str("}\n\n");
}

// ── 3. Service trait ─────────────────────────────────────────────────────

fn generate_service_trait(svc_name: &str, methods: &[MethodInfo], out: &mut String) {
    writeln!(out, "/// Server-side implementation trait for {svc_name}.").expect(WRITE_ERR);
    writeln!(out, "pub trait {svc_name}: Send + Sync + 'static {{").expect(WRITE_ERR);

    for m in methods {
        let ty = m.kind;
        let method_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;
        let path_const = m.snake.trim_start_matches("r#").to_uppercase();

        out.push_str("    /// Generated default returns `unimplemented`.\n");

        // Native `async fn` in a trait yields a **non-`Send`** `impl Future`, but
        // `Router::{unary,server_stream,…}` bound the handler future as `Send`.
        // So (mirroring the code-first `#[service]` macro) each method is emitted
        // as `fn … -> impl Future<Output = …> + Send { async move { … } }`, and
        // streaming outputs are boxed to `Pin<Box<dyn Stream + Send>>` since a
        // nested `impl Trait` inside the `Output` binding is illegal (E0562).
        match ty {
            MethodKind::Unary => {
                writeln!(out, "    fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, request: foundation_connectrpc::Request<{req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> impl ::core::future::Future<Output = foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<{res}>>> + Send").expect(WRITE_ERR);
                out.push_str("    {\n        async move {\n");
                writeln!(out, "            Err(foundation_connectrpc::ConnectError::unimplemented(procedure::{path_const}).into())").expect(WRITE_ERR);
                out.push_str("        }\n    }\n\n");
            }
            MethodKind::ServerStream => {
                writeln!(out, "    fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, request: foundation_connectrpc::Request<{req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> impl ::core::future::Future<Output = foundation_connectrpc::ConnectResult<::core::pin::Pin<Box<dyn futures::Stream<Item = foundation_connectrpc::ConnectResult<{res}>> + Send>>>> + Send").expect(WRITE_ERR);
                out.push_str("    {\n        async move {\n");
                writeln!(out, "            Err(foundation_connectrpc::ConnectError::unimplemented(procedure::{path_const}).into())").expect(WRITE_ERR);
                out.push_str("        }\n    }\n\n");
            }
            MethodKind::ClientStream => {
                writeln!(out, "    fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, requests: impl futures::Stream<Item = foundation_connectrpc::ConnectResult<{req}>> + Send + 'static)").expect(WRITE_ERR);
                writeln!(out, "        -> impl ::core::future::Future<Output = foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<{res}>>> + Send").expect(WRITE_ERR);
                out.push_str("    {\n        async move {\n");
                writeln!(out, "            Err(foundation_connectrpc::ConnectError::unimplemented(procedure::{path_const}).into())").expect(WRITE_ERR);
                out.push_str("        }\n    }\n\n");
            }
            MethodKind::BidiStream => {
                writeln!(out, "    fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, requests: impl futures::Stream<Item = foundation_connectrpc::ConnectResult<{req}>> + Send + 'static)").expect(WRITE_ERR);
                writeln!(out, "        -> impl ::core::future::Future<Output = foundation_connectrpc::ConnectResult<::core::pin::Pin<Box<dyn futures::Stream<Item = foundation_connectrpc::ConnectResult<{res}>> + Send>>>> + Send").expect(WRITE_ERR);
                out.push_str("    {\n        async move {\n");
                writeln!(out, "            Err(foundation_connectrpc::ConnectError::unimplemented(procedure::{path_const}).into())").expect(WRITE_ERR);
                out.push_str("        }\n    }\n\n");
            }
        }
    }

    out.push_str("}\n\n");
}

// ── 4. Registration function ─────────────────────────────────────────────

fn generate_registration(svc_name: &str, methods: &[MethodInfo], out: &mut String) {
    writeln!(
        out,
        "/// Register a {svc_name} implementation with a ConnectRPC router."
    )
    .expect(WRITE_ERR);
    writeln!(
        out,
        "pub fn register_{}<S: {svc_name}>(",
        to_snake_case(svc_name)
    )
    .expect(WRITE_ERR);
    out.push_str("    router: &mut foundation_connectrpc::Router,\n");
    out.push_str("    service: std::sync::Arc<S>,\n");
    out.push_str(") {\n");

    for m in methods {
        let path_const = m.snake.trim_start_matches("r#").to_uppercase();
        let req = &m.input_type;
        let res = &m.output_type;
        let method_name = &m.snake;
        let idem_suffix = idempotency_suffix(&m.idempotency_expr);

        out.push_str("    {\n");
        out.push_str("        let svc = service.clone();\n");

        let register_call = match m.kind {
            MethodKind::Unary => "unary",
            MethodKind::ServerStream => "server_stream",
            MethodKind::ClientStream => "client_stream",
            MethodKind::BidiStream => "bidi_stream",
        };

        writeln!(out, "        router.{register_call}(").expect(WRITE_ERR);
        writeln!(out, "            procedure::{path_const},").expect(WRITE_ERR);
        writeln!(
            out,
            "            foundation_connectrpc::ProcedureCodecs::<{req}, {res}>::defaults(),"
        )
        .expect(WRITE_ERR);

        match m.kind {
            MethodKind::Unary | MethodKind::ServerStream => {
                writeln!(out, "            move |ctx, req| {{ let svc = svc.clone(); async move {{ svc.{method_name}(ctx, req).await }} }},").expect(WRITE_ERR);
            }
            MethodKind::ClientStream | MethodKind::BidiStream => {
                writeln!(out, "            move |ctx, reqs| {{ let svc = svc.clone(); async move {{ svc.{method_name}(ctx, reqs).await }} }},").expect(WRITE_ERR);
            }
        }

        writeln!(
            out,
            "            foundation_connectrpc::HandlerOptions::new(){idem_suffix},"
        )
        .expect(WRITE_ERR);
        out.push_str("        );\n");
        out.push_str("    }\n\n");
    }

    out.push_str("}\n\n");
}

// ── 5. Unimplemented handler (R2) ────────────────────────────────────────

fn generate_unimplemented_handler(svc_name: &str, out: &mut String) {
    let struct_name = format!("Unimplemented{svc_name}Handler");

    writeln!(
        out,
        "/// An unimplemented {svc_name} handler — all methods return `unimplemented`."
    )
    .expect(WRITE_ERR);
    writeln!(out, "pub struct {struct_name};\n").expect(WRITE_ERR);

    writeln!(out, "impl {svc_name} for {struct_name} {{").expect(WRITE_ERR);
    out.push_str("    // All methods use the default `unimplemented` implementations.\n");
    out.push_str("}\n\n");
}

// ── 6. Typed client (R4) ─────────────────────────────────────────────────

fn generate_client(svc_name: &str, methods: &[MethodInfo], out: &mut String) {
    let client_name = format!("{svc_name}Client");
    let client_trait_name = format!("{svc_name}ClientExt");

    generate_client_struct(svc_name, methods, &client_name, out);
    generate_client_constructor(methods, &client_name, out);
    generate_client_methods(methods, out);
    generate_client_trait(svc_name, methods, &client_trait_name, out);
    generate_client_trait_impl(methods, &client_name, &client_trait_name, out);
}

/// Emit the client struct with one typed `Client` field per method.
fn generate_client_struct(
    svc_name: &str,
    methods: &[MethodInfo],
    client_name: &str,
    out: &mut String,
) {
    writeln!(out, "/// Typed client for {svc_name}.").expect(WRITE_ERR);
    writeln!(out, "pub struct {client_name} {{").expect(WRITE_ERR);

    for m in methods {
        let field_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;
        writeln!(
            out,
            "    {field_name}: foundation_connectrpc::Client<{req}, {res}>,"
        )
        .expect(WRITE_ERR);
    }
    out.push_str("}\n\n");
}

/// Emit the client `new` constructor.
fn generate_client_constructor(methods: &[MethodInfo], client_name: &str, out: &mut String) {
    writeln!(out, "impl {client_name} {{").expect(WRITE_ERR);

    out.push_str("    /// Build a new typed client.\n");
    out.push_str("    pub fn new(\n");
    out.push_str("        transport: std::sync::Arc<dyn foundation_connectrpc::Transport>,\n");
    out.push_str("        base_url: &str,\n");
    out.push_str("        options: foundation_connectrpc::ClientOptions,\n");
    out.push_str("    ) -> foundation_connectrpc::ConnectResult<Self> {\n");
    out.push_str("        Ok(Self {\n");

    for (i, m) in methods.iter().enumerate() {
        let field_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;
        let path_const = m.snake.trim_start_matches("r#").to_uppercase();
        let idem_suffix = idempotency_suffix(&m.idempotency_expr);
        let is_last = i == methods.len() - 1;

        writeln!(
            out,
            "            {field_name}: foundation_connectrpc::Client::new("
        )
        .expect(WRITE_ERR);
        out.push_str("                transport.clone(),\n");
        writeln!(
            out,
            "                &format!(\"{{}}{{}}\", base_url, procedure::{path_const}),"
        )
        .expect(WRITE_ERR);
        writeln!(
            out,
            "                foundation_connectrpc::ProcedureCodecs::<{req}, {res}>::defaults(),"
        )
        .expect(WRITE_ERR);
        if is_last {
            writeln!(out, "                options{idem_suffix},").expect(WRITE_ERR);
        } else {
            writeln!(out, "                options.clone(){idem_suffix},").expect(WRITE_ERR);
        }
        out.push_str("            )?,\n");
    }

    out.push_str("        })\n");
    out.push_str("    }\n\n");
}

/// Emit the per-method helpers on the client struct.
fn generate_client_methods(methods: &[MethodInfo], out: &mut String) {
    for m in methods {
        let method_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;

        match m.kind {
            MethodKind::Unary => {
                out.push_str("    /// Unary RPC.\n");
                writeln!(out, "    pub async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, request: foundation_connectrpc::Request<{req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<{res}>>").expect(WRITE_ERR);
                out.push_str("    {\n");
                writeln!(out, "        self.{method_name}.unary(ctx, request).await")
                    .expect(WRITE_ERR);
                out.push_str("    }\n\n");
            }
            MethodKind::ServerStream => {
                out.push_str("    /// Server-streaming RPC.\n");
                writeln!(out, "    pub async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, request: foundation_connectrpc::Request<{req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::ServerStream<{res}>>").expect(WRITE_ERR);
                out.push_str("    {\n");
                writeln!(
                    out,
                    "        self.{method_name}.server_stream(ctx, request).await"
                )
                .expect(WRITE_ERR);
                out.push_str("    }\n\n");
            }
            MethodKind::ClientStream => {
                out.push_str("    /// Client-streaming RPC.\n");
                writeln!(out, "    pub async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, reqs: impl futures::Stream<Item = {req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<{res}>>").expect(WRITE_ERR);
                out.push_str("    {\n");
                writeln!(
                    out,
                    "        self.{method_name}.client_stream(ctx, reqs).await"
                )
                .expect(WRITE_ERR);
                out.push_str("    }\n\n");
            }
            MethodKind::BidiStream => {
                out.push_str("    /// Bidirectional streaming RPC.\n");
                writeln!(
                    out,
                    "    pub async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx)"
                )
                .expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::BidiStream<{req}, {res}>>").expect(WRITE_ERR);
                out.push_str("    {\n");
                writeln!(out, "        self.{method_name}.bidi_stream(ctx).await")
                    .expect(WRITE_ERR);
                out.push_str("    }\n\n");
            }
        }
    }

    out.push_str("}\n\n");
}

/// Emit the client trait declaration (`<Service>ClientExt`).
fn generate_client_trait(
    svc_name: &str,
    methods: &[MethodInfo],
    client_trait_name: &str,
    out: &mut String,
) {
    writeln!(
        out,
        "/// Trait for {svc_name} client behaviour (supports mocking/testing)."
    )
    .expect(WRITE_ERR);
    writeln!(
        out,
        "pub trait {client_trait_name}: Send + Sync + 'static {{"
    )
    .expect(WRITE_ERR);

    for m in methods {
        let method_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;

        match m.kind {
            MethodKind::Unary => {
                writeln!(out, "    async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, request: foundation_connectrpc::Request<{req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<{res}>>;").expect(WRITE_ERR);
            }
            MethodKind::ServerStream => {
                writeln!(out, "    async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, request: foundation_connectrpc::Request<{req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::ServerStream<{res}>>;").expect(WRITE_ERR);
            }
            MethodKind::ClientStream => {
                writeln!(out, "    async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, reqs: impl futures::Stream<Item = {req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<{res}>>;").expect(WRITE_ERR);
            }
            MethodKind::BidiStream => {
                writeln!(
                    out,
                    "    async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx)"
                )
                .expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::BidiStream<{req}, {res}>>;").expect(WRITE_ERR);
            }
        }
    }

    out.push_str("}\n\n");
}

/// Emit the blanket impl of the client trait for the client struct.
fn generate_client_trait_impl(
    methods: &[MethodInfo],
    client_name: &str,
    client_trait_name: &str,
    out: &mut String,
) {
    writeln!(out, "impl {client_trait_name} for {client_name} {{").expect(WRITE_ERR);

    for m in methods {
        let method_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;

        match m.kind {
            MethodKind::Unary => {
                writeln!(out, "    async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, request: foundation_connectrpc::Request<{req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<{res}>>").expect(WRITE_ERR);
                out.push_str("    {\n");
                writeln!(out, "        self.{method_name}.unary(ctx, request).await")
                    .expect(WRITE_ERR);
                out.push_str("    }\n");
            }
            MethodKind::ServerStream => {
                writeln!(out, "    async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, request: foundation_connectrpc::Request<{req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::ServerStream<{res}>>").expect(WRITE_ERR);
                out.push_str("    {\n");
                writeln!(
                    out,
                    "        self.{method_name}.server_stream(ctx, request).await"
                )
                .expect(WRITE_ERR);
                out.push_str("    }\n");
            }
            MethodKind::ClientStream => {
                writeln!(out, "    async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx, reqs: impl futures::Stream<Item = {req}>)").expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<{res}>>").expect(WRITE_ERR);
                out.push_str("    {\n");
                writeln!(
                    out,
                    "        self.{method_name}.client_stream(ctx, reqs).await"
                )
                .expect(WRITE_ERR);
                out.push_str("    }\n");
            }
            MethodKind::BidiStream => {
                writeln!(
                    out,
                    "    async fn {method_name}(&self, ctx: foundation_connectrpc::RpcCtx)"
                )
                .expect(WRITE_ERR);
                writeln!(out, "        -> foundation_connectrpc::ConnectResult<foundation_connectrpc::BidiStream<{req}, {res}>>").expect(WRITE_ERR);
                out.push_str("    {\n");
                writeln!(out, "        self.{method_name}.bidi_stream(ctx).await")
                    .expect(WRITE_ERR);
                out.push_str("    }\n");
            }
        }
    }

    out.push_str("}\n");
}

// ── Tests ────────────────────────────────────────────────────────────────
