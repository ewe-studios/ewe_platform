//! The model slave of decision D16.
//!
//! A model slave is a second Durable Object (DO) that answers the
//! `SlaveService` with model turns. It gives no operating-system isolation: the
//! model never runs a program, it answers text. It exists so a model runtime is
//! reachable through the same `SandboxService` and `Provider` surface as an
//! isolated sandbox.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::{ConnectResult, Request, Response, RpcCtx};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use futures::future::{select, Either};

use crate::shared::cloudflare::control::{HttpRequestParts, HttpResponseParts};
use crate::shared::cloudflare::DoStorage;
use crate::shared::control::wait_until;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::rpc::messages::{
    Empty, ExecEventKind, ExecEventMsg, ExecInputRequest, ExecInputResponse, ExecRequest,
    ExecSignalRequest, FileChunk, FileReadRequest, FileWriteAbortRequest, FileWriteBeginRequest,
    FileWriteBeginResponse, FileWriteChunkRequest, FileWriteChunkResponse, FileWriteCommitRequest,
    FileWriteCommitResponse, InfoRequest, PrepareRequest, PrepareResponse, ShutdownRequest,
    SlaveInfo,
};
use crate::shared::rpc::SlaveService;
use crate::shared::BoxFuture;
use crate::shared::Secret;

/// The model service of one turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelBackend {
    /// `OpenRouter`, with its base URL and model name.
    OpenRouter {
        /// The base URL of the service.
        base_url: String,
        /// The operator's model name. There is no built-in default.
        model: String,
    },
    /// The Cloudflare Model API.
    CloudflareAi {
        /// The operator's model name. There is no built-in default.
        model: String,
    },
}

/// The configuration of the model slave.
///
/// Every limit is configuration. Build a value with [`ModelConfig::default`] and
/// change the fields that matter with the `with_` methods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelConfig {
    /// The backend, or `None` for none.
    ///
    /// Default `None`: a launch with no backend is `UnsupportedCapability`.
    pub backend: Option<ModelBackend>,
    /// The `Ctx` entry name that holds the `OpenRouter` key.
    ///
    /// Default `None`. An `OpenRouter` backend with no entry is
    /// `HostNotConfigured`.
    pub api_key_entry: Option<String>,
    /// The cap on the prompt built from the program and its arguments.
    ///
    /// Default 64 KiB.
    pub max_prompt_bytes: usize,
    /// The cap on standard output.
    ///
    /// Default 1 MiB. A larger answer ends `exec` with
    /// `Rpc { code: "resource_exhausted" }`.
    pub max_answer_bytes: usize,
    /// The deadline of one turn.
    ///
    /// Default 120 s.
    pub turn_timeout: Duration,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            backend: None,
            api_key_entry: None,
            max_prompt_bytes: 64 * 1024,
            max_answer_bytes: 1024 * 1024,
            turn_timeout: Duration::from_secs(120),
        }
    }
}

impl ModelConfig {
    /// A configuration with every default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces [`Self::backend`].
    #[must_use]
    pub fn with_backend(mut self, value: Option<ModelBackend>) -> Self {
        self.backend = value;
        self
    }

    /// Replaces [`Self::api_key_entry`].
    #[must_use]
    pub fn with_api_key_entry(mut self, value: Option<String>) -> Self {
        self.api_key_entry = value;
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

    /// Replaces [`Self::turn_timeout`].
    #[must_use]
    pub fn with_turn_timeout(mut self, value: Duration) -> Self {
        self.turn_timeout = value;
        self
    }
}

/// The model provider keys, by the name that [`ModelConfig::api_key_entry`]
/// gives.
///
/// A host stores one value of this type in the [`Ctx`]. The key never enters a
/// record or a log: [`Secret`] redacts it in `Debug`, `Display` and `Serialize`.
#[derive(Clone, Default)]
pub struct ModelSecrets {
    keys: BTreeMap<String, Secret>,
}

impl ModelSecrets {
    /// No keys.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same set with one more key.
    #[must_use]
    pub fn with_key(mut self, name: impl Into<String>, key: Secret) -> Self {
        self.keys.insert(name.into(), key);
        self
    }

