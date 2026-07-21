//! HTTP fault injection against the provider request path.
//!
//! WHY: the provider error branches — non-2xx handling, vendor error-body
//! parsing, `Retry-After`, the retry loop, malformed/truncated payloads — are
//! the least-covered code in the crate, and they are exactly the code that runs
//! when a vendor is having a bad day. The existing provider suites only serve
//! *happy* responses, so every one of these paths was reached only by unit tests
//! of the individual helpers, never end-to-end through `generate()`.
//!
//! WHAT: drives a real `OpenAIProvider` / `ResponsesProvider` against a
//! `TestHttpServer` that returns the failure the test names — 401/429/500,
//! vendor error JSON, non-JSON bodies, empty bodies, wrong-shaped JSON — and
//! asserts the provider surfaces an error rather than panicking or silently
//! producing an empty success.
//!
//! HOW: `max_retries(0)` on the retryable cases so the exponential backoff does
//! not make the suite slow; a separate test covers that a retryable status is
//! actually retried. No vendor credentials: the server is ours.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelProvider, TextContent, ToolShed,
    UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::valtron_test;
use foundation_netio::http::NativeHttpClient;
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_netio::shared::client::StaticSocketAddr;
use foundation_testing::http::{HttpResponse, TestHttpServer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn server_addr(server: &TestHttpServer) -> SocketAddr {
    server
        .base_url()
        .strip_prefix("http://")
        .expect("test server base_url is http")
        .parse()
        .expect("valid socket addr")
}

/// A canned response with an explicit status and body.
fn response(status: u16, status_text: &str, content_type: &str, body: &[u8]) -> HttpResponse {
    HttpResponse {
        status,
        status_text: status_text.to_string(),
        headers: vec![
            ("Content-Type".to_string(), content_type.to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
            ("Connection".to_string(), "close".to_string()),
        ],
        body: body.to_vec(),
    }
}

fn json_error(status: u16, status_text: &str, body: &[u8]) -> HttpResponse {
    response(status, status_text, "application/json", body)
}

/// Build a model against `server`, with `max_retries` retries.
///
/// Retries default to 3 with exponential backoff; the failure tests pass 0 so a
/// non-recoverable status fails immediately instead of sleeping.
fn model_for(server: &TestHttpServer, max_retries: u32) -> impl Model + use<'_> {
    let resolver = StaticSocketAddr::new(server_addr(server));
    let http_client: Arc<dyn HttpClient> = Arc::new(NativeHttpClient::new(resolver));
    let config = OpenAIConfig::new()
        .with_base_url(server.base_url())
        .with_max_retries(max_retries)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-key".to_string(),
        )));

    let provider = OpenAIProvider::with_http_client_and_config(http_client, config)
        .create(Some(
            OpenAIConfig::new()
                .with_base_url(server.base_url())
                .with_max_retries(max_retries),
        ))
        .expect("provider builds");

    provider
        .get_model(ModelId::Name("gpt-4".into(), None))
        .expect("model resolves")
}

