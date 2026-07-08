//! Proto-to-Rust code generation for ConnectRPC services (Decision 10).
//!
//! WHY: Every ConnectRPC service needs a server trait, handler registration, and
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
//! streaming attributes determine the four RPC shapes (unary, client_stream,
//! server_stream, bidi_stream). PascalCase method names are lowered to snake_case.
//! Idempotency levels are extracted from proto method options (field 34 on
//! MethodOptions).

use prost_types::{
    method_options, FileDescriptorProto, MethodDescriptorProto, MethodOptions,
    ServiceDescriptorProto,
};

// ── Public entry point ───────────────────────────────────────────────────

/// Generate ConnectRPC service code from proto file descriptors.
/// Returns Rust source code as a `String`.
///
/// Each file descriptor contributes its services. The output is ready to be
/// written to a `.rs` file included via `include!` or a protoc plugin response.
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
    out.push_str(&format!("// {svc_name}\n"));
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
        let snake = to_snake_case(&pascal);
        let input_type = extract_type_name(m.input_type.as_deref().unwrap_or("UnknownRequest"));
        let output_type =
            extract_type_name(m.output_type.as_deref().unwrap_or("UnknownResponse"));
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

/// Convert PascalCase to snake_case.
///
/// `GreetGroup` → `greet_group`, `GetURL` → `get_url`, `Greet` → `greet`.
fn to_snake_case(name: &str) -> String {
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
                let next_lower = next.map_or(false, |n| n.is_lowercase());
                if prev_lower || (next_lower && i + 1 < len) {
                    if result.as_bytes().last() != Some(&b'_') {
                        result.push('_');
                    }
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
fn extract_idempotency(options: &Option<MethodOptions>) -> String {
    match options {
        Some(opts) => match opts.idempotency_level {
            Some(level) if level == method_options::IdempotencyLevel::NoSideEffects as i32 => {
                "connectrpc::IdempotencyLevel::NoSideEffects".to_string()
            }
            Some(level) if level == method_options::IdempotencyLevel::Idempotent as i32 => {
                "connectrpc::IdempotencyLevel::Idempotent".to_string()
            }
            _ => "connectrpc::IdempotencyLevel::Unknown".to_string(),
        },
        None => "connectrpc::IdempotencyLevel::Unknown".to_string(),
    }
}

/// Get the handler-options suffix for the given idempotency expression.
/// Returns `.with_idempotency(…)` if not Unknown, else empty.
fn idempotency_suffix(expr: &str) -> String {
    if expr == "connectrpc::IdempotencyLevel::Unknown" {
        String::new()
    } else {
        format!(".with_idempotency({expr})")
    }
}

// ── 1. Service name constant (R3) ───────────────────────────────────────

fn generate_name_constant(package: &str, svc_name: &str, out: &mut String) {
    let const_name = format!("{}_NAME", svc_name.to_uppercase());
    let qualified = format!("{package}.{svc_name}");
    out.push_str(&format!(
        "/// Fully-qualified service name.\npub const {const_name}: &str = \"{qualified}\";\n\n"
    ));
}

// ── 2. Procedure path constants (R1) ─────────────────────────────────────

fn generate_procedure_module(package: &str, svc_name: &str, methods: &[MethodInfo], out: &mut String) {
    out.push_str("/// Procedure path constants — leading slash included (R1).\n");
    out.push_str("pub mod procedure {\n");
    for m in methods {
        let const_name = m.snake.to_uppercase();
        let path = format!("/{package}.{svc_name}/{}", m.pascal);
        out.push_str(&format!("    /// Procedure path for \"{path}\".\n"));
        out.push_str(&format!("    pub const {const_name}: &str = \"{path}\";\n"));
    }
    out.push_str("}\n\n");
}

// ── 3. Service trait ─────────────────────────────────────────────────────

fn generate_service_trait(svc_name: &str, methods: &[MethodInfo], out: &mut String) {
    out.push_str(&format!(
        "/// Server-side implementation trait for {svc_name}.\n"
    ));
    out.push_str(&format!(
        "pub trait {svc_name}: Send + Sync + 'static {{\n"
    ));

    for m in methods {
        let ty = m.kind;
        let method_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;
        let path_const = m.snake.to_uppercase();

        out.push_str("    /// Generated default returns `unimplemented`.\n");

        match ty {
            MethodKind::Unary => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, request: connectrpc::Request<{req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::Response<{res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        Err(connectrpc::ConnectError::unimplemented(procedure::{path_const}).into())\n"
                ));
                out.push_str("    }\n\n");
            }
            MethodKind::ServerStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, request: connectrpc::Request<{req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<impl futures::Stream<Item = connectrpc::ConnectResult<{res}>> + Send>\n"
                ));
                // Return pinned hidden type with stream::Empty
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        let r: connectrpc::ConnectResult<futures::stream::Empty<connectrpc::ConnectResult<{res}>>> = \n"
                ));
                out.push_str(&format!(
                    "            Err(connectrpc::ConnectError::unimplemented(procedure::{path_const}).into());\n"
                ));
                out.push_str("        r\n");
                out.push_str("    }\n\n");
            }
            MethodKind::ClientStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, requests: impl futures::Stream<Item = connectrpc::ConnectResult<{req}>>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::Response<{res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        Err(connectrpc::ConnectError::unimplemented(procedure::{path_const}).into())\n"
                ));
                out.push_str("    }\n\n");
            }
            MethodKind::BidiStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, requests: impl futures::Stream<Item = connectrpc::ConnectResult<{req}>>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<impl futures::Stream<Item = connectrpc::ConnectResult<{res}>> + Send>\n"
                ));
                // Return pinned hidden type with stream::Empty
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        let r: connectrpc::ConnectResult<futures::stream::Empty<connectrpc::ConnectResult<{res}>>> = \n"
                ));
                out.push_str(&format!(
                    "            Err(connectrpc::ConnectError::unimplemented(procedure::{path_const}).into());\n"
                ));
                out.push_str("        r\n");
                out.push_str("    }\n\n");
            }
        }
    }

    out.push_str("}\n\n");
}

