//! The Worker's client for one instance Durable Object (spec 83 feature 05,
//! section 5.5).
//!
//! WHY: a Worker does not run a turn. It forwards the call to the Durable
//! Object (DO) that holds the instance, over the DO stub.
//!
//! WHAT: [`InstanceStub`] (Connect unary and stream calls over `worker::Stub`)
//! and [`forwarded_principal_header`].
//!
//! HOW: a stub fetch runs as one plain async call, so the Worker's fetch
//! future drives it. The connectrpc client transport uses a valtron exchange
//! task, which only the engine polls; a Worker fetch is outside the engine,
//! so a nested DO call through `H1Transport` never starts (issue #965).
//! [`InstanceStub`] writes the Connect frame itself and reads the response.
//! A unary reply is small and buffered; a stream reads one frame at a time.

use std::collections::VecDeque;
use std::sync::Arc;

use buffa::Message;
use foundation_compact::SendWrapper;
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::client::config::ClientConfig;
use futures::Stream;
use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::core::registry::TurnEventStream;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1 as pb;

/// The Durable Object binding name.
pub const INSTANCE_BINDING: &str = "AGENT_INSTANCES";

/// The base URL of the instance client. The stub ignores the host; the value
/// gives the Connect call a well-formed URL.
pub const INSTANCE_BASE_URL: &str = "https://agent-instance.internal";

/// The header that carries the checked principal to the Durable Object.
pub const FORWARDED_PRINCIPAL_HEADER: &str = "ewe-forwarded-principal";

/// The content type of a unary Connect call.
pub const UNARY_CONTENT_TYPE: &str = "application/proto";
/// The content type of a Connect server-stream call.
pub const STREAM_CONTENT_TYPE: &str = "application/connect+proto";

fn config_error(what: &str, error: impl std::fmt::Debug) -> ErrorTrace<AgentRpcError> {
    ErrorTrace::new(AgentRpcError::Config(format!("{what}: {error:?}")))
}

/// One instance: the stub of its Durable Object.
#[derive(Clone)]
pub struct InstanceStub {
    stub: Arc<worker::Stub>,
    principal: String,
}

impl std::fmt::Debug for InstanceStub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceStub")
            .field("principal", &self.principal)
            .finish_non_exhaustive()
    }
}

impl InstanceStub {
    /// The stub of the Durable Object named `name`.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when the binding or the name is bad.
    pub fn new(env: &worker::Env, name: &str, principal: &str) -> AgentRpcResult<Self> {
        let namespace = env
            .durable_object(INSTANCE_BINDING)
            .map_err(|error| config_error("the instance binding", error))?;
        let id = namespace
            .id_from_name(name)
            .map_err(|error| config_error("the instance name", error))?;
        let stub = id
            .get_stub()
            .map_err(|error| config_error("the instance stub", error))?;
        Ok(Self {
            stub: Arc::new(stub),
            principal: principal.to_string(),
        })
    }

    /// A plain Connect unary call: a raw message body, a raw message reply.
    ///
    /// # Errors
    /// [`AgentRpcError::Transport`] for a failed call or a non-200 status.
    pub async fn unary(&self, procedure: &str, body: Vec<u8>) -> AgentRpcResult<Vec<u8>> {
        let response = self.fetch(procedure, UNARY_CONTENT_TYPE, body).await?;
        let response: web_sys::Response = response.into();
        if response.status() != 200 {
            let message = read_text(&response).await;
            return Err(ErrorTrace::new(AgentRpcError::Transport(format!(
                "the instance answered {}: {message}",
                response.status()
            ))));
        }
        let buffer = SendWrapper::new(JsFuture::from(
            response
                .array_buffer()
                .map_err(|error| config_error("the response body", error))?,
        ))
        .await
        .map_err(|error| config_error("the response body", error))?;
        Ok(Uint8Array::new(&buffer).to_vec())
    }

    /// A Connect server stream: one enveloped request, a stream of typed
    /// `TurnEvent` frames.
    ///
    /// # Errors
    /// [`AgentRpcError::Transport`] for a failed call or a non-200 status.
    pub async fn stream(&self, procedure: &str, body: Vec<u8>) -> AgentRpcResult<TurnEventStream> {
        let response = self.fetch(procedure, STREAM_CONTENT_TYPE, body).await?;
        let response: web_sys::Response = response.into();
        if response.status() != 200 {
            let message = read_text(&response).await;
            return Err(ErrorTrace::new(AgentRpcError::Transport(format!(
                "the instance answered {}: {message}",
                response.status()
            ))));
        }
        let reader = response
            .body()
            .ok_or_else(|| {
                ErrorTrace::new(AgentRpcError::Transport(
                    "the instance sent no stream body".to_string(),
                ))
            })?
            .get_reader()
            .unchecked_into::<web_sys::ReadableStreamDefaultReader>();
        let state = StreamState {
            reader,
            buffer: Vec::new(),
            pending: VecDeque::new(),
            done: false,
        };
        Ok(Box::pin(futures::stream::unfold(
            state,
            |mut state| async move {
                loop {
                    if let Some(event) = state.pending.pop_front() {
                        return Some((event, state));
                    }
                    if state.done {
                        return None;
                    }
                    let item = SendWrapper::new(JsFuture::from(state.reader.read())).await;
                    let Ok(item) = item else {
                        state.done = true;
                        return Some((
                            Err(ErrorTrace::new(AgentRpcError::Transport(
                                "the instance stream failed".to_string(),
                            ))),
                            state,
                        ));
                    };
                    let done =
                        js_sys::Reflect::get(&item, &wasm_bindgen::JsValue::from_str("done"))
                            .ok()
                            .and_then(|value| value.as_bool())
                            .unwrap_or(true);
                    if done {
                        state.done = true;
                        continue;
                    }
                    let value =
                        js_sys::Reflect::get(&item, &wasm_bindgen::JsValue::from_str("value")).ok();
                    if let Some(value) = value {
                        state
                            .buffer
                            .extend_from_slice(&Uint8Array::new(&value).to_vec());
                    }
                    take_frames(&mut state);
                }
            },
        )))
    }

