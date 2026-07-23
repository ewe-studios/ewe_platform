//! Model-catalog lookup: `get_all` / `get_one` / `get_model_by_spec` + caching.
//!
//! WHY: these are how a `ModelId` becomes a usable model, and they run before
//! every first generation — but the provider suites all jump straight to
//! `generate()` on an already-resolved model, so the catalog fetch, its
//! substring filter, the not-found path and the model cache were uncovered. A
//! filter bug here resolves the *wrong* model silently; a broken cache re-fetches
//! the catalog on every call.
//!
//! WHAT: serves an OpenAI `/models` list from a `TestHttpServer` and asserts the
//! filter, ordering, empty result, `NotFound` mapping, and that a repeat
//! `get_model` is served from cache rather than re-hitting the network.
//!
//! HOW: offline — the catalog is ours. No vendor credentials.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_ai::backends::anthropic_messages_provider::{
    AnthropicConfig, AnthropicMessagesProvider,
};
use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::backends::openai_responses_provider::{ResponsesConfig, ResponsesProvider};
use foundation_ai::types::{ModelId, ModelProvider};
use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::valtron_test;
use foundation_netio::{DynNetClient, HttpClientBuilder};
use foundation_testing::http::{HttpResponse, TestHttpServer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn json_ok(body: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        status_text: "OK".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), body.len().to_string()),
            ("Connection".to_string(), "close".to_string()),
        ],
        body: body.to_vec(),
    }
}

/// A `/models` catalog with three entries spanning two vendors.
const CATALOG: &[u8] = br#"{
    "object": "list",
    "data": [
        {"id": "gpt-4o",      "object": "model", "created": 1, "owned_by": "openai"},
        {"id": "gpt-4o-mini", "object": "model", "created": 2, "owned_by": "openai"},
        {"id": "claude-3",    "object": "model", "created": 3, "owned_by": "anthropic"}
    ]
}"#;

fn provider_for(server: &TestHttpServer) -> OpenAIProvider {
    let http_client: DynNetClient = HttpClientBuilder::new().build();
    let config = OpenAIConfig::new()
        .with_base_url(server.base_url())
        .with_max_retries(0)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-key".to_string(),
        )));

    OpenAIProvider::with_http_client_and_config(http_client, config.clone())
        .create(Some(config))
        .expect("provider builds")
}

fn catalog_server() -> TestHttpServer {
    TestHttpServer::with_response(|_req| json_ok(CATALOG))
}

// ---------------------------------------------------------------------------
// get_all — the substring filter
// ---------------------------------------------------------------------------

#[valtron_test]
fn get_all_returns_every_model_matching_the_pattern() {
    let server = catalog_server();
    let provider = provider_for(&server);

    let specs = provider
        .get_all(ModelId::Name("gpt-4o".into(), None))
        .expect("catalog fetch succeeds");

    // Substring match: both gpt-4o and gpt-4o-mini contain "gpt-4o".
    assert_eq!(specs.len(), 2, "got: {specs:?}");
    assert!(specs.iter().any(|s| s.name == "gpt-4o"));
    assert!(specs.iter().any(|s| s.name == "gpt-4o-mini"));
    assert!(
        !specs.iter().any(|s| s.name == "claude-3"),
        "a non-matching model must be filtered out"
    );
}

#[valtron_test]
fn get_all_filter_is_case_insensitive() {
    // The catalog is lowercase but callers pass mixed case; a case-sensitive
    // filter would report a present model as missing.
    let server = catalog_server();
    let provider = provider_for(&server);

    let specs = provider
        .get_all(ModelId::Name("GPT-4O".into(), None))
        .expect("catalog fetch succeeds");
    assert_eq!(specs.len(), 2, "uppercase must match lowercase ids: {specs:?}");
}

#[valtron_test]
fn get_all_matches_on_every_model_id_variant() {
    // The filter reads the string out of whichever ModelId variant it is given;
    // a variant reading the wrong field would match nothing.
    let server = catalog_server();
    let provider = provider_for(&server);

    for id in [
        ModelId::Name("claude".into(), None),
        ModelId::Alias("claude".into(), None),
        ModelId::Group("claude".into(), None),
        ModelId::Architecture("claude".into(), None),
    ] {
        let specs = provider
            .get_all(id.clone())
            .unwrap_or_else(|e| panic!("{id:?} fetch failed: {e}"));
        assert_eq!(specs.len(), 1, "{id:?} must match claude-3: {specs:?}");
    }
}

