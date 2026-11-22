//! Shared test doubles and helpers.
//!
//! The installer reaches every side effect through a trait, so these fakes
//! replace the HTTP client, the process runner and the Docker API. They keep
//! every test off the network and off a real daemon.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use foundation_context::{Ctx, MonotonicClock};
use foundation_deployment_docker::client::ContainerCreateBody;
use foundation_deployment_docker::generated::create::ContainerCreateResponse;
use foundation_deployment_docker::generated::json::{
    ContainerInspectResponse, ContainerState, ImageInspect,
};
use foundation_deployment_docker::DockerError;
use foundation_installer::{
    manifest::{DeploySpec, Entry, EntryName, SourceSpec},
    CommandError, CommandOutput, CommandRunner, DockerApi, InstallContext, InstallerConfig,
};
use foundation_netio::shared::client::http_client::{
    BoxedSseFutureStream, BoxedSseIterator, HttpClient,
};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::client::HttpExchangeClientTask;
use foundation_netio::shared::http::{
    HttpClientError, SendSafeBody, SimpleHeaders, SimpleResponse, Status,
};
use serde_json::Value;

/// A context with a real monotonic clock and a fixed seed.
#[must_use]
pub fn test_ctx() -> Ctx {
    Ctx::fixed(Arc::new(MonotonicClock::new()))
}

/// A configuration whose paths are under `root` and whose timeouts are short.
#[must_use]
pub fn test_config(root: &Path) -> InstallerConfig {
    InstallerConfig {
        state_dir: root.join("state"),
        artifact_dir: root.join("artifacts"),
        install_root: root.join("opt"),
        systemd_unit_dir: root.join("units"),
        start_timeout: Duration::from_secs(1),
        start_poll_interval: Duration::from_millis(10),
        lock_timeout: Duration::from_millis(50),
        lock_poll_interval: Duration::from_millis(5),
        program_search_dirs: Vec::new(),
        ..InstallerConfig::default()
    }
}

/// An entry with the given source and deploy target and no registrar blocks.
#[must_use]
pub fn entry(name: &str, version: &str, source: SourceSpec, deploy: DeploySpec) -> Entry {
    Entry {
        name: EntryName::parse_at(name, "/entries/0/name").expect("a valid test name"),
        version: version.to_string(),
        source,
        deploy,
        proxy: None,
        probes: None,
    }
}

/// A `none` deploy target.
#[must_use]
pub fn deploy_none() -> DeploySpec {
    DeploySpec::None
}

