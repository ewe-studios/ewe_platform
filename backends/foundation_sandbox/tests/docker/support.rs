//! A fake Docker Engine for the driver and container-control tests.
//!
//! The fake records every call and answers from a small in-memory state. It
//! never talks to a daemon, so the simulation tests run anywhere (feature 10,
//! section 7).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};

use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::io::readers::Data;
use foundation_deployment::providers::common::{ByteStream, JsonLinesStream};
use foundation_deployment_docker::client::body::{ByteBody, LogStream, ProgressStream};
use foundation_deployment_docker::client::containers_lifecycle::ContainerLogsOptions;
use foundation_deployment_docker::client::{ContainerCreateBody, DockerClient};
use foundation_deployment_docker::error::DockerError;
use foundation_deployment_docker::generated::create::{ContainerCreateResponse, CreateImageInfo};
use foundation_deployment_docker::generated::info::SystemInfo;
use foundation_deployment_docker::generated::json::{
    ContainerConfig, ContainerInspectResponse, ContainerState, ContainerSummary,
    ExecInspectResponse, HostConfig, ImageConfig, ImageInspect, NetworkSettings,
};
use foundation_deployment_docker::generated::shared::IDResponse;
use foundation_deployment_docker::generated::wait::ContainerWaitResponse;
use foundation_netio::shared::http::SendSafeBody;
use foundation_sandbox::native::docker::api::DockerApi;
use foundation_sandbox::shared::BoxFuture;
use serde_json::{json, Value};

/// One call that the fake Engine received.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    /// `POST /containers/create`.
    Create {
        /// The body, as JSON.
        body: Value,
        /// The container name.
        name: Option<String>,
    },
    /// `POST /containers/{id}/start`.
    Start(String),
    /// `POST /containers/{id}/stop`.
    Stop(String, Option<u32>),
    /// `DELETE /containers/{id}`.
    Remove(String, bool),
    /// `POST /containers/{id}/wait`.
    Wait(String),
    /// `GET /containers/{id}/json`.
    Inspect(String),
    /// `PUT /containers/{id}/archive`.
    Upload(String, String),
    /// `GET /containers/{id}/logs`.
    Logs(String),
    /// `POST /containers/{id}/kill`.
    Kill(String, Option<String>),
    /// `GET /containers/json`.
    List(bool, Option<String>),
    /// `POST /containers/{id}/exec`.
    ExecCreate(String, Vec<String>),
    /// `POST /exec/{id}/start`.
    ExecStart(String, bool),
    /// `GET /exec/{id}/json`.
    ExecInspect(String),
    /// `GET /images/{name}/json`.
    ImageInspect(String),
    /// `POST /images/create`.
    ImagePull(String),
    /// `GET /containers/{id}/export`.
    Export(String),
    /// `GET /info`.
    SystemInfo,
    /// `GET /_ping`.
    SystemPing,
}

/// One Engine operation that a test can make fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    SystemInfo,
    SystemPing,
    ImageInspect,
    ImagePull,
    Export,
    Create,
    Start,
    Upload,
    Inspect,
    Logs,
    Kill,
    Wait,
    Stop,
    Remove,
    List,
    ExecCreate,
    ExecStart,
    ExecInspect,
}

/// The failure that one operation gives.
#[derive(Clone, Debug)]
pub enum Failure {
    /// A daemon status.
    Status(u16, String),
    /// A pull that failed inside its stream.
    PullFailed(String),
    /// No daemon answered.
    Unreachable(String),
}

impl Failure {
    fn into_error(self) -> DockerError {
        match self {
            Self::Status(status, message) => DockerError::Api { status, message },
            Self::PullFailed(message) => DockerError::PullFailed(message),
            Self::Unreachable(message) => DockerError::Unavailable(message),
        }
    }
}

/// One container in the fake Engine.
#[derive(Clone, Debug, Default)]
pub struct FakeContainer {
    /// Whether the container runs.
    pub running: bool,
    /// The exit code of `wait`.
    pub exit_code: i64,
    /// The image of the container.
    pub image: String,
    /// The labels of the container.
    pub labels: BTreeMap<String, String>,
    /// The host port that the container publishes for `1024/tcp`.
    pub host_port: Option<u16>,
}

