//! The agent-rpc browser client, exported to JavaScript (spec 83 feature 05,
//! T22).
//!
//! WHY: a page uses the same Connect surface as a native caller. This crate
//! wraps [`WebClient`] in a `wasm-bindgen` type, so JavaScript calls plain
//! methods and awaits them.
//!
//! WHAT: [`BrowserClient`]: `fromAccessPayload`, `capabilities`,
//! `createInstance`, `destroyInstance` and `chat` (collects one turn's text).
//!
//! HOW: the JavaScript page loads the wasm module, calls
//! `BrowserClient.fromAccessPayload(payload)`, then the async methods. A
//! dropped call drops the transport stream, which cancels the response.

use foundation_agent_rpc::client::web::WebClient;
use foundation_agent_rpc::core::model::ModelRequest;
use foundation_agent_rpc::core::registry::InstanceSpec;
use foundation_agent_rpc::proto::v1 as pb;
use futures::StreamExt;
use wasm_bindgen::prelude::*;

/// The browser client of one host.
#[wasm_bindgen]
pub struct BrowserClient {
    inner: WebClient,
}

#[wasm_bindgen]
impl BrowserClient {
    /// Build a client from an operator-issued access payload.
    ///
    /// # Errors
    /// A JavaScript error that names the bad payload.
    #[wasm_bindgen(js_name = fromAccessPayload)]
    pub fn from_access_payload(payload: &str) -> Result<BrowserClient, JsValue> {
        let inner = WebClient::from_access_payload(payload)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(Self { inner })
    }

    /// The host base URL.
    #[wasm_bindgen(getter)]
    pub fn url(&self) -> String {
        self.inner.url().to_string()
    }

    /// The host capability report, as JSON.
    ///
    /// # Errors
    /// A JavaScript error that names the failed call.
    pub async fn capabilities(&self) -> Result<String, JsValue> {
        let response: pb::GetCapabilitiesResponse = self
            .inner
            .capabilities()
            .await
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let models: Vec<serde_json::Value> = response
            .models
            .iter()
            .map(|model| serde_json::json!({ "name": model.name }))
            .collect();
        Ok(serde_json::json!({
            "models": models,
            "protocols": response.protocols,
            "hostKind": response.host_kind,
        })
        .to_string())
    }

    /// Create one instance on the host.
    ///
    /// # Errors
    /// A JavaScript error that names the failed call.
    pub async fn create_instance(&self, name: &str, model: &str) -> Result<String, JsValue> {
        let spec = InstanceSpec {
            name: name.to_string(),
            model: ModelRequest {
                name: Some(model.to_string()),
                kind: None,
                device: None,
            },
            ..Default::default()
        };
        let info = self
            .inner
            .create_instance(spec)
            .await
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(serde_json::json!({ "name": info.name, "model": info.model }).to_string())
    }

    /// Destroy one instance.
    ///
    /// # Errors
    /// A JavaScript error that names the failed call.
    pub async fn destroy_instance(&self, name: &str) -> Result<(), JsValue> {
        self.inner
            .destroy_instance(name)
            .await
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Run one turn and return the text it produced.
    ///
    /// # Errors
    /// A JavaScript error that carries the failed turn's message.
    pub async fn chat(&self, instance: &str, prompt: &str) -> Result<String, JsValue> {
        let mut stream = self.inner.chat_stream(instance, prompt);
        let mut text = String::new();
        while let Some(item) = stream.next().await {
            let event = item.map_err(|error| JsValue::from_str(&error.to_string()))?;
            match event.event {
                Some(pb::__buffa::oneof::turn_event::Event::Text(chunk)) => text.push_str(&chunk),
                Some(pb::__buffa::oneof::turn_event::Event::Failed(failed)) => {
                    return Err(JsValue::from_str(&failed.message));
                }
                _ => {}
            }
        }
        Ok(text)
    }
}
