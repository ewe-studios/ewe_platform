//! Generator tests moved out of `src/generator.rs` (spec 70 feature 59,
//! rule 7).
//!
//! `to_snake_case` and `extract_idempotency` were private helpers; they are
//! now public and re-exported so the same assertions stay covered.

use foundation_connectrpc_codegen::{extract_idempotency, generate_services, to_snake_case};
use prost_types::{
    method_options, FileDescriptorProto, MethodDescriptorProto, MethodOptions,
    ServiceDescriptorProto,
};

/// WHY: TDD first test — verify `PascalCase` to `snake_case` conversion.
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
/// WHAT: Generate code for the D10 example `GreetService` and check key
/// markers in the output.
#[test]
fn test_generate_full_service() {
    let file = make_greet_file();
    let code = generate_services(&[file]);

    // Service name constant (R3)
    assert!(code.contains("pub const GREETSERVICE_NAME: &str"));
    assert!(code.contains("\"connectrpc.greet.v1.GreetService\""));

    // Procedure constants with leading slash (R1)
    assert!(code.contains("pub const GREET: &str = \"/connectrpc.greet.v1.GreetService/Greet\""));
    assert!(code.contains(
        "pub const GREET_GROUP: &str = \"/connectrpc.greet.v1.GreetService/GreetGroup\""
    ));
    assert!(code.contains(
        "pub const GREET_INDIVIDUALS: &str = \"/connectrpc.greet.v1.GreetService/GreetIndividuals\""
    ));
    assert!(
        code.contains("pub const CONVERSE: &str = \"/connectrpc.greet.v1.GreetService/Converse\"")
    );

    // Service trait
    assert!(code.contains("pub trait GreetService: Send + Sync + 'static"));
    // Trait methods are emitted as RPITIT `-> impl Future<…> + Send` (not
    // `async fn`) so the Router's `Send` handler bound is satisfied; streaming
    // outputs are boxed to `Pin<Box<dyn Stream + Send>>`.
    assert!(code.contains("fn greet(&self, ctx: foundation_connectrpc::RpcCtx"));
    assert!(code.contains("-> impl ::core::future::Future<Output = foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<GreetResponse>>> + Send"));
    assert!(code.contains("::core::pin::Pin<Box<dyn futures::Stream<Item = foundation_connectrpc::ConnectResult<ConverseResponse>> + Send>>"));

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
/// WHAT: A method with `NO_SIDE_EFFECTS` should produce
/// `.with_idempotency(foundation_connectrpc::IdempotencyLevel::NoSideEffects)`.
#[test]
fn test_idempotency_extraction() {
    let options = MethodOptions {
        idempotency_level: Some(method_options::IdempotencyLevel::NoSideEffects as i32),
        ..Default::default()
    };
    let expr = extract_idempotency(&Some(options));
    assert_eq!(
        expr,
        "foundation_connectrpc::IdempotencyLevel::NoSideEffects"
    );
}

/// WHY: Verify idempotent level extraction.
/// WHAT: IDEMPOTENT maps to `IdempotencyLevel::Idempotent`.
#[test]
fn test_idempotent_extraction() {
    let options = MethodOptions {
        idempotency_level: Some(method_options::IdempotencyLevel::Idempotent as i32),
        ..Default::default()
    };
    let expr = extract_idempotency(&Some(options));
    assert_eq!(expr, "foundation_connectrpc::IdempotencyLevel::Idempotent");
}

/// WHY: Verify that missing idempotency level defaults to Unknown.
/// WHAT: A method with no options should use Unknown.
#[test]
fn test_default_idempotency() {
    let expr = extract_idempotency(&None);
    assert_eq!(expr, "foundation_connectrpc::IdempotencyLevel::Unknown");
}

/// WHY: Verify generated code references `foundation_connectrpc::Client` properly.
/// WHAT: The client struct should contain `Client<Req,Res>` fields.
#[test]
fn test_client_struct_fields() {
    let file = make_greet_file();
    let code = generate_services(&[file]);
    assert!(code.contains("greet: foundation_connectrpc::Client<GreetRequest, GreetResponse>"));
    assert!(code
        .contains("greet_group: foundation_connectrpc::Client<GreetRequest, GreetGroupResponse>"));
    assert!(code
        .contains("greet_individuals: foundation_connectrpc::Client<GreetRequest, GreetResponse>"));
    assert!(
        code.contains("converse: foundation_connectrpc::Client<ConverseRequest, ConverseResponse>")
    );
}

/// WHY: Verify the registration function calls the right router methods.
/// WHAT: Check router.unary, `router.server_stream`, `router.client_stream`,
/// `router.bidi_stream` are all emitted.
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
    assert!(code.contains(".bidi_stream(ctx).await"));
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
