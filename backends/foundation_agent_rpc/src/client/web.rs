//! The browser client (spec 83 feature 05, section 6.3).
//!
//! WHY: a page is Wasm compiled by `wasm-bindgen`. It calls a Worker or a
//! native host over Connect with the browser `fetch`. It shows a bearer secret
//! on each call, because a page cannot choose a client certificate.
//!
//! WHAT: [`WebClient`] and its calls. [`WebClient::from_access_payload`] uses
//! an operator-issued access secret. Enrollment with a client key is a
//! separate step; see the module note.
//!
//! HOW: [`H1Transport::default`] gives the same transport as every other
//! caller, with `FetchHttpClient` under it. Every call carries
//! `Authorization: Bearer <access secret>`. A dropped `ChatStream` drops the
//! body reader, and the runtime cancels the response.
//!
//! Note: enrollment (`Enroll` with a WebCrypto client key) and IndexedDB
//! credential storage are not in this module yet. They need a host that serves
//! `BootstrapService`; the Worker host refuses enrollment until that path is
//! built. Use an operator-issued access payload.

use std::sync::Arc;

use foundation_compact::SendWrapper;
use foundation_connectrpc::{
    ClientOptions, ConnectError, H1Transport, Request, RpcCtx, ServerStream,
};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;

use crate::core::bootstrap::{decode, BootstrapAccess};
use crate::core::browser::check_browser_payload;
use crate::core::model::{Device, ModelKind};
use crate::core::registry::{InstanceInfo, InstanceSpec, TurnEventStream};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1 as pb;
use crate::proto::v1::agent_service::AgentServiceClient;

/// A browser client over one host.
pub struct WebClient {
    agent: Arc<AgentServiceClient>,
    /// The base URL of the host.
    url: String,
    /// The bearer secret of the current access window.
    access_secret: zeroize::Zeroizing<String>,
    ctx: Ctx,
}