    /// The key under `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Secret> {
        self.keys.get(name)
    }
}

/// One turn of the model.
#[derive(Clone)]
pub struct ModelRequest {
    /// The backend to call.
    pub backend: ModelBackend,
    /// The API key, when the backend needs one.
    pub api_key: Option<Secret>,
    /// The prompt.
    pub prompt: String,
}

/// One model call.
pub trait ModelClient: Send + Sync + 'static {
    /// Runs one turn and gives the answer.
    ///
    /// # Errors
    /// The error of the provider call.
    fn complete(&self, ctx: Ctx, request: ModelRequest) -> BoxFuture<'_, SandboxResult<Vec<u8>>>;
}

/// The logic inside the model slave Durable Object.
pub struct ModelSlave {
    ctx: Ctx,
    storage: Arc<dyn DoStorage>,
    client: Arc<dyn ModelClient>,
    config: ModelConfig,
    chunk_bytes: usize,
}

/// The storage key of the prepared flag.
const PREPARED_KEY: &str = "prepared";
/// The storage key prefix of the file table.
const FILE_PREFIX: &str = "files/";

impl ModelSlave {
    /// A model slave over its store, client and configuration.
    ///
    /// `chunk_bytes` is the largest file-read chunk.
    #[must_use]
    pub fn new(
        ctx: Ctx,
        storage: Arc<dyn DoStorage>,
        client: Arc<dyn ModelClient>,
        config: ModelConfig,
        chunk_bytes: usize,
    ) -> Self {
        Self {
            ctx,
            storage,
            client,
            config,
            chunk_bytes: chunk_bytes.max(1),
        }
    }

    /// The configuration of this slave.
    #[must_use]
    pub fn config(&self) -> &ModelConfig {
        &self.config
    }

    /// Handles one request to the slave Durable Object.
    pub async fn handle(&self, request: HttpRequestParts) -> HttpResponseParts {
        let mut router = foundation_connectrpc::Router::new();
        crate::shared::rpc::register_slave_service(&mut router, Arc::new(self.clone_for_router()));
        let handler = router.into_handler();
        handler.dispatch(self.ctx.clone(), request).await
    }

    /// A reference that the router can own.
    ///
    /// The router needs an `Arc`, and the slave is not `Clone` because its
    /// store and client are trait objects that share state. The parts are cheap
    /// to clone, so one more `Arc` of the same parts serves the router.
    fn clone_for_router(&self) -> Self {
        Self {
            ctx: self.ctx.clone(),
            storage: Arc::clone(&self.storage),
            client: Arc::clone(&self.client),
            config: self.config.clone(),
            chunk_bytes: self.chunk_bytes,
        }
    }

    /// One turn for one program.
    async fn turn(&self, request: ExecRequest) -> SandboxResult<Vec<u8>> {
        let prompt = request.argv.join(" ");
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
        let Some(backend) = self.config.backend.clone() else {
            return Err(SandboxError::UnsupportedCapability {
                capability: "model backend".to_string(),
            }
            .into());
        };
        let api_key = match (&backend, &self.config.api_key_entry) {
            (ModelBackend::OpenRouter { .. }, Some(entry)) => {
                let secrets = self
                    .ctx
                    .get::<ModelSecrets>()
                    .ok_or_else(|| ErrorTrace::new(SandboxError::HostNotConfigured))?;
                let key = secrets
                    .get(entry)
                    .ok_or_else(|| ErrorTrace::new(SandboxError::HostNotConfigured))?;
                Some(key.clone())
            }
            (ModelBackend::OpenRouter { .. }, None) => {
                return Err(SandboxError::HostNotConfigured.into());
            }
            (ModelBackend::CloudflareAi { .. }, _) => None,
        };
        let turn = self.client.complete(
            self.ctx.clone(),
            ModelRequest {
                backend,
                api_key,
                prompt,
            },
        );
        let deadline = self.ctx.clock().now() + self.config.turn_timeout;
        let timer = wait_until(deadline, &self.ctx);
        match select(Box::pin(turn), Box::pin(timer)).await {
            Either::Left((result, _timer)) => {
                let answer = result?;
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
            Either::Right((_timer, _turn)) => Err(SandboxError::Rpc {
                code: "deadline_exceeded".to_string(),
                message: format!(
                    "the model turn passed its deadline of {} ms",
                    self.config.turn_timeout.as_millis()
                ),
            }
            .into()),
        }
    }

    /// The storage key of one file path.
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] when the path is empty, absolute, or holds
    /// `..`.
    fn file_key(path: &str) -> SandboxResult<String> {
        let valid = !path.is_empty()
            && !path.starts_with('/')
            && !path.contains('\\')
            && !path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..");
        if !valid {
            return Err(SandboxError::InvalidSpec {
                field: "path".to_string(),
                reason: "the path must be relative and must not hold `..`".to_string(),
            }
            .into());
        }
        Ok(format!("{FILE_PREFIX}{path}"))
    }

