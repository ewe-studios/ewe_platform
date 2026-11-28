//! The model mode of `agent-slave` (spec 92 feature 07).
//!
//! WHY: a Cloud Run service runs one slave and the operator's local master
//! sends it turns. The slave must answer a turn itself, because the master
//! never holds the provider key. This module serves the same `SlaveService` as
//! the guest slave, but one `Exec` call is one model turn: the argument vector
//! joins into the prompt and the answer streams on standard output.
//!
//! WHAT: [`ModelConfig`] (the environment of the mode), [`ModelTurn`] (one
//! turn), [`OpenRouterTurn`] (the real turn through `foundation_ai`'s
//! OpenAI-compatible provider) and [`ModelSlave`] (the service).
//!
//! HOW: the listener speaks HTTP/1.1 and h2c on one port, because Cloud Run
//! forwards HTTP/2 when the service asks for it (`--use-http2`). Every call
//! must carry `Authorization: Bearer <EWE_SLAVE_TOKEN>`; a call without it is
//! `Unauthenticated`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use foundation_ai::backends::openai_provider::{OpenAIConfig, OpenAIProvider};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelProvider, ToolDeclarations,
};
use foundation_connectrpc::{
    ConnectError, ConnectResult, ConnectRpcServe, ConnectRpcServeH2, Request, Response, RpcCtx,
    Router,
};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_http::native::app::ServerApp;
use foundation_http::native::serve::{H2Serve, Serve};
use foundation_http::native::server::{HttpServer, ServerConfig};
use foundation_http::native::app::{HttpApp as NativeHttp1App, HttpApp as NativeHttp2App};
use foundation_http::ShutdownSignal;
use foundation_sandbox::native::model::ModelConfig as _;
use futures::{pin_mut, select, FutureExt, Stream};

use crate::native::authz::BearerAuth;
use crate::shared::error::{SandboxError, SandboxResult, SlaveErrorCode};
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::messages::{
    Empty, ExecEventKind, ExecEventMsg, ExecInputRequest, ExecInputResponse, ExecRequest,
    ExecSignalRequest, FileChunk, FileReadRequest, FileWriteAbortRequest, FileWriteBeginRequest,
    FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse, FileWriteCommitRequest,
    FileWriteCommitResponse, InfoRequest, PrepareRequest, PrepareResponse, ShutdownRequest,
    SlaveInfo,
};
use crate::shared::rpc::slave::{procedure, register_slave_service, SlaveService};
use crate::shared::{BoxFuture, Secret};

/// The default base URL of the OpenAI-compatible provider.
///
/// The provider appends the API version (`v1`) and the endpoint, so the turn
/// reaches `https://openrouter.ai/api/v1/chat/completions`.
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api";

/// The default deadline of one turn.
///
/// 120 s: the model slave of the Cloudflare part uses the same default, and it
/// leaves room for a slow free model.
pub const DEFAULT_TURN_TIMEOUT: Duration = Duration::from_secs(120);

/// The largest prompt. Default 64 KiB: the same cap as the Cloudflare model
/// slave.
pub const DEFAULT_MAX_PROMPT_BYTES: usize = 64 * 1024;

/// The largest answer. Default 1 MiB: the same cap as the Cloudflare model
/// slave.
pub const DEFAULT_MAX_ANSWER_BYTES: usize = 1024 * 1024;

/// The configuration of the model mode.
///
/// The two secrets enter from the environment in the binary entry point. Every
/// limit is a field, so a test and an operator can change it.
#[derive(Clone)]
pub struct ModelConfig {
    /// The bearer token that every call must carry.
    pub token: Secret,
    /// The provider key.
    pub api_key: Secret,
    /// The provider's model id, for example `google/gemma-4-26b-a4b-it:free`.
    pub model_id: String,
    /// The provider base URL, without the API version.
    pub base_url: String,
    /// The deadline of one turn.
    pub turn_timeout: Duration,
    /// The largest prompt in bytes.
    pub max_prompt_bytes: usize,
    /// The largest answer in bytes.
    pub max_answer_bytes: usize,
}