fn interaction() -> ModelInteraction {
    ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "hello".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

/// Run `generate` against a server that always returns `resp`; expect failure.
fn expect_generate_error(resp: HttpResponse, max_retries: u32) -> String {
    let server = TestHttpServer::with_response(move |_req| resp.clone());
    let model = model_for(&server, max_retries);
    match model.generate(interaction(), None) {
        Ok(out) => panic!("expected an error, got a successful generate: {out:?}"),
        Err(e) => e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Non-2xx statuses
// ---------------------------------------------------------------------------

#[valtron_test]
fn unauthorized_surfaces_an_error() {
    let body = br#"{"error":{"message":"Invalid API key","type":"invalid_request_error"}}"#;
    let msg = expect_generate_error(json_error(401, "Unauthorized", body), 0);
    assert!(
        msg.contains("Invalid API key") || msg.contains("401"),
        "a 401 must surface the vendor detail or the status: {msg}"
    );
}

#[valtron_test]
fn forbidden_surfaces_an_error() {
    let body = br#"{"error":{"message":"You do not have access","type":"permission_error"}}"#;
    let msg = expect_generate_error(json_error(403, "Forbidden", body), 0);
    assert!(!msg.is_empty(), "a 403 must produce a non-empty error");
}

#[valtron_test]
fn server_error_surfaces_an_error() {
    let body = br#"{"error":{"message":"upstream exploded","type":"server_error"}}"#;
    let msg = expect_generate_error(json_error(500, "Internal Server Error", body), 0);
    assert!(
        msg.contains("upstream exploded") || msg.contains("500"),
        "a 500 must surface the vendor detail or the status: {msg}"
    );
}

#[valtron_test]
fn rate_limit_surfaces_an_error_when_retries_are_exhausted() {
    let body = br#"{"error":{"message":"Rate limit reached","type":"rate_limit_error"}}"#;
    let msg = expect_generate_error(json_error(429, "Too Many Requests", body), 0);
    assert!(
        msg.contains("Rate limit") || msg.contains("429"),
        "an exhausted 429 must surface as an error: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Malformed / unexpected bodies
// ---------------------------------------------------------------------------

#[valtron_test]
fn non_json_error_body_still_produces_an_error() {
    // Proxies and gateways return HTML error pages. The vendor-error parser must
    // fall back to the raw text rather than failing to parse and losing the
    // status entirely.
    let body = b"<html><body>502 Bad Gateway</body></html>";
    let msg = expect_generate_error(response(502, "Bad Gateway", "text/html", body), 0);
    assert!(!msg.is_empty(), "a non-JSON error body must still error");
}

#[valtron_test]
fn malformed_json_on_a_200_is_a_parse_error() {
    // A 200 whose body is not valid JSON must NOT be reported as success.
    let body = b"{not valid json at all";
    let msg = expect_generate_error(response(200, "OK", "application/json", body), 0);
    assert!(
        msg.to_lowercase().contains("parse"),
        "a malformed 200 body must surface as a parse error: {msg}"
    );
}

#[valtron_test]
fn empty_body_on_a_200_is_an_error() {
    let msg = expect_generate_error(response(200, "OK", "application/json", b""), 0);
    assert!(
        !msg.is_empty(),
        "an empty 200 body must not be treated as a valid completion"
    );
}

#[valtron_test]
fn wrong_shaped_json_on_a_200_is_an_error() {
    // Valid JSON, wrong schema — deserialization into the response type must
    // fail loudly rather than yielding an empty completion.
    let body = br#"{"unexpected":"shape","choices":"not-an-array"}"#;
    let msg = expect_generate_error(response(200, "OK", "application/json", body), 0);
    assert!(
        !msg.is_empty(),
        "a wrong-shaped 200 body must not be treated as a valid completion"
    );
}

// ---------------------------------------------------------------------------
// Retry behaviour
// ---------------------------------------------------------------------------

#[valtron_test]
fn a_retryable_status_is_actually_retried() {
    // The retry loop is the reason a transient 503 does not surface to the user.
    // Count requests: with max_retries(2) a permanently-failing retryable status
    // must be attempted more than once.
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);

    let server = TestHttpServer::with_response(move |_req| {
        counter.fetch_add(1, Ordering::SeqCst);
        json_error(
            503,
            "Service Unavailable",
            br#"{"error":{"message":"try later","type":"server_error"}}"#,
        )
    });

    let model = model_for(&server, 2);
    // Exclude `get_model`'s catalog lookup — count only `generate`'s attempts.
    hits.store(0, Ordering::SeqCst);

    let result = model.generate(interaction(), None);
    assert!(result.is_err(), "a permanently-failing 503 must end in Err");

    // max_retries(2) => the initial attempt plus 2 retries.
    let attempts = hits.load(Ordering::SeqCst);
    assert_eq!(
        attempts, 3,
        "max_retries(2) must give 1 initial attempt + 2 retries, saw {attempts}"
    );
}

#[valtron_test]
fn a_non_retryable_status_is_not_retried() {
    // A 400 is the caller's fault — retrying wastes time and money.
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);

    let server = TestHttpServer::with_response(move |_req| {
        counter.fetch_add(1, Ordering::SeqCst);
        json_error(
            400,
            "Bad Request",
            br#"{"error":{"message":"bad param","type":"invalid_request_error"}}"#,
        )
    });

    let model = model_for(&server, 3);
    // `get_model` performs its own catalog lookup against the same server, so
    // zero the counter here — this test is about what `generate` does.
    hits.store(0, Ordering::SeqCst);

    let result = model.generate(interaction(), None);
    assert!(result.is_err(), "a 400 must end in Err");

    let attempts = hits.load(Ordering::SeqCst);
    assert_eq!(
        attempts, 1,
        "a non-retryable status must be attempted exactly once, saw {attempts}"
    );
}

#[valtron_test]
fn a_retry_eventually_succeeds() {
    // The payoff path: first attempt fails with a retryable status, the second
    // succeeds, and the caller sees a normal result with no error.
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);

    let ok_body = br#"{
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "created": 1,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "recovered"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    }"#;

    let server = TestHttpServer::with_response(move |_req| {
        let n = counter.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            json_error(
                503,
                "Service Unavailable",
                br#"{"error":{"message":"warming up","type":"server_error"}}"#,
            )
        } else {
            response(200, "OK", "application/json", ok_body)
        }
    });

    let model = model_for(&server, 3);
    // Exclude `get_model`'s catalog lookup so the first `generate` request is
    // the one the closure answers with a 503.
    hits.store(0, Ordering::SeqCst);

    let out = model
        .generate(interaction(), None)
        .expect("a transient 503 followed by a 200 must succeed");

    assert!(
        !out.is_empty(),
        "the recovered attempt must yield the assistant message"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "the success must have come from exactly one retry after the 503"
    );
}