/// The body that `container_export` answers with.
#[derive(Clone, Debug)]
pub enum ExportBody {
    /// One fixed archive.
    Bytes(Vec<u8>),
    /// `count` chunks of `size` zero bytes, made as the stream is read.
    ///
    /// The feature 13 stream test uses 200 MiB and never holds it in memory.
    #[allow(
        dead_code,
        reason = "the image import tests of feature 13 construct this variant"
    )]
    Chunks {
        /// The chunk count.
        count: usize,
        /// The bytes in each chunk.
        size: usize,
    },
}

impl Default for ExportBody {
    fn default() -> Self {
        Self::Bytes(Vec::new())
    }
}

struct FakeState {
    calls: Vec<Call>,
    images: HashSet<String>,
    containers: HashMap<String, FakeContainer>,
    /// The container name to the container identifier.
    names: HashMap<String, String>,
    next_id: u64,
    failures: HashMap<Op, Failure>,
    log_frames: Vec<(u8, Vec<u8>)>,
    exec_frames: Vec<(u8, Vec<u8>)>,
    exec_exit: i32,
    arch: String,
    ncpu: i64,
    export: ExportBody,
    image_configs: HashMap<String, ImageConfig>,
}

/// A fake Docker Engine.
#[derive(Clone)]
pub struct FakeDockerApi {
    state: Arc<Mutex<FakeState>>,
}

#[allow(
    dead_code,
    reason = "the fake gives more controls than one test module uses"
)]
impl FakeDockerApi {
    /// A fake with no image and no container.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                calls: Vec::new(),
                images: HashSet::new(),
                containers: HashMap::new(),
                names: HashMap::new(),
                next_id: 0,
                failures: HashMap::new(),
                log_frames: Vec::new(),
                exec_frames: Vec::new(),
                exec_exit: 0,
                arch: "x86_64".to_string(),
                ncpu: 8,
                export: ExportBody::default(),
                image_configs: HashMap::new(),
            })),
        }
    }

    /// The fake as the trait object that the driver holds.
    #[must_use]
    pub fn api(&self) -> Arc<dyn DockerApi> {
        Arc::new(self.clone())
    }

    /// Adds an image that the daemon already has.
    pub fn add_image(&self, name: &str) {
        self.with(|state| {
            state.images.insert(name.to_string());
        });
    }

    /// Adds a container under `name`.
    pub fn add_container(&self, name: &str, container: FakeContainer) {
        self.with(|state| {
            state.containers.insert(name.to_string(), container);
            state.names.insert(name.to_string(), name.to_string());
        });
    }

    /// Makes `op` fail with `failure`.
    pub fn fail(&self, op: Op, failure: Failure) {
        self.with(|state| {
            state.failures.insert(op, failure);
        });
    }

    /// Sets the stdout and stderr frames of the container log.
    pub fn set_log_frames(&self, frames: Vec<(u8, Vec<u8>)>) {
        self.with(|state| state.log_frames = frames);
    }

    /// Sets the stdout and stderr frames of one exec.
    pub fn set_exec_frames(&self, frames: Vec<(u8, Vec<u8>)>) {
        self.with(|state| state.exec_frames = frames);
    }

    /// Sets the exit code that `exec_inspect` answers.
    pub fn set_exec_exit(&self, code: i32) {
        self.with(|state| state.exec_exit = code);
    }

    /// Sets the daemon architecture.
    pub fn set_arch(&self, arch: &str) {
        self.with(|state| state.arch = arch.to_string());
    }

    /// Sets the archive that `container_export` answers with.
    pub fn set_export(&self, export: ExportBody) {
        self.with(|state| state.export = export);
    }

    /// Sets the container configuration that `image_inspect` answers.
    pub fn set_image_config(&self, name: &str, config: ImageConfig) {
        self.with(|state| {
            state.image_configs.insert(name.to_string(), config);
        });
    }

    /// The calls that the fake received.
    #[must_use]
    pub fn calls(&self) -> Vec<Call> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .calls
            .clone()
    }

    /// How often an operation was called.
    #[must_use]
    pub fn count(&self, wanted: impl Fn(&Call) -> bool) -> usize {
        self.calls().iter().filter(|call| wanted(call)).count()
    }

    /// Whether a container with `name` still exists.
    #[must_use]
    pub fn has_container(&self, name: &str) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.containers.contains_key(name) || state.names.contains_key(name)
    }

    fn with(&self, action: impl FnOnce(&mut FakeState)) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        action(&mut state);
    }

    fn record(&self, call: Call) {
        self.with(|state| state.calls.push(call));
    }

    fn failure(&self, op: Op) -> Option<DockerError> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .failures
            .get(&op)
            .cloned()
            .map(Failure::into_error)
    }

    fn find(&self, id: &str) -> Option<(String, FakeContainer)> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(container) = state.containers.get(id) {
            return Some((id.to_string(), container.clone()));
        }
        let key = state.names.get(id)?;
        state
            .containers
            .get(key)
            .map(|container| (key.clone(), container.clone()))
    }
}