impl std::fmt::Debug for ModelConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelConfig")
            .field("model_id", &self.model_id)
            .field("base_url", &self.base_url)
            .field("turn_timeout", &self.turn_timeout)
            .field("max_prompt_bytes", &self.max_prompt_bytes)
            .field("max_answer_bytes", &self.max_answer_bytes)
            .finish_non_exhaustive()
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            token: Secret::new(String::new()),
            api_key: Secret::new(String::new()),
            model_id: String::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
            turn_timeout: DEFAULT_TURN_TIMEOUT,
            max_prompt_bytes: DEFAULT_MAX_PROMPT_BYTES,
            max_answer_bytes: DEFAULT_MAX_ANSWER_BYTES,
        }
    }
}

impl ModelConfig {
    /// Reads the configuration of the mode from the environment.
    ///
    /// `EWE_SLAVE_TOKEN`, `OPENROUTER_API_KEY` and `EWE_MODEL_ID` are
    /// required. `EWE_MODEL_BASE_URL` and `EWE_MODEL_TIMEOUT_MS` have defaults.
    ///
    /// # Errors
    /// A text that names the missing or malformed value.
    pub fn from_env() -> Result<Self, String> {
        let token = required("EWE_SLAVE_TOKEN")?;
        let api_key = required("OPENROUTER_API_KEY")?;
        let model_id = required("EWE_MODEL_ID")?;
        let base_url =
            std::env::var("EWE_MODEL_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let turn_timeout = match std::env::var("EWE_MODEL_TIMEOUT_MS") {
            Ok(value) => Duration::from_millis(
                value
                    .parse()
                    .map_err(|error| format!("EWE_MODEL_TIMEOUT_MS is not a number: {error}"))?,
            ),
            Err(_) => DEFAULT_TURN_TIMEOUT,
        };
        Ok(Self {
            token: Secret::new(token),
            api_key: Secret::new(api_key),
            model_id,
            base_url,
            turn_timeout,
            max_prompt_bytes: DEFAULT_MAX_PROMPT_BYTES,
            max_answer_bytes: DEFAULT_MAX_ANSWER_BYTES,
        })
    }

    /// Replaces [`Self::turn_timeout`].
    #[must_use]
    pub fn with_turn_timeout(mut self, value: Duration) -> Self {
        self.turn_timeout = value;
        self
    }

    /// Replaces [`Self::max_prompt_bytes`].
    #[must_use]
    pub fn with_max_prompt_bytes(mut self, value: usize) -> Self {
        self.max_prompt_bytes = value;
        self
    }

    /// Replaces [`Self::max_answer_bytes`].
    #[must_use]
    pub fn with_max_answer_bytes(mut self, value: usize) -> Self {
        self.max_answer_bytes = value;
        self
    }
}

/// Reads one required environment value.
fn required(name: &str) -> Result<String, String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => Err(format!("{name} is not set")),
    }
}

/// One model turn.
pub trait ModelTurn: Send + Sync + 'static {
    /// Answers `prompt` with the bytes of standard output.
    fn turn(&self, ctx: Ctx, prompt: String) -> BoxFuture<'_, SandboxResult<Vec<u8>>>;
}

/// The real turn, through the OpenAI-compatible provider.
pub struct OpenRouterTurn {
    api_key: Secret,
    model_id: String,
    base_url: String,
    timeout: Duration,
}

impl OpenRouterTurn {
    /// A turn over `base_url` with `model_id`.
    #[must_use]
    pub fn new(api_key: Secret, model_id: String, base_url: String, timeout: Duration) -> Self {
        Self {
            api_key,
            model_id,
            base_url,
            timeout,
        }
    }
}