// ---------------------------------------------------------------------------
// Anthropic Messages — the same faults through a different provider
// ---------------------------------------------------------------------------

fn anthropic_model_for(server: &TestHttpServer, max_retries: u32) -> impl Model + use<'_> {
    let resolver = StaticSocketAddr::new(server_addr(server));
    let http_client: Arc<dyn HttpClient> = Arc::new(NativeHttpClient::new(resolver));
    let config = AnthropicConfig::new()
        .with_base_url(server.base_url())
        .with_max_retries(max_retries)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-key".to_string(),
        )));

    let provider = AnthropicMessagesProvider::with_http_client_and_config(http_client, config.clone())
        .create(Some(config))
        .expect("provider builds");

    provider
        .get_model(ModelId::Name("claude-3-5-sonnet".into(), None))
        .expect("model resolves")
}

fn expect_anthropic_error(resp: HttpResponse, max_retries: u32) -> String {
    let server = TestHttpServer::with_response(move |_req| resp.clone());
    let model = anthropic_model_for(&server, max_retries);
    match model.generate(interaction(), None) {
        Ok(out) => panic!("expected an error, got a successful generate: {out:?}"),
        Err(e) => e.to_string(),
    }
}

#[valtron_test]
fn anthropic_unauthorized_surfaces_an_error() {
    // Anthropic's error envelope differs from OpenAI's, so its parser needs its
    // own coverage rather than inheriting the OpenAI test.
    let body = br#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
    let msg = expect_anthropic_error(json_error(401, "Unauthorized", body), 0);
    assert!(
        msg.contains("invalid x-api-key") || msg.contains("401"),
        "a 401 must surface the vendor detail or the status: {msg}"
    );
}

#[valtron_test]
fn anthropic_overloaded_surfaces_an_error() {
    let body = br#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
    let msg = expect_anthropic_error(json_error(529, "Overloaded", body), 0);
    assert!(!msg.is_empty(), "a 529 must produce a non-empty error");
}

#[valtron_test]
fn anthropic_malformed_json_on_a_200_is_an_error() {
    let msg = expect_anthropic_error(
        response(200, "OK", "application/json", b"{broken"),
        0,
    );
    assert!(
        !msg.is_empty(),
        "a malformed 200 body must not be treated as a valid completion"
    );
}

#[valtron_test]
fn anthropic_empty_body_on_a_200_is_an_error() {
    let msg = expect_anthropic_error(response(200, "OK", "application/json", b""), 0);
    assert!(!msg.is_empty(), "an empty 200 body must error");
}