// ── 4. Registration function ─────────────────────────────────────────────

fn generate_registration(svc_name: &str, methods: &[MethodInfo], out: &mut String) {
    out.push_str(&format!(
        "/// Register a {svc_name} implementation with a ConnectRPC router.\n"
    ));
    out.push_str(&format!(
        "pub fn register_{}<S: {svc_name}>(\n",
        to_snake_case(svc_name)
    ));
    out.push_str("    router: &mut connectrpc::Router,\n");
    out.push_str("    service: std::sync::Arc<S>,\n");
    out.push_str(") {\n");

    for m in methods {
        let path_const = m.snake.to_uppercase();
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

        out.push_str(&format!("        router.{register_call}(\n"));
        out.push_str(&format!("            procedure::{path_const},\n"));
        out.push_str(&format!(
            "            connectrpc::ProcedureCodecs::<{req}, {res}>::defaults(),\n"
        ));

        match m.kind {
            MethodKind::Unary | MethodKind::ServerStream => {
                out.push_str(&format!(
                    "            move |ctx, req| {{ let svc = svc.clone(); async move {{ svc.{method_name}(ctx, req).await }} }},\n"
                ));
            }
            MethodKind::ClientStream | MethodKind::BidiStream => {
                out.push_str(&format!(
                    "            move |ctx, reqs| {{ let svc = svc.clone(); async move {{ svc.{method_name}(ctx, reqs).await }} }},\n"
                ));
            }
        }

        out.push_str(&format!(
            "            connectrpc::HandlerOptions::new(){idem_suffix},\n"
        ));
        out.push_str("        );\n");
        out.push_str("    }\n\n");
    }

    out.push_str("}\n\n");
}

// ── 5. Unimplemented handler (R2) ────────────────────────────────────────

fn generate_unimplemented_handler(svc_name: &str, out: &mut String) {
    let struct_name = format!("Unimplemented{svc_name}Handler");

    out.push_str(&format!(
        "/// An unimplemented {svc_name} handler — all methods return `unimplemented`.\n"
    ));
    out.push_str(&format!("pub struct {struct_name};\n\n"));

    out.push_str(&format!("impl {svc_name} for {struct_name} {{\n"));
    out.push_str("    // All methods use the default `unimplemented` implementations.\n");
    out.push_str("}\n\n");
}

// ── 6. Typed client (R4) ─────────────────────────────────────────────────