impl std::fmt::Debug for WebClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebClient")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl WebClient {
    /// Use an operator-issued payload whose token purpose is `Access`.
    ///
    /// The token is the bearer secret until it expires. The payload rules of
    /// section 6.4 apply: a pinned certificate and a host without the
    /// signed-secret mode are refused.
    ///
    /// # Errors
    /// `Codec` for a bad payload; `Config` for a payload that a browser cannot
    /// use; `Unauthenticated` for a token outside its window.
    pub fn from_access_payload(payload: &str) -> AgentRpcResult<Self> {
        let bootstrap = decode(payload)?;
        check_browser_payload(&bootstrap)?;
        let BootstrapAccess::Bearer { token } = bootstrap.access else {
            return Err(AgentRpcError::Config(
                "this payload enrolls a client; use an access payload for from_access_payload"
                    .to_string(),
            )
            .into());
        };
        let secret = token.encode();
        let options = ClientOptions::new().with_header(
            "Authorization".to_string(),
            format!("Bearer {}", secret.as_str()),
        );
        let transport = Arc::new(H1Transport::default());
        let url = bootstrap.url.to_string();
        let agent = AgentServiceClient::new(transport, &url, options).map_err(|error| {
            ErrorTrace::new(AgentRpcError::Config(format!(
                "the browser client cannot be built: {error}"
            )))
        })?;
        Ok(Self {
            agent: Arc::new(agent),
            url,
            access_secret: secret,
            ctx: Ctx::host(),
        })
    }

    /// The base URL of the host.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The capability report of the host.
    ///
    /// # Errors
    /// The wire status of a failed call.
    pub async fn capabilities(&self) -> AgentRpcResult<pb::GetCapabilitiesResponse> {
        let request = Request::new(pb::GetCapabilitiesRequest::default());
        let response = SendWrapper::new(
            self.agent
                .get_capabilities(RpcCtx::client(self.ctx.clone()), request),
        )
        .await
        .map_err(|error| map_error(&error))?;
        Ok(response.msg)
    }

    /// Create one instance.
    ///
    /// # Errors
    /// The wire status of a failed call.
    pub async fn create_instance(&self, spec: InstanceSpec) -> AgentRpcResult<InstanceInfo> {
        let request = Request::new(create_request(&spec));
        let response = SendWrapper::new(
            self.agent
                .create_instance(RpcCtx::client(self.ctx.clone()), request),
        )
        .await
        .map_err(|error| map_error(&error))?;
        Ok(instance_info(response.msg))
    }

    /// Destroy one instance.
    ///
    /// # Errors
    /// The wire status of a failed call.
    pub async fn destroy_instance(&self, name: &str) -> AgentRpcResult<()> {
        let request = Request::new(pb::DestroyInstanceRequest {
            name: name.to_string(),
            ..Default::default()
        });
        SendWrapper::new(
            self.agent
                .destroy_instance(RpcCtx::client(self.ctx.clone()), request),
        )
        .await
        .map_err(|error| map_error(&error))?;
        Ok(())
    }

    /// One turn as a stream of events.
    ///
    /// Dropping the stream drops the body reader, which cancels the call.
    #[must_use]
    pub fn chat_stream(&self, instance: &str, prompt: &str) -> TurnEventStream {
        let state = ChatState {
            agent: Arc::clone(&self.agent),
            ctx: self.ctx.clone(),
            request: Some(pb::ChatRequest {
                instance: instance.to_string(),
                prompt: prompt.to_string(),
                ..Default::default()
            }),
            stream: None,
            done: false,
        };
        Box::pin(futures::stream::unfold(state, |mut state| async move {
            if state.done {
                return None;
            }
            if state.stream.is_none() {
                let request = state.request.take()?;
                match SendWrapper::new(
                    state
                        .agent
                        .chat_stream(RpcCtx::client(state.ctx.clone()), Request::new(request)),
                )
                .await
                {
                    Ok(stream) => state.stream = Some(SendWrapper::new(stream)),
                    Err(error) => {
                        state.done = true;
                        return Some((Err(map_error(&error)), state));
                    }
                }
            }
            let stream = state.stream.as_mut()?;
            match SendWrapper::new(stream.receive()).await {
                Ok(Some(event)) => Some((Ok(event), state)),
                Ok(None) => None,
                Err(error) => {
                    state.done = true;
                    Some((Err(map_error(&error)), state))
                }
            }
        }))
    }

    /// The number of characters of the bearer secret. A test hook that proves
    /// the secret travels and is never logged in full.
    #[doc(hidden)]
    #[must_use]
    pub fn access_secret_len(&self) -> usize {
        self.access_secret.len()
    }
}

struct ChatState {
    agent: Arc<AgentServiceClient>,
    ctx: Ctx,
    request: Option<pb::ChatRequest>,
    stream: Option<SendWrapper<ServerStream<pb::TurnEvent>>>,
    done: bool,
}

fn map_error(error: &ErrorTrace<ConnectError>) -> ErrorTrace<AgentRpcError> {
    ErrorTrace::new(AgentRpcError::Transport(
        error.current_context().to_string(),
    ))
}

fn create_request(spec: &InstanceSpec) -> pb::CreateInstanceRequest {
    let device: buffa::MessageField<pb::Device, buffa::Inline<pb::Device>> = spec
        .model
        .device
        .map_or_else(buffa::MessageField::default, |device| {
            Some(device.to_wire()).into()
        });
    pb::CreateInstanceRequest {
        name: spec.name.clone(),
        model: spec.model.name.clone().unwrap_or_default(),
        system_prompt: spec.system_prompt.clone(),
        max_tokens: spec
            .max_tokens
            .and_then(|tokens| u32::try_from(tokens).ok()),
        temperature: spec.temperature,
        kind: buffa::EnumValue::Known(spec.model.kind.map_or_default(ModelKind::to_wire)),
        device,
        ..Default::default()
    }
}

fn instance_info(info: pb::InstanceInfo) -> InstanceInfo {
    InstanceInfo {
        name: info.name,
        model: info.model,
        kind: ModelKind::Api,
        device: Device::Remote,
        created_unix_ms: info.created_unix_ms,
        turns: info.turns,
    }
}