#[valtron_test]
fn get_all_with_no_match_is_empty_not_an_error() {
    // "no such model" is an empty list here; `get_one` is what turns that into
    // NotFound. Conflating the two would make an empty catalog look like a
    // transport failure.
    let server = catalog_server();
    let provider = provider_for(&server);

    let specs = provider
        .get_all(ModelId::Name("does-not-exist".into(), None))
        .expect("a non-matching filter is still a successful fetch");
    assert!(specs.is_empty(), "got: {specs:?}");
}

#[valtron_test]
fn get_all_surfaces_a_transport_failure_as_not_found() {
    let server = TestHttpServer::with_response(|_req| HttpResponse {
        status: 500,
        status_text: "Internal Server Error".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), "2".to_string()),
            ("Connection".to_string(), "close".to_string()),
        ],
        body: b"{}".to_vec(),
    });
    let provider = provider_for(&server);

    assert!(
        provider.get_all(ModelId::Name("gpt-4o".into(), None)).is_err(),
        "a failing catalog fetch must be an error, not an empty list"
    );
}

// ---------------------------------------------------------------------------
// get_one — first match, or NotFound
// ---------------------------------------------------------------------------

#[valtron_test]
fn get_one_returns_the_first_match() {
    let server = catalog_server();
    let provider = provider_for(&server);

    let spec = provider
        .get_one(ModelId::Name("gpt-4o".into(), None))
        .expect("a matching model resolves");
    assert!(spec.name.starts_with("gpt-4o"), "got: {spec:?}");
}

#[valtron_test]
fn get_one_with_no_match_is_not_found() {
    let server = catalog_server();
    let provider = provider_for(&server);

    let err = provider
        .get_one(ModelId::Name("nope".into(), None))
        .expect_err("an unmatched model must be NotFound");
    assert!(
        err.to_string().to_lowercase().contains("no model matching")
            || err.to_string().to_lowercase().contains("not found"),
        "the error must say which model was missing: {err}"
    );
}

// ---------------------------------------------------------------------------
// get_model_by_spec
// ---------------------------------------------------------------------------

#[valtron_test]
fn get_model_by_spec_resolves_via_the_specs_id() {
    let server = catalog_server();
    let provider = provider_for(&server);

    let spec = provider
        .get_one(ModelId::Name("gpt-4o-mini".into(), None))
        .expect("spec resolves");
    assert!(
        provider.get_model_by_spec(spec).is_ok(),
        "a spec obtained from the catalog must load a model"
    );
}

// ---------------------------------------------------------------------------
// Model cache
// ---------------------------------------------------------------------------

#[valtron_test]
fn a_repeated_get_model_is_served_from_cache() {
    // Without the cache every model resolution re-fetches the whole catalog,
    // which is a network round trip on the hot path.
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let server = TestHttpServer::with_response(move |_req| {
        counter.fetch_add(1, Ordering::SeqCst);
        json_ok(CATALOG)
    });
    let provider = provider_for(&server);

    let id = ModelId::Name("gpt-4o".into(), None);
    provider.get_model(id.clone()).expect("first resolve");
    let after_first = hits.load(Ordering::SeqCst);

    provider.get_model(id).expect("second resolve");
    let after_second = hits.load(Ordering::SeqCst);

    assert_eq!(
        after_first, after_second,
        "the second get_model must hit the cache, not the network \
         ({after_first} -> {after_second} requests)"
    );
}

#[valtron_test]
fn distinct_models_are_cached_separately() {
    // A cache keyed too coarsely would hand back the first model for every id.
    let server = catalog_server();
    let provider = provider_for(&server);

    let a = provider
        .get_model(ModelId::Name("gpt-4o-mini".into(), None))
        .expect("mini resolves");
    let b = provider
        .get_model(ModelId::Name("claude-3".into(), None))
        .expect("claude resolves");

    use foundation_ai::types::Model;
    assert_ne!(
        a.spec().name,
        b.spec().name,
        "two different ids must not share a cache entry"
    );
}

// ---------------------------------------------------------------------------
// Anthropic — synthesises a spec instead of listing (no /models endpoint)
// ---------------------------------------------------------------------------

fn anthropic_provider() -> AnthropicMessagesProvider {
    let config = AnthropicConfig::new().with_auth(AuthCredential::SecretOnly(
        ConfidentialText::new("test-key".to_string()),
    ));
    AnthropicMessagesProvider::new()
        .create(Some(config))
        .expect("provider builds")
}

#[test]
fn anthropic_get_all_synthesises_a_single_spec_without_network() {
    // Anthropic exposes no model-listing API, so `get_all` echoes the requested
    // id back as one spec. It must therefore never fail for an unknown model —
    // resolution is the caller's problem, not a catalog lookup.
    let provider = anthropic_provider();
    let specs = provider
        .get_all(ModelId::Name("claude-opus-4-8".into(), None))
        .expect("synthesised lookup cannot fail");

    assert_eq!(specs.len(), 1, "exactly one synthesised spec: {specs:?}");
    assert_eq!(specs[0].name, "claude-opus-4-8");
}