fn generate_client(svc_name: &str, methods: &[MethodInfo], out: &mut String) {
    let client_name = format!("{svc_name}Client");
    let client_trait_name = format!("{svc_name}ClientExt");

    // ── Client struct ────────────────────────────────────────────────────
    out.push_str(&format!("/// Typed client for {svc_name}.\n"));
    out.push_str(&format!("pub struct {client_name} {{\n"));

    for m in methods {
        let field_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;
        out.push_str(&format!(
            "    {field_name}: connectrpc::Client<{req}, {res}>,\n"
        ));
    }
    out.push_str("}\n\n");

    // ── Client constructor ───────────────────────────────────────────────
    out.push_str(&format!("impl {client_name} {{\n"));

    out.push_str("    /// Build a new typed client.\n");
    out.push_str("    pub fn new(\n");
    out.push_str("        transport: std::sync::Arc<dyn connectrpc::Transport>,\n");
    out.push_str("        base_url: &str,\n");
    out.push_str("        options: connectrpc::ClientOptions,\n");
    out.push_str("    ) -> connectrpc::ConnectResult<Self> {\n");
    out.push_str("        Ok(Self {\n");

    for (i, m) in methods.iter().enumerate() {
        let field_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;
        let path_const = m.snake.to_uppercase();
        let idem_suffix = idempotency_suffix(&m.idempotency_expr);
        let is_last = i == methods.len() - 1;

        if !is_last {
            out.push_str(&format!(
                "            {field_name}: connectrpc::Client::new(\n"
            ));
            out.push_str("                transport.clone(),\n");
            out.push_str(&format!(
                "                &format!(\"{{}}{{}}\", base_url, procedure::{path_const}),\n"
            ));
            out.push_str(&format!(
                "                connectrpc::ProcedureCodecs::<{req}, {res}>::defaults(),\n"
            ));
            out.push_str(&format!("                options.clone(){idem_suffix},\n"));
            out.push_str("            )?,\n");
        } else {
            out.push_str(&format!(
                "            {field_name}: connectrpc::Client::new(\n"
            ));
            out.push_str("                transport.clone(),\n");
            out.push_str(&format!(
                "                &format!(\"{{}}{{}}\", base_url, procedure::{path_const}),\n"
            ));
            out.push_str(&format!(
                "                connectrpc::ProcedureCodecs::<{req}, {res}>::defaults(),\n"
            ));
            out.push_str(&format!("                options{idem_suffix},\n"));
            out.push_str("            )?,\n");
        }
    }

    out.push_str("        })\n");
    out.push_str("    }\n\n");

    // ── Per-method client methods ────────────────────────────────────────
    for m in methods {
        let method_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;

        match m.kind {
            MethodKind::Unary => {
                out.push_str("    /// Unary RPC.\n");
                out.push_str(&format!(
                    "    pub async fn {method_name}(&self, ctx: connectrpc::Ctx, request: connectrpc::Request<{req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::Response<{res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        self.{method_name}.unary(ctx, request).await\n"
                ));
                out.push_str("    }\n\n");
            }
            MethodKind::ServerStream => {
                out.push_str("    /// Server-streaming RPC.\n");
                out.push_str(&format!(
                    "    pub async fn {method_name}(&self, ctx: connectrpc::Ctx, request: connectrpc::Request<{req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::ServerStream<{res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        self.{method_name}.server_stream(ctx, request).await\n"
                ));
                out.push_str("    }\n\n");
            }
            MethodKind::ClientStream => {
                out.push_str("    /// Client-streaming RPC.\n");
                out.push_str(&format!(
                    "    pub async fn {method_name}(&self, ctx: connectrpc::Ctx, reqs: impl futures::Stream<Item = {req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::Response<{res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        self.{method_name}.client_stream(ctx, reqs).await\n"
                ));
                out.push_str("    }\n\n");
            }
            MethodKind::BidiStream => {
                out.push_str("    /// Bidirectional streaming RPC.\n");
                out.push_str(&format!(
                    "    pub async fn {method_name}(&self, ctx: connectrpc::Ctx, reqs: impl futures::Stream<Item = {req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::BidiStream<{req}, {res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        self.{method_name}.bidi_stream(ctx, reqs).await\n"
                ));
                out.push_str("    }\n\n");
            }
        }
    }

    out.push_str("}\n\n");

    // ── Client trait (R4) ────────────────────────────────────────────────
    out.push_str(&format!(
        "/// Trait for {svc_name} client behaviour (supports mocking/testing).\n"
    ));
    out.push_str(&format!(
        "pub trait {client_trait_name}: Send + Sync + 'static {{\n"
    ));

    for m in methods {
        let method_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;

        match m.kind {
            MethodKind::Unary => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, request: connectrpc::Request<{req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::Response<{res}>>;\n"
                ));
            }
            MethodKind::ServerStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, request: connectrpc::Request<{req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::ServerStream<{res}>>;\n"
                ));
            }
            MethodKind::ClientStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, reqs: impl futures::Stream<Item = {req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::Response<{res}>>;\n"
                ));
            }
            MethodKind::BidiStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, reqs: impl futures::Stream<Item = {req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::BidiStream<{req}, {res}>>;\n"
                ));
            }
        }
    }

    out.push_str("}\n\n");

    // Blanket impl of client trait for the client struct
    out.push_str(&format!(
        "impl {client_trait_name} for {client_name} {{\n"
    ));

    for m in methods {
        let method_name = &m.snake;
        let req = &m.input_type;
        let res = &m.output_type;

        match m.kind {
            MethodKind::Unary => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, request: connectrpc::Request<{req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::Response<{res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        self.{method_name}.unary(ctx, request).await\n"
                ));
                out.push_str("    }\n");
            }
            MethodKind::ServerStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, request: connectrpc::Request<{req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::ServerStream<{res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        self.{method_name}.server_stream(ctx, request).await\n"
                ));
                out.push_str("    }\n");
            }
            MethodKind::ClientStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, reqs: impl futures::Stream<Item = {req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::Response<{res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        self.{method_name}.client_stream(ctx, reqs).await\n"
                ));
                out.push_str("    }\n");
            }
            MethodKind::BidiStream => {
                out.push_str(&format!(
                    "    async fn {method_name}(&self, ctx: connectrpc::Ctx, reqs: impl futures::Stream<Item = {req}>)\n"
                ));
                out.push_str(&format!(
                    "        -> connectrpc::ConnectResult<connectrpc::BidiStream<{req}, {res}>>\n"
                ));
                out.push_str("    {\n");
                out.push_str(&format!(
                    "        self.{method_name}.bidi_stream(ctx, reqs).await\n"
                ));
                out.push_str("    }\n");
            }
        }
    }

    out.push_str("}\n");
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: TDD first test — verify PascalCase to snake_case conversion.
    /// WHAT: Edge cases: single word, compound, acronyms.
    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("Greet"), "greet");
        assert_eq!(to_snake_case("GreetGroup"), "greet_group");
        assert_eq!(to_snake_case("GreetIndividuals"), "greet_individuals");
        assert_eq!(to_snake_case("GetUser"), "get_user");
        assert_eq!(to_snake_case("ParseURL"), "parse_url");
        assert_eq!(to_snake_case("HTML"), "html");
        assert_eq!(to_snake_case("Simple"), "simple");
    }

    /// WHY: Verify the generator handles a full service with all four RPC kinds.
    /// WHAT: Generate code for the D10 example GreetService and check key
    /// markers in the output.
    #[test]
    fn test_generate_full_service() {
        let file = make_greet_file();
        let code = generate_services(&[file]);

        // Service name constant (R3)
        assert!(code.contains("pub const GREETSERVICE_NAME: &str"));
        assert!(code.contains("\"connectrpc.greet.v1.GreetService\""));

        // Procedure constants with leading slash (R1)
        assert!(code.contains(
            "pub const GREET: &str = \"/connectrpc.greet.v1.GreetService/Greet\""
        ));
        assert!(code.contains(
            "pub const GREET_GROUP: &str = \"/connectrpc.greet.v1.GreetService/GreetGroup\""
        ));
        assert!(code.contains(
            "pub const GREET_INDIVIDUALS: &str = \"/connectrpc.greet.v1.GreetService/GreetIndividuals\""
        ));
        assert!(code.contains(
            "pub const CONVERSE: &str = \"/connectrpc.greet.v1.GreetService/Converse\""
        ));

        // Service trait
        assert!(code.contains("pub trait GreetService: Send + Sync + 'static"));
        assert!(code.contains("async fn greet("));
        assert!(code.contains("async fn greet_group("));
        assert!(code.contains("async fn greet_individuals("));
        assert!(code.contains("async fn converse("));

        // Unimplemented handler (R2)
        assert!(code.contains("pub struct UnimplementedGreetServiceHandler;"));
        assert!(code.contains("impl GreetService for UnimplementedGreetServiceHandler"));

        // Registration function
        assert!(code.contains("pub fn register_greet_service<"));

        // Client struct (R4)
        assert!(code.contains("pub struct GreetServiceClient"));
        assert!(code.contains("impl GreetServiceClient"));

        // Client trait
        assert!(code.contains("pub trait GreetServiceClientExt"));
        assert!(code.contains("impl GreetServiceClientExt for GreetServiceClient"));
    }

    /// WHY: Verify that idempotency level is extracted from method options.
    /// WHAT: A method with NO_SIDE_EFFECTS should produce
    /// `.with_idempotency(connectrpc::IdempotencyLevel::NoSideEffects)`.
    #[test]
    fn test_idempotency_extraction() {
        let options = MethodOptions {
            idempotency_level: Some(
                method_options::IdempotencyLevel::NoSideEffects as i32,
            ),
            ..Default::default()
        };
        let expr = extract_idempotency(&Some(options));
        assert_eq!(expr, "connectrpc::IdempotencyLevel::NoSideEffects");
    }

    /// WHY: Verify idempotent level extraction.
    /// WHAT: IDEMPOTENT maps to IdempotencyLevel::Idempotent.
    #[test]
    fn test_idempotent_extraction() {
        let options = MethodOptions {
            idempotency_level: Some(
                method_options::IdempotencyLevel::Idempotent as i32,
            ),
            ..Default::default()
        };
        let expr = extract_idempotency(&Some(options));
        assert_eq!(expr, "connectrpc::IdempotencyLevel::Idempotent");
    }

    /// WHY: Verify that missing idempotency level defaults to Unknown.
    /// WHAT: A method with no options should use Unknown.
    #[test]
    fn test_default_idempotency() {
        let expr = extract_idempotency(&None);
        assert_eq!(expr, "connectrpc::IdempotencyLevel::Unknown");
    }

    /// WHY: Verify generated code references `connectrpc::Client` properly.
    /// WHAT: The client struct should contain `Client<Req,Res>` fields.
    #[test]
    fn test_client_struct_fields() {
        let file = make_greet_file();
        let code = generate_services(&[file]);
        assert!(code.contains("greet: connectrpc::Client<GreetRequest, GreetResponse>"));
        assert!(code.contains("greet_group: connectrpc::Client<GreetRequest, GreetGroupResponse>"));
        assert!(code.contains(
            "greet_individuals: connectrpc::Client<GreetRequest, GreetResponse>"
        ));
        assert!(code.contains(
            "converse: connectrpc::Client<ConverseRequest, ConverseResponse>"
        ));
    }

    /// WHY: Verify the registration function calls the right router methods.
    /// WHAT: Check router.unary, router.server_stream, router.client_stream,
    /// router.bidi_stream are all emitted.
    #[test]
    fn test_registration_router_calls() {
        let file = make_greet_file();
        let code = generate_services(&[file]);
        assert!(code.contains("router.unary("));
        assert!(code.contains("router.server_stream("));
        assert!(code.contains("router.client_stream("));
        assert!(code.contains("router.bidi_stream("));
    }

    /// WHY: Verify the Python protobuf naming `snake_case` works in the method type.
    /// WHAT: Client-streaming calls use `ClientStream` as receiver method.
    #[test]
    fn test_client_method_names() {
        let file = make_greet_file();
        let code = generate_services(&[file]);
        assert!(code.contains(".unary(ctx, request).await"));
        assert!(code.contains(".server_stream(ctx, request).await"));
        assert!(code.contains(".client_stream(ctx, reqs).await"));
        assert!(code.contains(".bidi_stream(ctx, reqs).await"));
    }

    // ── Test helpers ─────────────────────────────────────────────────────

    fn make_greet_file() -> FileDescriptorProto {
        FileDescriptorProto {
            name: Some("greet.proto".to_string()),
            package: Some("connectrpc.greet.v1".to_string()),
            syntax: Some("proto3".to_string()),
            service: vec![ServiceDescriptorProto {
                name: Some("GreetService".to_string()),
                method: vec![
                    MethodDescriptorProto {
                        name: Some("Greet".to_string()),
                        input_type: Some(".connectrpc.greet.v1.GreetRequest".to_string()),
                        output_type: Some(".connectrpc.greet.v1.GreetResponse".to_string()),
                        client_streaming: Some(false),
                        server_streaming: Some(false),
                        options: Some(MethodOptions {
                            idempotency_level: Some(
                                method_options::IdempotencyLevel::NoSideEffects as i32,
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    MethodDescriptorProto {
                        name: Some("GreetGroup".to_string()),
                        input_type: Some(".connectrpc.greet.v1.GreetRequest".to_string()),
                        output_type: Some(".connectrpc.greet.v1.GreetGroupResponse".to_string()),
                        client_streaming: Some(true),
                        server_streaming: Some(false),
                        ..Default::default()
                    },
                    MethodDescriptorProto {
                        name: Some("GreetIndividuals".to_string()),
                        input_type: Some(".connectrpc.greet.v1.GreetRequest".to_string()),
                        output_type: Some(".connectrpc.greet.v1.GreetResponse".to_string()),
                        client_streaming: Some(false),
                        server_streaming: Some(true),
                        ..Default::default()
                    },
                    MethodDescriptorProto {
                        name: Some("Converse".to_string()),
                        input_type: Some(".connectrpc.greet.v1.ConverseRequest".to_string()),
                        output_type: Some(".connectrpc.greet.v1.ConverseResponse".to_string()),
                        client_streaming: Some(true),
                        server_streaming: Some(true),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }
    }
}
