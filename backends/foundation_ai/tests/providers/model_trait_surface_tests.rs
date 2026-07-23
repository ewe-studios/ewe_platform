//! The `Model` trait surface: `spec` / `descriptor` / `costing` / `tool_formatter`.
//!
//! WHY: every provider implements these four, and callers use them to decide
//! how to talk to a model — `descriptor().cost` prices a call before it is made,
//! `tool_formatter()` decides whether tools go as native JSON or XML text, and
//! `costing()` is the running total a budget check reads. All four were
//! uncovered on the HTTP providers: the suites resolve a model and immediately
//! generate, never inspecting it. A wrong descriptor silently misprices; a wrong
//! formatter sends tools in a shape the vendor cannot parse.
//!
//! WHAT: the four accessors on the Anthropic, OpenAI and Responses models,
//! asserting each reports its own provider rather than a copy-pasted neighbour's.
//!
//! HOW: Anthropic needs no network to resolve a model (it synthesises the spec);
//! the OpenAI-shaped providers resolve against a `TestHttpServer` catalog.


use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};
use foundation_ai::types::{Model, ModelId, ModelProvider, ModelProviders};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::valtron_test;
use foundation_netio::{DynNetClient, HttpClientBuilder};
use foundation_testing::http::{HttpResponse, TestHttpServer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const CATALOG: &[u8] = br#"{
    "object": "list",
    "data": [{"id": "gpt-4o", "object": "model", "created": 1, "owned_by": "openai"}]
}"#;

fn catalog_server() -> TestHttpServer {
    TestHttpServer::with_response(|_req| HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), CATALOG.len().to_string()),
            ("Connection".to_string(), "close".to_string()),
        ],
        body: CATALOG.to_vec(),
    })
}

fn secret() -> AuthCredential {
    AuthCredential::SecretOnly(ConfidentialText::new("test-key".to_string()))
}

fn anthropic_model() -> impl Model {
    let cfg = AnthropicConfig::new().with_auth(secret());
    AnthropicMessagesProvider::new()
        .create(Some(cfg))
        .expect("provider builds")
        .get_model(ModelId::Name("claude-sonnet-4-6".into(), None))
        .expect("anthropic resolves without network")
}

// ---------------------------------------------------------------------------
// Anthropic
// ---------------------------------------------------------------------------

#[test]
fn anthropic_spec_reports_the_requested_model() {
    let m = anthropic_model();
    assert_eq!(
        m.spec().name,
        "claude-sonnet-4-6",
        "spec must name the model actually loaded"
    );
}

#[test]
fn anthropic_descriptor_identifies_anthropic() {
    let d = anthropic_model()
        .descriptor()
        .expect("an HTTP provider must expose a descriptor");
    assert_eq!(d.id, "anthropic");
    assert_eq!(d.provider, ModelProviders::ANTHROPIC);
    assert!(d.reasoning, "Claude models support reasoning");
}

#[test]
fn anthropic_costing_starts_at_zero() {
    // The accumulator is per-model-instance; a fresh model must not inherit a
    // previous instance's spend.
    let usage = anthropic_model().costing().expect("costing is available");
    assert_eq!(usage.total_tokens, 0.0);
}

#[test]
fn anthropic_tool_formatter_is_available() {
    // The formatter decides whether tools go as native JSON or XML text — a
    // missing one would make tool calls unencodable.
    let m = anthropic_model();
    let formatter = m.tool_formatter();
    // Anthropic uses its native format, so it needs no prompt instructions.
    assert!(
        formatter.tool_calling_instructions().is_none(),
        "a native-tool provider must not inject XML prompt instructions"
    );
}

// ---------------------------------------------------------------------------
// OpenAI Chat Completions
// ---------------------------------------------------------------------------

#[valtron_test]
fn openai_model_trait_surface() {
    let server = catalog_server();
    let http_client: DynNetClient = HttpClientBuilder::new().build();
    let cfg = OpenAIConfig::new()
        .with_base_url(server.base_url())
        .with_max_retries(0)
        .with_auth(secret());

    let m = OpenAIProvider::with_http_client_and_config(http_client, cfg.clone())
        .create(Some(cfg))
        .expect("provider builds")
        .get_model(ModelId::Name("gpt-4o".into(), None))
        .expect("model resolves");

    assert_eq!(m.spec().name, "gpt-4o");

    let d = m.descriptor().expect("descriptor available");
    assert_eq!(
        d.provider,
        ModelProviders::OPENAI,
        "each provider must report ITSELF, not a copy-pasted neighbour"
    );

    assert_eq!(m.costing().expect("costing").total_tokens, 0.0);

    assert!(
        m.tool_formatter().tool_calling_instructions().is_none(),
        "OpenAI has native function calling and needs no prompt instructions"
    );
}

// ---------------------------------------------------------------------------
// OpenAI Responses
// ---------------------------------------------------------------------------

#[valtron_test]
fn responses_model_trait_surface() {
    let server = catalog_server();
    let http_client: DynNetClient = HttpClientBuilder::new().build();
    let cfg = ResponsesConfig::new()
        .with_base_url(server.base_url())
        .with_max_retries(0)
        .with_auth(secret());

    let m = ResponsesProvider::with_http_client_and_config(http_client, cfg.clone())
        .create(Some(cfg))
        .expect("provider builds")
        .get_model(ModelId::Name("gpt-4o".into(), None))
        .expect("model resolves");

    assert_eq!(m.spec().name, "gpt-4o");
    let _ = m.tool_formatter();

    // Previously ResponsesModel returned no descriptor and a fixed empty
    // costing report — callers got no provider identity at all and could never
    // read a running token total. Now at parity with OpenAIModel/AnthropicModel.
    let d = m
        .descriptor()
        .expect("Responses must expose a descriptor like its sibling providers");
    assert_eq!(d.id, "openai-responses");
    assert_eq!(d.provider, ModelProviders::OPENAIRESPONSES);
    assert!(
        d.reasoning,
        "the Responses API serves reasoning models (o1/o3)"
    );

    // A fresh model starts clean; the accumulator sums per-call usage, which
    // parse_response already extracts from the vendor response.
    assert_eq!(
        m.costing().expect("costing").total_tokens,
        0.0,
        "a fresh model instance must not inherit a previous one's total"
    );
}

// ---------------------------------------------------------------------------
// Cross-provider
// ---------------------------------------------------------------------------

#[valtron_test]
fn each_provider_reports_a_distinct_identity() {
    // The failure this guards: a descriptor copy-pasted between providers, so
    // two different backends claim the same id and pricing.
    let server = catalog_server();
    let http_client: DynNetClient = HttpClientBuilder::new().build();
    let cfg = OpenAIConfig::new()
        .with_base_url(server.base_url())
        .with_max_retries(0)
        .with_auth(secret());
    let openai = OpenAIProvider::with_http_client_and_config(http_client, cfg.clone())
        .create(Some(cfg))
        .expect("builds")
        .get_model(ModelId::Name("gpt-4o".into(), None))
        .expect("resolves");

    let anthropic_id = anthropic_model().descriptor().expect("descriptor").id;
    let openai_id = openai.descriptor().expect("descriptor").id;

    assert_ne!(
        anthropic_id, openai_id,
        "two providers must not share a descriptor id"
    );
}
