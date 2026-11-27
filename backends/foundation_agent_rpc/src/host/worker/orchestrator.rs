//! The thin orchestrator entry on a Cloudflare Worker (spec 83 feature 06,
//! T4 and T5).
//!
//! WHY: a Worker cannot run a local large language model (LLM). It can run the
//! agentic layer of `foundation_ai` with a remote model: Workers AI by default,
//! or a remote native agent-rpc when a custom model is necessary. The Worker is
//! therefore a thin orchestrator: it owns no model and no instance state
//! between requests.
//!
//! WHAT: [`Orchestrator`] (the Worker entry over `foundation_http`'s
//! `dispatch_cf` and the shared Connect router), [`OrchestratorConfig`] (its
//! settings and limits) and [`RemoteAgent`] (the Connect client of a remote
//! native agent-rpc, the F24 transport of spec 85).
//!
//! HOW: `Orchestrator::new` builds the Worker host in its orchestrator shape:
//! the agentic layer, the remote catalog and an in-isolate instance registry,
//! served by the same Connect router as the full host. The JavaScript runtime
//! polls [`Orchestrator::fetch`]. `RemoteAgent` calls a remote agent-rpc over
//! `H1Transport`, which is the fetch transport on wasm; its response stays a
//! stream.

use std::sync::Arc;

use foundation_compact::SendWrapper;
use foundation_connectrpc::{ClientOptions, H1Transport, Request, RpcCtx, ServerStream};
use foundation_context::Ctx;

use crate::core::registry::TurnEventStream;
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::host::worker::entry::WorkerHost;
use crate::proto::v1 as pb;
use crate::proto::v1::agent_service::AgentServiceClient;

/// The Worker var that names the remote native agent-rpc.
///
/// `None` means this Worker uses Workers AI only.
pub const REMOTE_URL_VAR: &str = "AGENT_RPC_REMOTE_URL";
/// The Worker secret that holds the bearer token of the remote agent-rpc.
pub const REMOTE_TOKEN_SECRET: &str = "AGENT_RPC_REMOTE_TOKEN";

/// The settings of the thin orchestrator.
///
/// Every limit of the served calls is a field of the Worker host configuration
/// (spec 83 feature 05, section 5.2). This struct adds only the remote model
/// source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrchestratorConfig {
    /// The remote native agent-rpc base URL, when the Worker has one.
    pub remote_url: Option<String>,
    /// The Worker secret that holds the remote bearer token.
    pub remote_token_secret: String,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            remote_url: None,
            remote_token_secret: REMOTE_TOKEN_SECRET.to_string(),
        }
    }
}

impl OrchestratorConfig {
    /// Read the settings of the Worker.
    #[must_use]
    pub fn from_env(env: &worker::Env) -> Self {
        let remote_url = env
            .var(REMOTE_URL_VAR)
            .map(|value| value.to_string())
            .ok()
            .filter(|value| !value.is_empty());
        Self {
            remote_url,
            ..Self::default()
        }
    }
}

/// The thin orchestrator of one Worker isolate.
pub struct Orchestrator {
    host: WorkerHost,
    config: OrchestratorConfig,
}

impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Orchestrator")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Orchestrator {
    /// Build the orchestrator once per isolate.
    ///
    /// The caller installs the `Env` on `ctx` first: the in-isolate registry
    /// reads its identifiers and its timer from it.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for a bad var and [`AgentRpcError::Auth`]
    /// for bad public keys.
    pub async fn new(env: worker::Env, ctx: Ctx) -> AgentRpcResult<Self> {
        let config = OrchestratorConfig::from_env(&env);
        Ok(Self {
            host: WorkerHost::new_orchestrator(env, ctx).await?,
            config,
        })
    }

    /// Serve one request from the internet.
    ///
    /// The response body is a `ReadableStream`, so a turn streams to the
    /// caller as the model makes it.
    ///
    /// # Errors
    /// A JavaScript error while building the response.
    pub async fn fetch(&self, req: worker::Request) -> worker::Result<worker::Response> {
        self.host.fetch(req).await
    }

    /// The remote native agent-rpc of this Worker, when it has one.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when the remote URL or the bearer secret is
    /// missing.
    pub fn remote_agent(&self, env: &worker::Env, ctx: Ctx) -> AgentRpcResult<Option<RemoteAgent>> {
        let Some(url) = self.config.remote_url.as_ref() else {
            return Ok(None);
        };
        let token = env
            .secret(&self.config.remote_token_secret)
            .map(|value| value.to_string())
            .ok();
        RemoteAgent::new(url, token.as_deref(), ctx).map(Some)
    }
}