#[valtron_test]
fn anthropic_non_retryable_status_is_not_retried() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);

    let server = TestHttpServer::with_response(move |_req| {
        counter.fetch_add(1, Ordering::SeqCst);
        json_error(
            400,
            "Bad Request",
            br#"{"type":"error","error":{"type":"invalid_request_error","message":"bad"}}"#,
        )
    });

    let model = anthropic_model_for(&server, 3);
    hits.store(0, Ordering::SeqCst);

    assert!(model.generate(interaction(), None).is_err());
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "a 400 must not be retried"
    );
}

// ---------------------------------------------------------------------------
// OpenAI Responses API — /v1/responses has its own request+parse path
// ---------------------------------------------------------------------------

fn responses_model_for(server: &TestHttpServer, max_retries: u32) -> impl Model + use<'_> {
    let resolver = StaticSocketAddr::new(server_addr(server));
    let http_client: Arc<dyn HttpClient> = Arc::new(NativeHttpClient::new(resolver));
    let config = ResponsesConfig::new()
        .with_base_url(server.base_url())
        .with_max_retries(max_retries)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-key".to_string(),
        )));

    let provider = ResponsesProvider::with_http_client_and_config(http_client, config.clone())
        .create(Some(config))
        .expect("provider builds");

    provider
        .get_model(ModelId::Name("o3-mini".into(), None))
        .expect("model resolves")
}

fn expect_responses_error(resp: HttpResponse, max_retries: u32) -> String {
    let server = TestHttpServer::with_response(move |_req| resp.clone());
    let model = responses_model_for(&server, max_retries);
    match model.generate(interaction(), None) {
        Ok(out) => panic!("expected an error, got a successful generate: {out:?}"),
        Err(e) => e.to_string(),
    }
}

#[valtron_test]
fn responses_unauthorized_surfaces_an_error() {
    let body = br#"{"error":{"message":"Invalid API key","type":"invalid_request_error"}}"#;
    let msg = expect_responses_error(json_error(401, "Unauthorized", body), 0);
    assert!(
        msg.contains("Invalid API key") || msg.contains("401"),
        "a 401 must surface the vendor detail or the status: {msg}"
    );
}

#[valtron_test]
fn responses_server_error_surfaces_an_error() {
    let body = br#"{"error":{"message":"boom","type":"server_error"}}"#;
    let msg = expect_responses_error(json_error(500, "Internal Server Error", body), 0);
    assert!(!msg.is_empty(), "a 500 must produce a non-empty error");
}

#[valtron_test]
fn responses_malformed_json_on_a_200_is_an_error() {
    let msg = expect_responses_error(response(200, "OK", "application/json", b"{nope"), 0);
    assert!(
        !msg.is_empty(),
        "a malformed 200 body must not be treated as a valid completion"
    );
}

#[valtron_test]
fn responses_retryable_status_is_retried_the_configured_number_of_times() {
    // /v1/responses carries its own retry loop, so OpenAI's coverage does not
    // apply to it.
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let server = TestHttpServer::with_response(move |_req| {
        counter.fetch_add(1, Ordering::SeqCst);
        json_error(
            503,
            "Service Unavailable",
            br#"{"error":{"message":"try later","type":"server_error"}}"#,
        )
    });

    let model = responses_model_for(&server, 2);
    // Exclude get_model's own catalog lookup.
    hits.store(0, Ordering::SeqCst);

    assert!(model.generate(interaction(), None).is_err());
    assert_eq!(
        hits.load(Ordering::SeqCst),
        3,
        "max_retries(2) must give 1 initial attempt + 2 retries"
    );
}

#[valtron_test]
fn responses_non_retryable_status_is_not_retried() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let server = TestHttpServer::with_response(move |_req| {
        counter.fetch_add(1, Ordering::SeqCst);
        json_error(
            400,
            "Bad Request",
            br#"{"error":{"message":"bad param","type":"invalid_request_error"}}"#,
        )
    });

    let model = responses_model_for(&server, 3);
    hits.store(0, Ordering::SeqCst);

    assert!(model.generate(interaction(), None).is_err());
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "a 400 is the caller's fault — retrying wastes time and money"
    );
}
