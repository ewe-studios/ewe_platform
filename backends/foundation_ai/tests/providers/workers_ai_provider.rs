//! Integration tests for `WorkersAiProvider` against a real HTTP server,
//! verifying the Workers AI REST API (`ai/run`) — both the non-streaming
//! (`generate`) and SSE streaming (`stream`) paths.
//!
//! - `TestHttpServer` (request-response) — verifies non-streaming response parsing
//! - `SseTestServer` (streaming) — verifies SSE parsing with a clean EOF close.
//!   Workers AI streams carry no `[DONE]` sentinel; the stream ends when the
//!   server closes the connection, so the test must close it cleanly.

use std::net::SocketAddr;
use std::sync::Arc;

use foundation_ai::backends::workers_ai::{WorkersAiConfig, WorkersAiProvider};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelProvider,
    StopReason, TextContent, ToolDeclarations, UserModelContent,
};

use foundation_auth::{AuthCredential, ConfidentialText};
use foundation_core::valtron::{valtron_test, Stream};
use foundation_netio::{DynNetClient, HttpClientBuilder};
use foundation_netio::shared::client::http_client::HttpClient;
use foundation_testing::http::HttpRequest;
use foundation_testing::http::{
    HttpResponse, SseConnectionResult, SseStreamWriter, SseTestServer, TestHttpServer,
};

fn json_response(body: &[u8]) -> HttpResponse {
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

fn make_interaction(prompt: &str) -> ModelInteraction {
    ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: prompt.to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolDeclarations::default(),
        chat_template: None,
        tool_choice: None,
    }
}

fn setup_provider_and_model(base_url: &str) -> impl Model + use<'_> {
    let addr: SocketAddr = base_url
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .unwrap();
    let http_client: DynNetClient = HttpClientBuilder::new().build();
    let config = WorkersAiConfig::new()
        .with_account_id("test-account")
        .with_base_url(base_url)
        .with_auth(AuthCredential::SecretOnly(ConfidentialText::new(
            "test-token".to_string(),
        )));

    let provider = WorkersAiProvider::with_http_client_and_config(http_client, config)
        .create(Some(
            WorkersAiConfig::new()
                .with_account_id("test-account")
                .with_base_url(base_url),
        ))
        .unwrap();

    provider
        .get_model(ModelId::Name(
            "@cf/meta/llama-3.1-8b-instruct".into(),
            None,
        ))
        .unwrap()
}

/// Verify non-streaming generation parses `{"result":{"response":"..."}}`.
#[valtron_test]
fn test_workers_ai_generate() {
    let chat_response = br#"{"result":{"response":"Hello from Workers AI!"}}"#;

    let server = TestHttpServer::with_response(move |_req| json_response(chat_response));
    let model = setup_provider_and_model(server.base_url());

    let result = model.generate(make_interaction("Hi"), None).unwrap();
    assert_eq!(result.len(), 1);

    if let Messages::Assistant {
        content,
        stop_reason,
        ..
    } = &result[0]
    {
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "Hello from Workers AI!");
        } else {
            panic!("Expected Text output");
        }
        assert_eq!(*stop_reason, StopReason::Stop);
    } else {
        panic!("Expected Assistant message");
    }
}

/// Verify SSE streaming yields the first token before the stream closes, then
/// the full accumulated text.
#[valtron_test]
fn test_workers_ai_stream() {
    let server = SseTestServer::with_handler(
        move |_req: &HttpRequest, writer: &mut SseStreamWriter, _conn: usize| {
            let _ = writer.event(r#"{"response":"Hello"}"#);
            let _ = writer.event(r#"{"response":"Hello world"}"#);
            SseConnectionResult::clean_close()
        },
    );
    let model = setup_provider_and_model(server.base_url());

    let mut stream = model.stream(make_interaction("Hi"), None).unwrap();

    let mut messages: Vec<Messages> = Vec::new();
    for item in &mut stream {
        if let Stream::Next(msg) = item {
            messages.push(msg);
        }
    }

    assert!(
        messages.len() >= 2,
        "Expected at least 2 streamed messages (first token + full text), got {}",
        messages.len()
    );

    // The first streamed message carries the first token, proving streaming is
    // not buffered.
    if let Messages::Assistant { content, .. } = &messages[0] {
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "Hello");
        } else {
            panic!("Expected Text output in first message");
        }
    } else {
        panic!("Expected Assistant message");
    }

    // The last message carries the full accumulated text.
    if let Messages::Assistant {
        content,
        stop_reason,
        ..
    } = messages.last().unwrap()
    {
        if let ModelOutput::Text(tc) = content {
            assert_eq!(tc.content, "Hello world");
        } else {
            panic!("Expected Text output in last message");
        }
        assert_eq!(*stop_reason, StopReason::Stop);
    } else {
        panic!("Expected Assistant message");
    }
}