    async fn prepared(&self) -> bool {
        self.storage
            .get(PREPARED_KEY)
            .await
            .ok()
            .flatten()
            .is_some_and(|value| value == b"1")
    }

    async fn read_file(&self, path: &str) -> SandboxResult<Vec<u8>> {
        let key = Self::file_key(path)?;
        let value = self.storage.get(&key).await?;
        value.ok_or_else(|| {
            ErrorTrace::new(SandboxError::NotFound {
                what: format!("file {path}"),
            })
        })
    }
}

#[allow(
    clippy::unused_async_trait_impl,
    reason = "the service trait declares async methods; two bodies answer from storage"
)]
impl SlaveService for ModelSlave {
    async fn info(
        &self,
        _ctx: RpcCtx,
        _req: Request<InfoRequest>,
    ) -> ConnectResult<Response<SlaveInfo>> {
        Ok(Response::new(SlaveInfo {
            slave_version: "model-slave".to_string(),
            arch: crate::shared::spec::GuestArch::host().as_str().to_string(),
            kernel: "model".to_string(),
            is_pid1: false,
            prepared: self.prepared().await,
            can_control: false,
        }))
    }

    async fn prepare(
        &self,
        _ctx: RpcCtx,
        req: Request<PrepareRequest>,
    ) -> ConnectResult<Response<PrepareResponse>> {
        self.storage
            .put(PREPARED_KEY, b"1".to_vec())
            .await
            .map_err(connect_error)?;
        Ok(Response::new(PrepareResponse {
            root_mode: req.msg.root.mode,
        }))
    }