/// The SHA-256 of `bytes` as lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut out = String::new();
    for byte in hasher.finalize() {
        use core::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// A context with every fake in place.
#[must_use]
pub fn fake_context(
    config: InstallerConfig,
    http: Arc<FakeHttpClient>,
    commands: Arc<FakeCommands>,
    docker: Arc<FakeDocker>,
) -> InstallContext {
    InstallContext::new(config, test_ctx())
        .with_http(http)
        .with_commands(commands)
        .with_docker(docker)
}

/// An HTTP client that answers every request with one status and one body and
/// records each URL.
pub struct FakeHttpClient {
    /// The status of every answer.
    pub status: Status,
    /// The body of every answer.
    pub body: Vec<u8>,
    /// Every URL that the code under test requested.
    pub seen: Mutex<Vec<String>>,
}

impl FakeHttpClient {
    /// A client that answers `200 OK` with `body`.
    #[must_use]
    pub fn ok(body: impl Into<Vec<u8>>) -> Arc<Self> {
        Arc::new(Self {
            status: Status::OK,
            body: body.into(),
            seen: Mutex::new(Vec::new()),
        })
    }

    /// A client that answers with `status` and `body`.
    #[must_use]
    pub fn with_status(status: Status, body: impl Into<Vec<u8>>) -> Arc<Self> {
        Arc::new(Self {
            status,
            body: body.into(),
            seen: Mutex::new(Vec::new()),
        })
    }

    /// The URLs that the code under test requested.
    #[must_use]
    pub fn seen(&self) -> Vec<String> {
        self.seen.lock().expect("the fake lock").clone()
    }

    fn answer(&self, request: &PreparedRequest) -> SimpleResponse<SendSafeBody> {
        self.seen
            .lock()
            .expect("the fake lock")
            .push(request.url.to_string());
        SimpleResponse::new(
            self.status.clone(),
            SimpleHeaders::new(),
            SendSafeBody::Bytes(self.body.clone()),
        )
    }
}

#[async_trait]
impl HttpClient for FakeHttpClient {
    async fn send_async(
        &self,
        request: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Ok(self.answer(&request))
    }

    async fn send_sse_async(
        &self,
        _request: PreparedRequest,
    ) -> Result<BoxedSseFutureStream, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn send(
        &self,
        request: PreparedRequest,
    ) -> Result<SimpleResponse<SendSafeBody>, HttpClientError> {
        Ok(self.answer(&request))
    }

    fn send_sse(&self, _request: PreparedRequest) -> Result<BoxedSseIterator, HttpClientError> {
        Err(HttpClientError::NotSupported)
    }

    fn open_exchange(&self, _request: PreparedRequest) -> HttpExchangeClientTask {
        unimplemented!("the fakes never open an exchange")
    }
}

#[derive(Debug)]
struct Response {
    program: String,
    contains: Vec<String>,
    output: CommandOutput,
}

/// A process runner that records every call and answers from a table.
pub struct FakeCommands {
    calls: Mutex<Vec<(String, Vec<String>)>>,
    responses: Mutex<Vec<Response>>,
}

impl FakeCommands {
    /// A runner that answers every call with exit code 0 and no output.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            responses: Mutex::new(Vec::new()),
        })
    }

    /// Answer a call to `program` whose argument list contains every item of
    /// `contains`. The first matching answer wins.
    pub fn respond(&self, program: &str, contains: &[&str], success: bool, stdout: &str) {
        self.responses
            .lock()
            .expect("the fake lock")
            .push(Response {
                program: program.to_string(),
                contains: contains.iter().map(|item| (*item).to_string()).collect(),
                output: CommandOutput {
                    status: i32::from(!success),
                    stdout: stdout.to_string(),
                    stderr: String::new(),
                },
            });
    }

    /// Every call, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<(String, Vec<String>)> {
        self.calls.lock().expect("the fake lock").clone()
    }
}

#[async_trait]
impl CommandRunner for FakeCommands {
    async fn run(&self, program: &str, args: &[String]) -> Result<CommandOutput, CommandError> {
        self.calls
            .lock()
            .expect("the fake lock")
            .push((program.to_string(), args.to_vec()));
        let found = self
            .responses
            .lock()
            .expect("the fake lock")
            .iter()
            .find(|response| {
                response.program == program
                    && response
                        .contains
                        .iter()
                        .all(|needle| args.iter().any(|arg| arg.contains(needle)))
            })
            .map(|response| response.output.clone());
        Ok(found.unwrap_or(CommandOutput {
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
        }))
    }
}

/// A Docker API that keeps containers in memory.
pub struct FakeDocker {
    digest: String,
    pulls: Mutex<Vec<(String, String)>>,
    created: Mutex<Vec<(String, Value)>>,
    /// Every key (id or name) that names a container, and its id.
    keys: Mutex<HashMap<String, String>>,
    /// The running flag of every container id.
    running: Mutex<HashMap<String, bool>>,
    renamed: Mutex<Vec<(String, String)>>,
    removed: Mutex<Vec<String>>,
    /// When true, `start_container` answers OK but the container never runs,
    /// so the caller reaches its start timeout.
    pub start_fails: bool,
}

impl FakeDocker {
    /// A daemon whose images report `digest`.
    #[must_use]
    pub fn new(digest: &str) -> Arc<Self> {
        Arc::new(Self {
            digest: digest.to_string(),
            pulls: Mutex::new(Vec::new()),
            created: Mutex::new(Vec::new()),
            keys: Mutex::new(HashMap::new()),
            running: Mutex::new(HashMap::new()),
            renamed: Mutex::new(Vec::new()),
            removed: Mutex::new(Vec::new()),
            start_fails: false,
        })
    }

    /// A daemon that starts no container.
    #[must_use]
    pub fn with_start_failure(digest: &str) -> Arc<Self> {
        Arc::new(Self {
            digest: digest.to_string(),
            pulls: Mutex::new(Vec::new()),
            created: Mutex::new(Vec::new()),
            keys: Mutex::new(HashMap::new()),
            running: Mutex::new(HashMap::new()),
            renamed: Mutex::new(Vec::new()),
            removed: Mutex::new(Vec::new()),
            start_fails: true,
        })
    }

