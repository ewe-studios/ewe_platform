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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};
use foundation_ai::types::ModelParams;
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelProvider, TextContent, ToolShed,
    UserModelContent,
};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::valtron_test;
use foundation_netio::{DynNetClient, HttpClientBuilder};
use foundation_testing::http::{HttpResponse, TestHttpServer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
    let http_client: DynNetClient = HttpClientBuilder::new().build();
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
    let http_client: DynNetClient = HttpClientBuilder::new().build();
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
    let http_client: DynNetClient = HttpClientBuilder::new().build();
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

// ---------------------------------------------------------------------------
// The /v1/embeddings path
// ---------------------------------------------------------------------------
//
// `generate()` routes to `/v1/embeddings` instead of `/v1/chat/completions`
// when the interaction carries an `Assistant` message whose content is
// `ModelOutput::Embedding` — a marker, not real content. That whole branch had
// no coverage: a caller asking for embeddings would otherwise be silently sent
// to the chat endpoint and get prose back instead of a vector.

const EMBEDDING_BODY: &[u8] = br#"{
    "object": "list",
    "model": "text-embedding-3-small",
    "data": [{"index": 0, "object": "embedding", "embedding": [0.1, 0.2, 0.3]}],
    "usage": {"prompt_tokens": 4, "completion_tokens": 0, "total_tokens": 4}
}"#;