    /// Send one request to the stub.
    async fn fetch(
        &self,
        procedure: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> AgentRpcResult<worker::Response> {
        let url = format!("{INSTANCE_BASE_URL}{procedure}");
        let headers =
            web_sys::Headers::new().map_err(|error| config_error("the request headers", error))?;
        headers
            .set("Content-Type", content_type)
            .map_err(|error| config_error("the content type", error))?;
        headers
            .set(FORWARDED_PRINCIPAL_HEADER, &self.principal)
            .map_err(|error| config_error("the principal header", error))?;
        let init = web_sys::RequestInit::new();
        init.set_method("POST");
        init.set_headers(&headers);
        let bytes = Uint8Array::from(body.as_slice());
        init.set_body(&bytes.into());
        let request = web_sys::Request::new_with_str_and_init(&url, &init)
            .map_err(|error| config_error("the request", error))?;
        SendWrapper::new(self.stub.fetch_with_request(request.into()))
            .await
            .map_err(|error| {
                ErrorTrace::new(AgentRpcError::Transport(format!(
                    "the instance call failed: {error}"
                )))
            })
    }

    /// A generated client over the stub, for a caller that wants one.
    ///
    /// It is unused by the registry: `H1Transport` uses a valtron exchange
    /// task, which a Worker fetch does not drive (issue #965).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when the binding or the name is bad.
    pub fn generated_client(
        env: &worker::Env,
        name: &str,
        principal: &str,
    ) -> AgentRpcResult<crate::proto::v1::agent_service::AgentServiceClient> {
        let config = ClientConfig::default();
        let stub = foundation_netio::cloudflare::StubHttpClient::new_with_config(
            env,
            INSTANCE_BINDING,
            name,
            &config,
        )
        .map_err(|error| config_error("the instance stub", error))?;
        let transport = foundation_connectrpc::H1Transport::new(Arc::new(stub));
        let mut options = foundation_connectrpc::ClientOptions::new();
        let (key, value) = forwarded_principal_header(principal);
        options = options.with_header(key, value);
        crate::proto::v1::agent_service::AgentServiceClient::new(
            Arc::new(transport),
            INSTANCE_BASE_URL,
            options,
        )
        .map_err(|error| {
            ErrorTrace::new(AgentRpcError::Config(format!(
                "the instance client cannot be built: {error}"
            )))
        })
    }
}

/// Read a short error body as text.
async fn read_text(response: &web_sys::Response) -> String {
    let Ok(promise) = response.text() else {
        return String::new();
    };
    match SendWrapper::new(JsFuture::from(promise)).await {
        Ok(value) => value.as_string().unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// The state of one forwarded Connect stream.
struct StreamState {
    reader: web_sys::ReadableStreamDefaultReader,
    buffer: Vec<u8>,
    pending: VecDeque<AgentRpcResult<pb::TurnEvent>>,
    done: bool,
}

/// Take every complete Connect frame from `state.buffer`.
///
/// A frame is one flag byte, a big-endian `u32` length and the message. A
/// flag of `0x02` is the end of the stream. A frame that does not decode is
/// reported once and ends the stream.
fn take_frames(state: &mut StreamState) {
    loop {
        if state.buffer.len() < 5 {
            return;
        }
        let flag = state.buffer[0];
        let length = u32::from_be_bytes([
            state.buffer[1],
            state.buffer[2],
            state.buffer[3],
            state.buffer[4],
        ]);
        let length = usize::try_from(length).unwrap_or(usize::MAX);
        if state.buffer.len() < 5 + length {
            return;
        }
        let message: Vec<u8> = state.buffer[5..5 + length].to_vec();
        state.buffer.drain(..5 + length);
        if flag == 0x02 {
            state.done = true;
            return;
        }
        if flag != 0 {
            continue;
        }
        match pb::TurnEvent::decode_from_slice(&message) {
            Ok(event) => state.pending.push_back(Ok(event)),
            Err(error) => {
                state.done = true;
                state
                    .pending
                    .push_back(Err(ErrorTrace::new(AgentRpcError::Transport(format!(
                        "a turn event did not decode: {error}"
                    )))));
                return;
            }
        }
    }
}

/// The forwarded principal header pair.
///
/// The Durable Object trusts this header only because the Worker removes every
/// incoming `ewe-forwarded-*` header before it adds its own (invariant I4).
#[must_use]
pub fn forwarded_principal_header(subject: &str) -> (String, String) {
    (FORWARDED_PRINCIPAL_HEADER.to_string(), subject.to_string())
}