    async fn exec(
        &self,
        _ctx: RpcCtx,
        req: Request<ExecRequest>,
    ) -> ConnectResult<
        std::pin::Pin<
            Box<dyn futures::Stream<Item = ConnectResult<ExecEventMsg>> + Send + 'static>,
        >,
    > {
        let answer = self.turn(req.msg).await;
        let events = match answer {
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
            Err(error) => vec![Err(ErrorTrace::new(
                error.current_context().to_connect(64 * 1024),
            ))],
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }

    async fn exec_input(
        &self,
        _ctx: RpcCtx,
        _req: Request<ExecInputRequest>,
    ) -> ConnectResult<Response<ExecInputResponse>> {
        Err(unsupported("exec input"))
    }

    async fn exec_signal(
        &self,
        _ctx: RpcCtx,
        _req: Request<ExecSignalRequest>,
    ) -> ConnectResult<Response<Empty>> {
        Err(unsupported("exec signal"))
    }

    async fn file_write_begin(
        &self,
        _ctx: RpcCtx,
        req: Request<FileWriteBeginRequest>,
    ) -> ConnectResult<Response<FileWriteBeginResponse>> {
        let key = Self::file_key(&req.msg.path).map_err(connect_error)?;
        self.storage
            .put(&key, Vec::new())
            .await
            .map_err(connect_error)?;
        Ok(Response::new(FileWriteBeginResponse {
            upload_id: req.msg.path,
        }))
    }

    async fn file_write_chunk(
        &self,
        _ctx: RpcCtx,
        req: Request<FileWriteChunkRequest>,
    ) -> ConnectResult<Response<FileWriteChunkResponse>> {
        let key = Self::file_key(&req.msg.upload_id).map_err(connect_error)?;
        let mut data = self
            .storage
            .get(&key)
            .await
            .map_err(connect_error)?
            .unwrap_or_default();
        if req.msg.offset != u64::try_from(data.len()).unwrap_or(u64::MAX) {
            return Err(connect_error(ErrorTrace::new(SandboxError::SequenceGap)));
        }
        data.extend_from_slice(&req.msg.data);
        self.storage
            .put(&key, data.clone())
            .await
            .map_err(connect_error)?;
        Ok(Response::new(FileWriteChunkResponse {
            next_offset: u64::try_from(data.len()).unwrap_or(u64::MAX),
        }))
    }

    async fn file_write_commit(
        &self,
        _ctx: RpcCtx,
        req: Request<FileWriteCommitRequest>,
    ) -> ConnectResult<Response<FileWriteCommitResponse>> {
        let key = Self::file_key(&req.msg.upload_id).map_err(connect_error)?;
        let data = self
            .storage
            .get(&key)
            .await
            .map_err(connect_error)?
            .unwrap_or_default();
        let size = u64::try_from(data.len()).unwrap_or(u64::MAX);
        Ok(Response::new(FileWriteCommitResponse { size }))
    }

    async fn file_write_abort(
        &self,
        _ctx: RpcCtx,
        req: Request<FileWriteAbortRequest>,
    ) -> ConnectResult<Response<Empty>> {
        let key = Self::file_key(&req.msg.upload_id).map_err(connect_error)?;
        self.storage.delete(&key).await.map_err(connect_error)?;
        Ok(Response::new(Empty {}))
    }

    async fn file_read(
        &self,
        _ctx: RpcCtx,
        req: Request<FileReadRequest>,
    ) -> ConnectResult<
        std::pin::Pin<Box<dyn futures::Stream<Item = ConnectResult<FileChunk>> + Send + 'static>>,
    > {
        let data = self.read_file(&req.msg.path).await.map_err(connect_error)?;
        let chunk = self.chunk_bytes;
        let mut chunks = Vec::new();
        let mut offset = 0_usize;
        if data.is_empty() {
            chunks.push(Ok(FileChunk {
                offset: 0,
                data: Vec::new(),
                eof: true,
            }));
        }
        while offset < data.len() {
            let end = (offset + chunk).min(data.len());
            let piece = data[offset..end].to_vec();
            let eof = end == data.len();
            chunks.push(Ok(FileChunk {
                offset: u64::try_from(offset).unwrap_or(u64::MAX),
                data: piece,
                eof,
            }));
            offset = end;
        }
        Ok(Box::pin(futures::stream::iter(chunks)))
    }

    async fn shutdown(
        &self,
        _ctx: RpcCtx,
        _req: Request<ShutdownRequest>,
    ) -> ConnectResult<Response<Empty>> {
        self.storage
            .delete(PREPARED_KEY)
            .await
            .map_err(connect_error)?;
        Ok(Response::new(Empty {}))
    }
}

/// One error of a service call, as a Connect error.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the call sites pass the trace from map_err"
)]
fn connect_error(
    error: ErrorTrace<SandboxError>,
) -> ErrorTrace<foundation_connectrpc::ConnectError> {
    ErrorTrace::new(error.current_context().to_connect(64 * 1024))
}

/// The error of a call that the model slave does not serve.
fn unsupported(capability: &str) -> ErrorTrace<foundation_connectrpc::ConnectError> {
    connect_error(
        SandboxError::UnsupportedCapability {
            capability: capability.to_string(),
        }
        .into(),
    )
}