impl ModelTurn for OpenRouterTurn {
    fn turn(&self, ctx: Ctx, prompt: String) -> BoxFuture<'_, SandboxResult<Vec<u8>>> {
        let config = OpenAIConfig::openrouter(self.api_key.expose())
            .with_base_url(self.base_url.clone())
            // The HTTP client takes whole seconds; one second is its floor.
            .with_timeout_secs(u64::try_from(self.timeout.as_secs()).unwrap_or(u64::MAX).max(1))
            // One unary answer: no server-sent event stream.
            .with_streaming(false);
        let model_id = self.model_id.clone();
        Box::pin(async move {
            let provider = OpenAIProvider::new(ctx)
                .create(Some(config))
                .map_err(|error| provider_error("start the provider", &error))?;
            let model = provider
                .get_model(ModelId::from(model_id.as_str()))
                .map_err(|error| provider_error("select the model", &error))?;
            let interaction = ModelInteraction {
                system_prompt: None,
                soul: None,
                // The demo has no tools: one prompt, one text answer.
                tools_shed: ToolDeclarations {
                    shed: None,
                    tools: Vec::new(),
                },
                messages: vec![Messages::user(prompt)],
                chat_template: None,
                tool_choice: None,
            };
            let answers = model
                .generate_async(interaction, None)
                .await
                .map_err(|error| provider_error("run the turn", &error))?;
            let mut text = String::new();
            for answer in answers {
                if let Messages::Assistant {
                    content: ModelOutput::Text(content),
                    ..
                } = answer
                {
                    text.push_str(&content.content);
                }
            }
            if text.is_empty() {
                return Err(interior("the provider returned no text"));
            }
            Ok(text.into_bytes())
        })
    }
}

/// The one sandbox error that an internal failure uses.
fn interior(message: &str) -> ErrorTrace<SandboxError> {
    SandboxError::Rpc {
        code: "internal".to_string(),
        message: message.to_string(),
    }
    .into()
}

/// The sandbox error of a provider failure.
fn provider_error(stage: &str, error: &impl std::fmt::Display) -> ErrorTrace<SandboxError> {
    SandboxError::Rpc {
        code: "unavailable".to_string(),
        message: format!("openrouter: {stage}: {error}"),
    }
    .into()
}

/// The model slave: `SlaveService` with one turn per `Exec`.
pub struct ModelSlave {
    ctx: Ctx,
    turn: Arc<dyn ModelTurn>,
    config: ModelConfig,
}

impl ModelSlave {
    /// A slave over `turn` and `config`.
    #[must_use]
    pub fn new(ctx: Ctx, turn: Arc<dyn ModelTurn>, config: ModelConfig) -> Self {
        Self { ctx, turn, config }
    }

    /// One turn for one request.
    async fn answer(&self, request: &ExecRequest) -> SandboxResult<Vec<u8>> {
        let prompt = request.argv.join(" ");
        if prompt.is_empty() {
            return Err(SandboxError::SlaveError {
                code: SlaveErrorCode::BadRequest,
                errno: None,
                message: "the prompt is empty: put the text in argv".to_string(),
            }
            .into());
        }
        if prompt.len() > self.config.max_prompt_bytes {
            return Err(SandboxError::Rpc {
                code: "resource_exhausted".to_string(),
                message: format!(
                    "the prompt is larger than the cap of {} bytes",
                    self.config.max_prompt_bytes
                ),
            }
            .into());
        }
        let timeout = request
            .timeout_ms
            .map_or(self.config.turn_timeout, Duration::from_millis);
        let turn = self.turn.turn(self.ctx.clone(), prompt).fuse();
        let timer = foundation_core::valtron::timer::sleep_for(timeout, &self.ctx).fuse();
        pin_mut!(turn, timer);
        let answer = match select(turn, timer).await {
            futures::future::Either::Left((answer, _timer)) => answer?,
            futures::future::Either::Right(((), _turn)) => {
                return Err(SandboxError::Rpc {
                    code: "deadline_exceeded".to_string(),
                    message: format!("the model turn passed its deadline of {timeout:?}"),
                }
                .into());
            }
        };
        if answer.len() > self.config.max_answer_bytes {
            return Err(SandboxError::Rpc {
                code: "resource_exhausted".to_string(),
                message: format!(
                    "the answer is larger than the cap of {} bytes",
                    self.config.max_answer_bytes
                ),
            }
            .into());
        }
        Ok(answer)
    }
}