impl Default for FakeDockerApi {
    fn default() -> Self {
        Self::new()
    }
}

impl DockerApi for FakeDockerApi {
    fn create_container<'a>(
        &'a self,
        config: &'a ContainerCreateBody,
        name: Option<&'a str>,
    ) -> BoxFuture<'a, Result<ContainerCreateResponse, DockerError>> {
        self.record(Call::Create {
            body: serde_json::to_value(config).unwrap_or(Value::Null),
            name: name.map(ToString::to_string),
        });
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Create) {
                return Err(error);
            }
            let id = {
                let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                state.next_id += 1;
                let id = format!("{:064x}", state.next_id);
                let image = config.config.image.clone().unwrap_or_default();
                let labels = config
                    .config
                    .labels
                    .as_ref()
                    .and_then(serde_json::Value::as_object)
                    .map_or_else(BTreeMap::new, |labels| {
                        labels
                            .iter()
                            .filter_map(|(key, value)| {
                                value.as_str().map(|value| (key.clone(), value.to_string()))
                            })
                            .collect()
                    });
                let container_name = name.map_or_else(|| id.clone(), ToString::to_string);
                state.containers.insert(
                    id.clone(),
                    FakeContainer {
                        running: false,
                        exit_code: 0,
                        image,
                        labels,
                        host_port: None,
                    },
                );
                state.names.insert(container_name, id.clone());
                id
            };
            Ok(ContainerCreateResponse {
                id,
                warnings: Vec::new(),
            })
        })
    }

    fn start_container<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<(), DockerError>> {
        self.record(Call::Start(id.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Start) {
                return Err(error);
            }
            match self.find(id) {
                Some((name, mut container)) => {
                    container.running = true;
                    self.with(|state| {
                        state.containers.insert(name, container);
                    });
                    Ok(())
                }
                None => Err(DockerError::Api {
                    status: 404,
                    message: format!("no such container: {id}"),
                }),
            }
        })
    }

    fn wait_container<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ContainerWaitResponse, DockerError>> {
        self.record(Call::Wait(id.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Wait) {
                return Err(error);
            }
            match self.find(id) {
                Some((name, mut container)) => {
                    container.running = false;
                    let status_code = container.exit_code;
                    self.with(|state| {
                        state.containers.insert(name, container);
                    });
                    Ok(ContainerWaitResponse {
                        error: None,
                        status_code,
                    })
                }
                None => Err(DockerError::Api {
                    status: 404,
                    message: format!("no such container: {id}"),
                }),
            }
        })
    }

    fn stop_container<'a>(
        &'a self,
        id: &'a str,
        timeout_secs: Option<u32>,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        self.record(Call::Stop(id.to_string(), timeout_secs));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Stop) {
                return Err(error);
            }
            if let Some((name, mut container)) = self.find(id) {
                container.running = false;
                self.with(|state| {
                    state.containers.insert(name, container);
                });
            }
            Ok(())
        })
    }

    fn remove_container<'a>(
        &'a self,
        id: &'a str,
        force: bool,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        self.record(Call::Remove(id.to_string(), force));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Remove) {
                return Err(error);
            }
            let removed = {
                let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                let key = if state.containers.contains_key(id) {
                    Some(id.to_string())
                } else {
                    state.names.get(id).cloned()
                };
                key.is_some_and(|key| {
                    state.names.retain(|_, value| value != &key);
                    state.names.remove(id);
                    state.containers.remove(&key).is_some()
                })
            };
            if removed {
                Ok(())
            } else {
                Err(DockerError::Api {
                    status: 404,
                    message: format!("no such container: {id}"),
                })
            }
        })
    }

    fn inspect_container<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ContainerInspectResponse, DockerError>> {
        self.record(Call::Inspect(id.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Inspect) {
                return Err(error);
            }
            match self.find(id) {
                Some((name, container)) => Ok(inspect_of(&name, &container)),
                None => Err(DockerError::Api {
                    status: 404,
                    message: format!("no such container: {id}"),
                }),
            }
        })
    }

    fn upload_to_container<'a>(
        &'a self,
        id: &'a str,
        path: &'a str,
        _tar: SendSafeBody,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        self.record(Call::Upload(id.to_string(), path.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Upload) {
                return Err(error);
            }
            Ok(())
        })
    }

    fn container_logs<'a>(
        &'a self,
        id: &'a str,
        _options: &'a ContainerLogsOptions,
    ) -> BoxFuture<'a, Result<LogStream, DockerError>> {
        self.record(Call::Logs(id.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Logs) {
                return Err(error);
            }
            let frames = self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .log_frames
                .clone();
            Ok(log_stream(&frames))
        })
    }

    fn kill_container<'a>(
        &'a self,
        id: &'a str,
        signal: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), DockerError>> {
        self.record(Call::Kill(id.to_string(), signal.map(ToString::to_string)));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Kill) {
                return Err(error);
            }
            if let Some((name, mut container)) = self.find(id) {
                container.running = false;
                self.with(|state| {
                    state.containers.insert(name, container);
                });
            }
            Ok(())
        })
    }

    fn container_list<'a>(
        &'a self,
        all: bool,
        filters: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Vec<ContainerSummary>, DockerError>> {
        self.record(Call::List(all, filters.map(ToString::to_string)));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::List) {
                return Err(error);
            }
            let want_control =
                filters.is_some_and(|filters| filters.contains("ewe.container.control"));
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let mut summaries = Vec::new();
            for (name, container) in &state.containers {
                if !all && !container.running {
                    continue;
                }
                if want_control
                    && container
                        .labels
                        .get("ewe.container.control")
                        .map(String::as_str)
                        != Some("true")
                {
                    continue;
                }
                summaries.push(ContainerSummary {
                    id: Some(name.clone()),
                    image: Some(container.image.clone()),
                    state: Some("running".to_string()),
                    labels: Some(json!(container.labels)),
                    ..ContainerSummary::default()
                });
            }
            Ok(summaries)
        })
    }

    fn exec_create<'a>(
        &'a self,
        id: &'a str,
        cmd: &'a [&'a str],
    ) -> BoxFuture<'a, Result<IDResponse, DockerError>> {
        self.record(Call::ExecCreate(
            id.to_string(),
            cmd.iter().map(|part| (*part).to_string()).collect(),
        ));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::ExecCreate) {
                return Err(error);
            }
            Ok(IDResponse {
                id: "exec-1".to_string(),
            })
        })
    }

    fn exec_start<'a>(
        &'a self,
        id: &'a str,
        detach: bool,
    ) -> BoxFuture<'a, Result<LogStream, DockerError>> {
        self.record(Call::ExecStart(id.to_string(), detach));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::ExecStart) {
                return Err(error);
            }
            let frames = self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .exec_frames
                .clone();
            Ok(log_stream(&frames))
        })
    }

    fn exec_inspect<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Result<ExecInspectResponse, DockerError>> {
        self.record(Call::ExecInspect(id.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::ExecInspect) {
                return Err(error);
            }
            let exit = self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .exec_exit;
            Ok(ExecInspectResponse {
                exit_code: Some(i64::from(exit)),
                pid: Some(42),
                ..ExecInspectResponse::default()
            })
        })
    }

    fn image_inspect<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<ImageInspect, DockerError>> {
        self.record(Call::ImageInspect(name.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::ImageInspect) {
                return Err(error);
            }
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.images.contains(name) {
                Ok(ImageInspect {
                    architecture: Some(state.arch.clone()),
                    repo_tags: Some(vec![name.to_string()]),
                    id: Some(format!("sha256:{}", name.replace(':', "-"))),
                    config: state.image_configs.get(name).cloned(),
                    ..ImageInspect::default()
                })
            } else {
                Err(DockerError::Api {
                    status: 404,
                    message: format!("no such image: {name}"),
                })
            }
        })
    }

    fn image_pull<'a>(
        &'a self,
        name: &'a str,
    ) -> BoxFuture<'a, Result<ProgressStream<CreateImageInfo>, DockerError>> {
        self.record(Call::ImagePull(name.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::ImagePull) {
                return Err(error);
            }
            {
                let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
                state.images.insert(name.to_string());
            }
            let lines = JsonLinesStream::new(
                SendSafeBody::Bytes(b"{\"status\":\"Pulling\"}\n".to_vec()),
                1024 * 1024,
            );
            Ok(ProgressStream::new(lines, DockerError::PullFailed))
        })
    }

    fn container_export<'a>(&'a self, id: &'a str) -> BoxFuture<'a, Result<ByteBody, DockerError>> {
        self.record(Call::Export(id.to_string()));
        Box::pin(async move {
            if let Some(error) = self.failure(Op::Export) {
                return Err(error);
            }
            let export = self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .export
                .clone();
            Ok(export_body(&export))
        })
    }

    fn system_info(&self) -> BoxFuture<'_, Result<SystemInfo, DockerError>> {
        self.record(Call::SystemInfo);
        Box::pin(async move {
            if let Some(error) = self.failure(Op::SystemInfo) {
                return Err(error);
            }
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            Ok(SystemInfo {
                architecture: Some(state.arch.clone()),
                ncpu: Some(state.ncpu),
                ..SystemInfo::default()
            })
        })
    }

    fn system_ping(&self) -> BoxFuture<'_, Result<(), DockerError>> {
        self.record(Call::SystemPing);
        Box::pin(async move {
            if let Some(error) = self.failure(Op::SystemPing) {
                return Err(error);
            }
            Ok(())
        })
    }
}