/// An interaction carrying the embedding marker plus the text to embed.
fn embedding_interaction(texts: &[&str]) -> ModelInteraction {
    use foundation_ai::types::{ModelOutput, ModelProviders, StopReason, UsageCosting, UsageReport};

    let mut messages: Vec<Messages> = texts
        .iter()
        .map(|t| Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: (*t).into(),
                signature: None,
            }),
            signature: None,
        })
        .collect();

    // The marker that switches generate() onto the embeddings endpoint.
    messages.push(Messages::Assistant {
        id: foundation_compact::ids::new_scru128(),
        model: ModelId::Name("text-embedding-3-small".into(), None),
        timestamp: foundation_compact::SystemTime::UNIX_EPOCH,
        usage: UsageReport {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            cost: UsageCosting::zero(foundation_ai::types::CostStatus::Estimated),
        },
        content: ModelOutput::Embedding {
            dimensions: 0,
            values: Vec::new(),
        },
        stop_reason: StopReason::Stop,
        provider: ModelProviders::OPENAI,
        error_detail: None,
        signature: None,
        metadata: None,
    });

    ModelInteraction {
        system_prompt: None,
        soul: None,
        messages,
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

#[valtron_test]
fn an_embedding_marker_routes_to_the_embeddings_endpoint() {
    // Capture the path so we can prove it went to /embeddings, not
    // /chat/completions — sending an embedding request to chat would return
    // prose where the caller expects a vector.
    let seen = Arc::new(std::sync::Mutex::new(String::new()));
    let recorder = Arc::clone(&seen);

    let server = TestHttpServer::with_response(move |req| {
        *recorder.lock().unwrap() = format!("{:?}", req.path.url);
        response(200, "OK", "application/json", EMBEDDING_BODY)
    });

    let model = model_for(&server, 0);
    let out = model
        .generate(embedding_interaction(&["embed me"]), None)
        .expect("an embedding request must succeed");

    let path = seen.lock().unwrap().clone();
    assert!(
        path.contains("embeddings"),
        "the request must go to the embeddings endpoint, got: {path}"
    );
    assert!(!out.is_empty(), "an embedding response must yield a message");
}

#[valtron_test]
fn an_embedding_response_is_returned_as_a_vector() {
    let server = TestHttpServer::with_response(|_req| {
        response(200, "OK", "application/json", EMBEDDING_BODY)
    });
    let model = model_for(&server, 0);

    let out = model
        .generate(embedding_interaction(&["embed me"]), None)
        .expect("embedding request succeeds");

    use foundation_ai::types::ModelOutput;
    let has_embedding = out.iter().any(|m| {
        matches!(
            m,
            Messages::Assistant {
                content: ModelOutput::Embedding { .. },
                ..
            }
        )
    });
    assert!(
        has_embedding,
        "the result must be an Embedding output, not text: {out:?}"
    );
}

#[valtron_test]
fn an_embedding_response_with_no_data_is_an_error() {
    // An empty `data` array must not be reported as a successful embedding —
    // the caller would get no vector and no explanation.
    let empty = br#"{"object":"list","model":"m","data":[],"usage":null}"#;
    let server = TestHttpServer::with_response(move |_req| {
        response(200, "OK", "application/json", empty)
    });
    let model = model_for(&server, 0);

    let err = model
        .generate(embedding_interaction(&["x"]), None)
        .expect_err("an empty data array must error");
    assert!(
        err.to_string().to_lowercase().contains("embedding"),
        "the error must name what was missing: {err}"
    );
}

// ---------------------------------------------------------------------------
// Responses build_request: instructions + parameter gating
// ---------------------------------------------------------------------------
//
// `build_request` folds system_prompt and soul into one `instructions` field
// and gates each optional parameter on a sentinel. Both matter: dropping `soul`
// loses the agent's persona, and sending `temperature: 0` when the caller meant
// "unset" changes model behaviour rather than leaving the vendor default.

/// One request as the test server saw it.
#[derive(Clone)]
struct SeenRequest {
    method: String,
    path: String,
    body: String,
}

/// Run one generate() against a recording server and return the request body.
///
/// Every request is recorded, not just the last one: `get_model` performs a
/// catalog read against `/models/{id}` before the generation POST, so a
/// single-slot recorder both loses the payload under test and — when the
/// generation request never arrives — reports nothing but "the capture was
/// empty", which is the one fact that does not help. Keeping the whole
/// transcript means a failure prints exactly what the server did receive, and
/// the generate() result is printed alongside it so a transport error is not
/// mistaken for a request that was never sent.
///
/// `blocking_read` is what makes the capture trustworthy. `TestHttpServer`
/// defaults to a *non-blocking* read: if the request body has not landed in the
/// socket by the time the server reads, it takes the `WouldBlock` and hands the
/// handler a request with an **empty body** — no error, no retry. That is the
/// mechanism behind this helper's long-standing intermittent failure, caught in
/// the act by the transcript above:
///
/// ```text
/// generate() succeeded
/// requests seen:
///   POST /v1/models/o3-mini body=""
///   POST /v1/responses      body=""
/// ```
///
/// Both requests arrived and the provider was satisfied, yet the payload the
/// test exists to inspect was gone. A blocking read with a timeout makes the
/// server wait for the body it was sent.
fn responses_request_body(interaction: ModelInteraction, params: Option<ModelParams>) -> String {
    use foundation_netio::shared::http::SendSafeBody;

    let seen: Arc<std::sync::Mutex<Vec<SeenRequest>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorder = Arc::clone(&seen);

    let server = TestHttpServer::with_response(move |req| {
        // The client may send either representation; read both so the capture
        // cannot silently come back empty (which would make every "field is
        // absent" assertion below pass vacuously).
        let body = match &req.body {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
            _ => String::new(),
        };
        recorder.lock().unwrap().push(SeenRequest {
            method: format!("{:?}", req.method),
            path: req.path.url.clone(),
            body,
        });
        response(
            200,
            "OK",
            "application/json",
            br#"{"id":"r1","object":"response","created_at":1,"model":"gpt-4o",
                 "status":"completed","output":[],
                 "usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}"#,
        )
    })
    .blocking_read(Some(std::time::Duration::from_millis(500)));

    let model = responses_model_for(&server, 0);
    let outcome = model.generate(interaction, params);

    let requests = seen.lock().unwrap().clone();
    let transcript = if requests.is_empty() {
        "<the server received no requests at all>".to_string()
    } else {
        requests
            .iter()
            .map(|r| format!("  {} {} body={:?}", r.method, r.path, r.body))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let outcome = match &outcome {
        Ok(_) => "generate() succeeded".to_string(),
        Err(e) => format!("generate() failed: {e}"),
    };

    // The generation request is the one carrying a payload; the catalog read is
    // bodyless. Match on that rather than on position, so an extra preflight
    // request cannot silently shift which body is inspected.
    requests
        .into_iter()
        .find(|r| !r.body.is_empty())
        .map(|r| r.body)
        .unwrap_or_else(|| {
            panic!(
                "no request with a body reached the server — every \"field is absent\" \
                 assertion would have passed for the wrong reason.\n{outcome}\nrequests seen:\n{transcript}"
            )
        })
}

fn interaction_with(system: Option<&str>, soul: Option<&str>) -> ModelInteraction {
    ModelInteraction {
        system_prompt: system.map(str::to_string),
        soul: soul.map(str::to_string),
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "hi".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

#[valtron_test]
fn system_prompt_and_soul_are_combined() {
    let body = responses_request_body(
        interaction_with(Some("SYSTEM_MARKER"), Some("SOUL_MARKER")),
        None,
    );
    assert!(body.contains("SYSTEM_MARKER"), "system prompt must be sent: {body}");
    assert!(
        body.contains("SOUL_MARKER"),
        "soul must not be dropped — it carries the agent's persona: {body}"
    );
}

#[valtron_test]
fn a_soul_alone_still_becomes_instructions() {
    let body = responses_request_body(interaction_with(None, Some("SOUL_ONLY")), None);
    assert!(
        body.contains("SOUL_ONLY"),
        "a soul with no system prompt must still be sent: {body}"
    );
}

#[valtron_test]
fn a_system_prompt_alone_becomes_instructions() {
    let body = responses_request_body(interaction_with(Some("SYS_ONLY"), None), None);
    assert!(body.contains("SYS_ONLY"), "{body}");
}

#[valtron_test]
fn neither_prompt_nor_soul_omits_instructions() {
    // The field is skipped when None; sending `"instructions":null` would be a
    // different request than omitting it.
    let body = responses_request_body(interaction_with(None, None), None);
    assert!(
        !body.contains("\"instructions\":null"),
        "an absent instruction must be omitted, not sent as null: {body}"
    );
}

#[valtron_test]
fn a_zero_temperature_is_omitted_rather_than_sent() {
    // `0.0` is the struct default meaning "unset". Sending it would pin the
    // model to greedy decoding instead of using the vendor default.
    let params = ModelParams {
        temperature: 0.0,
        max_tokens: 0,
        top_p: 0.0,
        ..Default::default()
    };
    let body = responses_request_body(interaction_with(Some("s"), None), Some(params));
    assert!(
        !body.contains("temperature"),
        "an unset temperature must not reach the vendor: {body}"
    );
    assert!(
        !body.contains("max_output_tokens"),
        "an unset max_tokens must not reach the vendor: {body}"
    );
}

#[valtron_test]
fn explicit_parameters_are_sent() {
    let params = ModelParams {
        temperature: 0.7,
        max_tokens: 256,
        top_p: 0.9,
        ..Default::default()
    };
    let body = responses_request_body(interaction_with(Some("s"), None), Some(params));
    assert!(body.contains("temperature"), "{body}");
    assert!(body.contains("max_output_tokens"), "{body}");
    assert!(body.contains("top_p"), "{body}");
}

#[valtron_test]
fn a_top_p_of_one_is_omitted_as_a_no_op() {
    // top_p = 1.0 selects the whole distribution, i.e. no nucleus sampling.
    // The gate is `> 0.0 && < 1.0`, so 1.0 is treated as unset.
    let params = ModelParams {
        top_p: 1.0,
        temperature: 0.5,
        max_tokens: 16,
        ..Default::default()
    };
    let body = responses_request_body(interaction_with(Some("s"), None), Some(params));
    assert!(
        !body.contains("top_p"),
        "top_p = 1.0 is a no-op and must be omitted: {body}"
    );
}