    /// The pulls, as `(repository, tag)`.
    #[must_use]
    pub fn pulls(&self) -> Vec<(String, String)> {
        self.pulls.lock().expect("the fake lock").clone()
    }

    /// The created containers, as `(name, body)`.
    #[must_use]
    pub fn created(&self) -> Vec<(String, Value)> {
        self.created.lock().expect("the fake lock").clone()
    }

    /// The renames, as `(id, new name)`.
    #[must_use]
    pub fn renamed(&self) -> Vec<(String, String)> {
        self.renamed.lock().expect("the fake lock").clone()
    }

    /// The removed container keys.
    #[must_use]
    pub fn removed(&self) -> Vec<String> {
        self.removed.lock().expect("the fake lock").clone()
    }

    fn resolve(&self, key: &str) -> Option<String> {
        self.keys.lock().expect("the fake lock").get(key).cloned()
    }
}

#[async_trait]
impl DockerApi for FakeDocker {
    async fn image_pull(&self, repository: &str, tag: &str) -> Result<(), DockerError> {
        self.pulls
            .lock()
            .expect("the fake lock")
            .push((repository.to_string(), tag.to_string()));
        Ok(())
    }

    async fn image_inspect(
        &self,
        repository: &str,
        tag: &str,
    ) -> Result<ImageInspect, DockerError> {
        Ok(ImageInspect {
            repo_digests: Some(vec![format!(
                "{repository}:{tag}@{digest}",
                digest = self.digest
            )]),
            ..ImageInspect::default()
        })
    }

    async fn create_container(
        &self,
        body: &ContainerCreateBody,
        name: &str,
    ) -> Result<ContainerCreateResponse, DockerError> {
        let id = format!("id-{name}");
        self.keys
            .lock()
            .expect("the fake lock")
            .insert(name.to_string(), id.clone());
        self.keys
            .lock()
            .expect("the fake lock")
            .insert(id.clone(), id.clone());
        self.running
            .lock()
            .expect("the fake lock")
            .insert(id.clone(), false);
        let value = serde_json::to_value(body).expect("the create body serializes");
        self.created
            .lock()
            .expect("the fake lock")
            .push((name.to_string(), value));
        Ok(ContainerCreateResponse {
            id,
            warnings: Vec::new(),
        })
    }

    async fn start_container(&self, id: &str) -> Result<(), DockerError> {
        let id = self.resolve(id).ok_or_else(|| DockerError::Api {
            status: 404,
            message: "no such container".to_string(),
        })?;
        if !self.start_fails {
            self.running.lock().expect("the fake lock").insert(id, true);
        }
        Ok(())
    }

    async fn inspect_container(&self, id: &str) -> Result<ContainerInspectResponse, DockerError> {
        let id = self.resolve(id).ok_or_else(|| DockerError::Api {
            status: 404,
            message: "no such container".to_string(),
        })?;
        let running = self
            .running
            .lock()
            .expect("the fake lock")
            .get(&id)
            .copied()
            .unwrap_or(false);
        Ok(ContainerInspectResponse {
            state: Some(ContainerState {
                running: Some(running),
                ..ContainerState::default()
            }),
            ..ContainerInspectResponse::default()
        })
    }

    async fn stop_container(
        &self,
        id: &str,
        _timeout_secs: Option<u32>,
    ) -> Result<(), DockerError> {
        if let Some(id) = self.resolve(id) {
            self.running
                .lock()
                .expect("the fake lock")
                .insert(id, false);
        }
        Ok(())
    }

    async fn remove_container(&self, id: &str, _force: bool) -> Result<(), DockerError> {
        if let Some(id) = self.resolve(id) {
            self.keys
                .lock()
                .expect("the fake lock")
                .retain(|_key, value| *value != id);
            self.running.lock().expect("the fake lock").remove(&id);
        }
        self.removed
            .lock()
            .expect("the fake lock")
            .push(id.to_string());
        Ok(())
    }

    async fn rename_container(&self, id: &str, name: &str) -> Result<(), DockerError> {
        let id = self.resolve(id).ok_or_else(|| DockerError::Api {
            status: 404,
            message: "no such container".to_string(),
        })?;
        self.keys
            .lock()
            .expect("the fake lock")
            .insert(name.to_string(), id.clone());
        self.renamed
            .lock()
            .expect("the fake lock")
            .push((id, name.to_string()));
        Ok(())
    }
}