/// The body that `container_export` answers with.
fn export_body(export: &ExportBody) -> ByteBody {
    match export {
        ExportBody::Bytes(bytes) => {
            ByteBody::new(ByteStream::new(SendSafeBody::Bytes(bytes.clone())))
        }
        ExportBody::Chunks { count, size } => ByteBody::new(ByteStream::new(SendSafeBody::Stream(
            Some(Box::new(ChunkIterator {
                remaining: *count,
                size: *size,
            })),
        ))),
    }
}

/// A lazy iterator of zero-filled export chunks.
struct ChunkIterator {
    remaining: usize,
    size: usize,
}

impl Iterator for ChunkIterator {
    type Item = Result<Data, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some(Ok(Data::Bytes(vec![0_u8; self.size])))
    }
}

/// The inspect answer of one fake container.
fn inspect_of(name: &str, container: &FakeContainer) -> ContainerInspectResponse {
    let ports = container.host_port.map(|port| {
        json!({
            "1024/tcp": [{
                "HostIp": "127.0.0.1",
                "HostPort": port.to_string(),
            }]
        })
    });
    ContainerInspectResponse {
        id: Some(name.to_string()),
        name: Some(format!("/{name}")),
        image: Some(container.image.clone()),
        state: Some(ContainerState {
            running: Some(container.running),
            exit_code: Some(container.exit_code),
            ..ContainerState::default()
        }),
        config: Some(ContainerConfig {
            image: Some(container.image.clone()),
            labels: Some(json!(container.labels)),
            ..ContainerConfig::default()
        }),
        network_settings: Some(NetworkSettings {
            ports,
            ..NetworkSettings::default()
        }),
        ..ContainerInspectResponse::default()
    }
}

/// A log stream over the given Docker multiplexed frames.
///
/// The frame kind is 1 for standard output and 2 for standard error.
fn log_stream(frames: &[(u8, Vec<u8>)]) -> LogStream {
    LogStream::new(
        ByteStream::new(SendSafeBody::Bytes(multiplexed(frames))),
        16 * 1024 * 1024,
    )
}

/// The Docker multiplexed encoding of frames.
#[must_use]
pub fn multiplexed(frames: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (kind, payload) in frames {
        bytes.push(*kind);
        bytes.extend_from_slice(&[0, 0, 0]);
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        bytes.extend_from_slice(payload);
    }
    bytes
}

/// A `DockerClient` value that the trait object tests can name.
///
/// It exists so the test binary links the client crate even when a test only
/// uses the fake; the real-engine tests use it.
#[allow(dead_code, reason = "the real-engine tests name the client type")]
pub type RealClient = DockerClient;

/// A `HostConfig` value that the create-body test reads.
#[allow(dead_code, reason = "the create-body test names the generated type")]
pub type CreateHostConfig = HostConfig;