/// The Connect client of one remote native agent-rpc.
///
/// It uses `H1Transport`, which is the fetch transport on wasm (spec 85). A
/// turn streams back through the Worker; no step buffers it.
pub struct RemoteAgent {
    client: Arc<AgentServiceClient>,
    ctx: Ctx,
}

impl std::fmt::Debug for RemoteAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteAgent").finish_non_exhaustive()
    }
}

impl RemoteAgent {
    /// A client of `url`, with the optional bearer token.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when the URL is not a valid client base URL.
    pub fn new(url: &str, token: Option<&str>, ctx: Ctx) -> AgentRpcResult<Self> {
        let transport: Arc<dyn foundation_connectrpc::Transport> = Arc::new(H1Transport::default());
        let options = match token {
            Some(token) => ClientOptions::new()
                .with_header("Authorization".to_string(), format!("Bearer {token}")),
            None => ClientOptions::new(),
        };
        let client = AgentServiceClient::new(transport, url, options).map_err(|error| {
            AgentRpcError::Config(format!("the remote agent-rpc URL is not valid: {error}"))
        })?;
        Ok(Self {
            client: Arc::new(client),
            ctx,
        })
    }

    /// One turn on `instance` as a stream of events.
    ///
    /// # Errors
    /// The Connect error of the call arrives as the first stream item, so the
    /// caller reads it as a typed error and never as a panic.
    pub fn chat_stream(&self, instance: &str, prompt: &str) -> TurnEventStream {
        let client = Arc::clone(&self.client);
        let ctx = self.ctx.clone();
        let request = pb::ChatRequest {
            instance: instance.to_string(),
            prompt: prompt.to_string(),
            ..Default::default()
        };
        let state = RemoteChatState {
            client,
            ctx,
            request: Some(request),
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
                        .client
                        .chat_stream(RpcCtx::client(state.ctx.clone()), Request::new(request)),
                )
                .await
                {
                    Ok(stream) => state.stream = Some(SendWrapper::new(stream)),
                    Err(error) => {
                        state.done = true;
                        return Some((Err(transport_error(error)), state));
                    }
                }
            }
            let stream = state.stream.as_mut()?;
            match SendWrapper::new(stream.receive()).await {
                Ok(Some(event)) => Some((Ok(event), state)),
                Ok(None) => None,
                Err(error) => {
                    state.done = true;
                    Some((Err(transport_error(error)), state))
                }
            }
        }))
    }
}

/// The state of one remote turn stream.
struct RemoteChatState {
    client: Arc<AgentServiceClient>,
    ctx: Ctx,
    request: Option<pb::ChatRequest>,
    stream: Option<SendWrapper<ServerStream<pb::TurnEvent>>>,
    done: bool,
}

fn transport_error(
    error: impl std::fmt::Display,
) -> foundation_errstacks::ErrorTrace<AgentRpcError> {
    AgentRpcError::Transport(error.to_string()).into()
}

/// The entry of the thin orchestrator.
///
/// It is not a `#[wasm_bindgen]` export: the Worker crate declares the export
/// and calls this function, because the build is async and the export must
/// stay in the crate that `wrangler` loads. The caller initializes the
/// single-threaded valtron pool and installs the `Env` first.
///
/// # Errors
/// [`AgentRpcError::Config`] for a bad var and [`AgentRpcError::Auth`] for bad
/// public keys.
pub async fn wasm_worker_entry(env: worker::Env, ctx: Ctx) -> AgentRpcResult<Orchestrator> {
    Orchestrator::new(env, ctx).await
}