/// One sandbox error as a Connect error.
fn connect_error(error: ErrorTrace<SandboxError>) -> ErrorTrace<ConnectError> {
    let max = SandboxLimits::default().max_error_message;
    ErrorTrace::new(error.current_context().to_connect(max))
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "the service trait declares async methods; two bodies answer from the turn"
)]
impl SlaveService for ModelSlave {
    async fn info(
        &self,
        _ctx: RpcCtx,
        _request: Request<InfoRequest>,
    ) -> ConnectResult<Response<SlaveInfo>> {
        Ok(Response::new(SlaveInfo {
            slave_version: crate::VERSION.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            // The kernel field names the model path, so a caller can see that
            // this slave answers through OpenRouter.
            kernel: format!("openrouter:{}", self.config.model_id),
            is_pid1: false,
            prepared: false,
            can_control: false,
        }))
    }

    async fn prepare(
        &self,
        _ctx: RpcCtx,
        request: Request<PrepareRequest>,
    ) -> ConnectResult<Response<PrepareResponse>> {
        // The model mode runs no program, so it has no root to set up. It
        // accepts `Prepare` so the same client can drive both slave kinds.
        Ok(Response::new(PrepareResponse {
            root_mode: request.msg.root.mode,
        }))
    }

    async fn exec(
        &self,
        _ctx: RpcCtx,
        request: Request<ExecRequest>,
    ) -> ConnectResult<
        std::pin::Pin<Box<dyn Stream<Item = ConnectResult<ExecEventMsg>> + Send + 'static>>,
    > {
        let events = match self.answer(&request.msg).await {
            Ok(answer) => vec![
                Ok(ExecEventMsg {
                    seq: 0,
                    event: ExecEventKind::Started { pid: 0 },
                }),
                Ok(ExecEventMsg {
                    seq: 1,
                    event: ExecEventKind::Stdout { data: answer },
                }),
                Ok(ExecEventMsg {
                    seq: 2,
                    event: ExecEventKind::Exit {
                        code: Some(0),
                        signal: None,
                    },
                }),
            ],
            Err(error) => vec![Err(connect_error(error))],
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }

    async fn exec_input(
        &self,
        _ctx: RpcCtx,
        _request: Request<ExecInputRequest>,
    ) -> ConnectResult<Response<ExecInputResponse>> {
        Err(unsupported("exec input"))
    }

    async fn exec_signal(
        &self,
        _ctx: RpcCtx,
        _request: Request<ExecSignalRequest>,
    ) -> ConnectResult<Response<Empty>> {
        Err(unsupported("exec signal"))
    }

    async fn file_write_begin(
        &self,
        _ctx: RpcCtx,
        _request: Request<FileWriteBeginRequest>,
    ) -> ConnectResult<Response<FileWriteBeginResponse>> {
        Err(unsupported("file write"))
    }

    async fn file_write_chunk(
        &self,
        _ctx: RpcCtx,
        _request: Request<FileWriteChunkRequest>,
    ) -> ConnectResult<Response<FileWriteChunkResponse>> {
        Err(unsupported("file write"))
    }

    async fn file_write_commit(
        &self,
        _ctx: RpcCtx,
        _request: Request<FileWriteCommitRequest>,
    ) -> ConnectResult<Response<FileWriteCommitResponse>> {
        Err(unsupported("file write"))
    }

    async fn file_write_abort(
        &self,
        _ctx: RpcCtx,
        _request: Request<FileWriteAbortRequest>,
    ) -> ConnectResult<Response<Empty>> {
        Err(unsupported("file write"))
    }

    async fn file_read(
        &self,
        _ctx: RpcCtx,
        _request: Request<FileReadRequest>,
    ) -> ConnectResult<
        std::pin::Pin<Box<dyn Stream<Item = ConnectResult<FileChunk>> + Send + 'static>>,
    > {
        Err(unsupported("file read"))
    }

    async fn shutdown(
        &self,
        _ctx: RpcCtx,
        _request: Request<ShutdownRequest>,
    ) -> ConnectResult<Response<Empty>> {
        Ok(Response::new(Empty {}))
    }
}

/// The error of a call that the model slave does not serve.
fn unsupported(capability: &str) -> ErrorTrace<ConnectError> {
    connect_error(
        SandboxError::UnsupportedCapability {
            capability: capability.to_string(),
        }
        .into(),
    )
}

/// Binds the model listener. `shutdown` stops the server.
///
/// # Errors
/// The operating system error of the bind call.
pub fn bind(addr: SocketAddr, shutdown: &Arc<ShutdownSignal>) -> std::io::Result<std::net::TcpListener> {
    // The server parks its accept loop on the shared reactor; the signal wakes
    // it at once instead of after the poll time.
    let _ = shutdown.register_wake();
    std::net::TcpListener::bind(addr)
}

/// Serves `SlaveService` over HTTP/1.1 and h2c until `shutdown` turns on.
pub fn serve(
    ctx: Ctx,
    config: ModelConfig,
    listener: &std::net::TcpListener,
    shutdown: Arc<ShutdownSignal>,
) {
    let turn = Arc::new(OpenRouterTurn::new(
        config.api_key.clone(),
        config.model_id.clone(),
        config.base_url.clone(),
        config.turn_timeout,
    ));
    let service = Arc::new(ModelSlave::new(ctx.clone(), turn, config.clone()));
    let auth = Arc::new(BearerAuth::new(
        config.token.clone(),
        SandboxLimits::default().max_error_message,
    ));

    // Cloud Run sends h2c to the container when the service uses HTTP/2
    // end-to-end, and HTTP/1.1 otherwise. One port serves both.
    let app = routed_app(&slave_paths(), || {
        let mut router = Router::new()
            .with_global_interceptors(vec![Arc::clone(&auth) as Arc<dyn foundation_connectrpc::Interceptor>]);
        register_slave_service(&mut router, Arc::clone(&service));
        router
    });

    let server = HttpServer::from_app_with_config(app, "ewe-model-slave", ServerConfig::defaults());
    server.serve_with_listener(listener, &shutdown);
}

/// Every procedure path of `SlaveService`.
fn slave_paths() -> [&'static str; 18] {
    [
        procedure::INFO,
        procedure::PREPARE,
        procedure::EXEC,
        procedure::EXEC_INPUT,
        procedure::EXEC_SIGNAL,
        procedure::FILE_WRITE_BEGIN,
        procedure::FILE_WRITE_CHUNK,
        procedure::FILE_WRITE_COMMIT,
        procedure::FILE_WRITE_ABORT,
        procedure::FILE_READ,
        procedure::UNPACK_BEGIN,
        procedure::UNPACK_CHUNK,
        procedure::UNPACK_COMMIT,
        procedure::UNPACK_ABORT,
        procedure::SHUTDOWN,
    ]
}

/// The HTTP/1.1 and h2c applications of one router factory.
fn routed_app(paths: &[&str], router: impl Fn() -> Router) -> ServerApp {
    let h1: Arc<dyn Serve> = Arc::new(ConnectRpcServe::new(router().into_handler()));
    let h2: Arc<dyn H2Serve> = Arc::new(ConnectRpcServeH2::new(router().into_handler()));
    let mut http1 = NativeHttp1App::new_serve();
    let mut http2 = NativeHttp2App::new_h2_serve();
    for path in paths {
        http1.router.add_route_any(path, &h1);
        http2.route_any_h2(path, Arc::clone(&h2));
    }
    ServerApp::both(http1, http2)
}