#[test]
fn anthropic_get_all_echoes_any_id_including_unknown_ones() {
    let provider = anthropic_provider();
    let specs = provider
        .get_all(ModelId::Name("not-a-real-model".into(), None))
        .expect("synthesised lookup cannot fail");
    assert_eq!(
        specs[0].name, "not-a-real-model",
        "the id is echoed verbatim; there is no catalog to validate against"
    );
}

#[test]
fn anthropic_get_one_resolves_the_synthesised_spec() {
    let provider = anthropic_provider();
    let spec = provider
        .get_one(ModelId::Name("claude-sonnet-4-6".into(), None))
        .expect("get_one resolves");
    assert_eq!(spec.name, "claude-sonnet-4-6");
}

// ---------------------------------------------------------------------------
// Responses API — same catalog path as Chat Completions, separate code
// ---------------------------------------------------------------------------

fn responses_provider_for(server: &TestHttpServer) -> ResponsesProvider {
    let http_client: DynNetClient = HttpClientBuilder::new().build();
    let config = ResponsesConfig::new()
        .with_base_url(server.base_url())
        .with_max_retries(0)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-key".to_string(),
        )));

    ResponsesProvider::with_http_client_and_config(http_client, config.clone())
        .create(Some(config))
        .expect("provider builds")
}

#[valtron_test]
fn responses_get_all_filters_the_catalog() {
    // /v1/responses has its own copy of the catalog path, so it needs its own
    // coverage rather than inheriting the Chat Completions test.
    let server = catalog_server();
    let provider = responses_provider_for(&server);

    let specs = provider
        .get_all(ModelId::Name("gpt-4o".into(), None))
        .expect("catalog fetch succeeds");
    assert_eq!(specs.len(), 2, "got: {specs:?}");
}

#[valtron_test]
fn responses_get_all_with_no_match_is_empty() {
    let server = catalog_server();
    let provider = responses_provider_for(&server);

    let specs = provider
        .get_all(ModelId::Name("absent".into(), None))
        .expect("a non-matching filter is still a successful fetch");
    assert!(specs.is_empty(), "got: {specs:?}");
}

#[valtron_test]
fn responses_get_all_surfaces_a_transport_failure() {
    let server = TestHttpServer::with_response(|_req| HttpResponse {
        status: 503,
        status_text: "Service Unavailable".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Content-Length".to_string(), "2".to_string()),
            ("Connection".to_string(), "close".to_string()),
        ],
        body: b"{}".to_vec(),
    });
    let provider = responses_provider_for(&server);

    assert!(
        provider.get_all(ModelId::Name("gpt-4o".into(), None)).is_err(),
        "a failing catalog fetch must error, not yield an empty list"
    );
}

#[valtron_test]
fn responses_get_one_returns_the_first_match() {
    let server = catalog_server();
    let provider = responses_provider_for(&server);
    let spec = provider
        .get_one(ModelId::Name("gpt-4o".into(), None))
        .expect("a matching model resolves");
    assert!(spec.name.starts_with("gpt-4o"), "got: {spec:?}");
}

#[valtron_test]
fn responses_get_one_with_no_match_is_not_found() {
    let server = catalog_server();
    let provider = responses_provider_for(&server);
    assert!(
        provider.get_one(ModelId::Name("absent".into(), None)).is_err(),
        "an unmatched model must be NotFound, not an empty success"
    );
}

#[valtron_test]
fn responses_get_model_by_spec_resolves_via_the_specs_id() {
    let server = catalog_server();
    let provider = responses_provider_for(&server);
    let spec = provider
        .get_one(ModelId::Name("gpt-4o".into(), None))
        .expect("spec resolves");
    assert!(
        provider.get_model_by_spec(spec).is_ok(),
        "a spec from the catalog must load a model"
    );
}

#[valtron_test]
fn responses_repeated_get_model_is_served_from_cache() {
    // The Responses provider keeps its own models_cache, separate from the Chat
    // Completions one, so it needs its own proof that a repeat resolve does not
    // re-fetch the catalog.
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    let server = TestHttpServer::with_response(move |_req| {
        counter.fetch_add(1, Ordering::SeqCst);
        json_ok(CATALOG)
    });
    let provider = responses_provider_for(&server);

    let id = ModelId::Name("gpt-4o".into(), None);
    provider.get_model(id.clone()).expect("first resolve");
    let after_first = hits.load(Ordering::SeqCst);

    provider.get_model(id).expect("second resolve");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        after_first,
        "the second resolve must hit the cache, not the network"
    );
}
