//! The container control of feature 00, section 6.13, on the Docker Engine.
//!
//! [`DockerContainerControl`] implements [`ContainerControl`] and
//! [`DockerContainerOps`] implements [`ContainerOps`] (feature 10, section
//! 6.7.1). A caller starts a container, runs commands in it, signals it and
//! stops it with no sandbox record. The driver and the control share one
//! [`DockerApi`].

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_deployment_docker::client::body::LogStream as DockerLogStream;
use foundation_deployment_docker::client::containers_lifecycle::ContainerLogsOptions;

use foundation_deployment_docker::generated::json::{
    ContainerConfig, ContainerInspectResponse, ContainerSummary, HostConfig, Resources,
};
use foundation_deployment_docker::generated::wait::ContainerWaitResponse;
use foundation_deployment_docker::streaming::decoder::LogOutput;
use serde_json::json;

use crate::native::docker::api::{
    create_body, is_not_found, map_error, DockerApi, LABEL_CONTAINER_CONTROL,
};
use crate::shared::command::{Command, ExecEvent, ExitStatus};
use crate::shared::container::{
    ContainerCapabilities, ContainerControl, ContainerHandle, ContainerId, ContainerInfo,
    ContainerOps, ContainerResources, ContainerSpec, ContainerStatus, PortRequest, PortResponse,
};
use crate::shared::driver::SubstrateExit;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::handles::{ExecEvents, ExecHandle, ExecHandleOps, ExecStdin};
use crate::shared::ids::{DriverId, ExecId};
use crate::shared::spec::NetworkMode;
use crate::shared::BoxFuture;

/// How many log lines the control reads for one error message.
///
/// Default 1000: enough to show the start failure of a container, and small
/// enough to read only a bounded part of a chatty log.
const LOG_TAIL_LINES: &str = "1000";

/// The container control on one Docker Engine.
pub struct DockerContainerControl {
    api: Arc<dyn DockerApi>,
}

impl DockerContainerControl {
    /// A control over `api`.
    #[must_use]
    pub fn new(api: Arc<dyn DockerApi>) -> Self {
        Self { api }
    }

    /// The Engine connection that this control uses.
    #[must_use]
    pub fn engine(&self) -> Arc<dyn DockerApi> {
        Arc::clone(&self.api)
    }
}

impl ContainerControl for DockerContainerControl {
    fn start(
        &self,
        ctx: Ctx,
        spec: ContainerSpec,
    ) -> BoxFuture<'_, SandboxResult<ContainerHandle>> {
        let api = Arc::clone(&self.api);
        let ops_ctx = ctx.clone();
        Box::pin(async move {
            if spec.resources.instance_type.is_some() {
                return Err(SandboxError::UnsupportedCapability {
                    capability: "instance_type".to_string(),
                }
                .into());
            }
            let body = control_body(&spec);
            let created = api.create_container(&body, None).await.map_err(map_error)?;
            if let Err(error) = api.start_container(&created.id).await {
                let _ = api.remove_container(&created.id, true).await;
                return Err(map_error(error).into());
            }
            let id = ContainerId::new(created.id.clone())?;
            let info = ContainerInfo {
                id: id.clone(),
                driver: DriverId::Docker,
                image: spec.image.clone(),
                status: ContainerStatus::Running,
                created_unix_ms: created_unix_ms(&ctx),
                resources: spec.resources.clone(),
            };
            let ops = Arc::new(DockerContainerOps {
                api,
                ctx: ops_ctx,
                info,
            });
            Ok(ContainerHandle::new(id, ops))
        })
    }

    fn get(
        &self,
        ctx: Ctx,
        id: &ContainerId,
    ) -> BoxFuture<'_, SandboxResult<Option<ContainerHandle>>> {
        let api = Arc::clone(&self.api);
        let id = id.clone();
        Box::pin(async move {
            let inspected = match api.inspect_container(id.as_str()).await {
                Ok(inspected) => inspected,
                Err(error) if is_not_found(&error) => return Ok(None),
                Err(error) => return Err(map_error(error).into()),
            };
            if !container_runs(&inspected) {
                return Ok(None);
            }
            let info = info_from_inspect(&ctx, &id, &inspected);
            let ops = Arc::new(DockerContainerOps { api, ctx, info });
            Ok(Some(ContainerHandle::new(id, ops)))
        })
    }

    fn list(&self, ctx: Ctx) -> BoxFuture<'_, SandboxResult<Vec<ContainerInfo>>> {
        let api = Arc::clone(&self.api);
        Box::pin(async move {
            let filters =
                json!({ "label": [format!("{LABEL_CONTAINER_CONTROL}=true")] }).to_string();
            let summaries = api
                .container_list(false, Some(&filters))
                .await
                .map_err(map_error)?;
            Ok(summaries
                .iter()
                .filter_map(|summary| info_from_summary(&ctx, summary))
                .collect())
        })
    }
}

/// The operations of one container.
pub struct DockerContainerOps {
    api: Arc<dyn DockerApi>,
    ctx: Ctx,
    info: ContainerInfo,
}

impl DockerContainerOps {
    /// The operations of one container that this control knows.
    #[must_use]
    pub fn new(api: Arc<dyn DockerApi>, ctx: Ctx, info: ContainerInfo) -> Self {
        Self { api, ctx, info }
    }
}

impl ContainerOps for DockerContainerOps {
    fn info(&self) -> ContainerInfo {
        self.info.clone()
    }

    fn capabilities(&self) -> ContainerCapabilities {
        ContainerCapabilities {
            exec: true,
            port_fetch: false,
            signal: true,
            logs: true,
        }
    }

    fn fetch(
        &self,
        _port: u16,
        _request: PortRequest,
    ) -> BoxFuture<'_, SandboxResult<PortResponse>> {
        Box::pin(async {
            Err(SandboxError::UnsupportedCapability {
                capability: "port_fetch".to_string(),
            }
            .into())
        })
    }

    fn exec(&self, command: Command) -> BoxFuture<'_, SandboxResult<ExecHandle>> {
        let api = Arc::clone(&self.api);
        let id = self.info.id.clone();
        let ctx = self.ctx.clone();
        Box::pin(async move {
            let argv: Vec<&str> = command.argv.iter().map(String::as_str).collect();
            let created = api
                .exec_create(id.as_str(), &argv)
                .await
                .map_err(map_error)?;
            let stream = api
                .exec_start(&created.id, false)
                .await
                .map_err(map_error)?;
            let exec_id = ExecId::generate(&ctx);
            let ops = Arc::new(DockerExecOps {
                api,
                docker_id: created.id,
                stream: Mutex::new(Some(stream)),
                exit: Mutex::new(None),
            });
            Ok(ExecHandle::new(exec_id, ops))
        })
    }

    fn signal(&self, signo: i32) -> BoxFuture<'_, SandboxResult<()>> {
        let api = Arc::clone(&self.api);
        let id = self.info.id.clone();
        Box::pin(async move {
            api.kill_container(id.as_str(), Some(&signo.to_string()))
                .await
                .map_err(|error| map_error(error).into())
        })
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let api = Arc::clone(&self.api);
        let id = self.info.id.clone();
        Box::pin(async move {
            let response = api.wait_container(id.as_str()).await.map_err(map_error)?;
            Ok(exit_of(&response))
        })
    }

    fn stop(&self, grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let api = Arc::clone(&self.api);
        let id = self.info.id.clone();
        Box::pin(async move {
            api.stop_container(id.as_str(), Some(grace_secs(grace)))
                .await
                .map_err(map_error)?;
            let response = api.wait_container(id.as_str()).await.map_err(map_error)?;
            Ok(exit_of(&response))
        })
    }

    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>> {
        let api = Arc::clone(&self.api);
        let id = self.info.id.clone();
        let ctx = self.ctx.clone();
        Box::pin(async move {
            match api.remove_container(id.as_str(), true).await {
                Ok(()) => {}
                Err(error) if is_not_found(&error) => {}
                Err(error) => return Err(map_error(error).into()),
            }
            remove_run_dir(&ctx, id.as_str())
        })
    }

    fn log_tail(&self, max: usize) -> BoxFuture<'_, SandboxResult<Vec<u8>>> {
        let api = Arc::clone(&self.api);
        let id = self.info.id.clone();
        Box::pin(async move { Ok(read_log_tail(&*api, id.as_str(), max).await) })
    }
}

/// The operations behind one Docker exec.
struct DockerExecOps {
    api: Arc<dyn DockerApi>,
    docker_id: String,
    stream: Mutex<Option<DockerLogStream>>,
    exit: Mutex<Option<ExitStatus>>,
}

impl DockerExecOps {
    fn take_stream(&self) -> Option<DockerLogStream> {
        self.stream
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// The event stream: the frames of the exec output, then one `Exit`.
    fn event_stream(&self) -> ExecEvents {
        let api = Arc::clone(&self.api);
        let docker_id = self.docker_id.clone();
        let stream = self.take_stream();
        let events = futures::stream::unfold(
            (stream, api, docker_id, false),
            |(stream, api, docker_id, done)| async move {
                if done {
                    return None;
                }
                let mut stream = stream?;
                loop {
                    match stream.next().await {
                        Some(Ok(
                            LogOutput::StdOut { message } | LogOutput::Console { message },
                        )) => {
                            return Some((
                                Ok(ExecEvent::Stdout(message.to_vec())),
                                (Some(stream), api, docker_id, false),
                            ));
                        }
                        Some(Ok(LogOutput::StdErr { message })) => {
                            return Some((
                                Ok(ExecEvent::Stderr(message.to_vec())),
                                (Some(stream), api, docker_id, false),
                            ));
                        }
                        Some(Ok(LogOutput::StdIn { .. })) => {}
                        Some(Err(error)) => {
                            return Some((
                                Err(map_error(error).into()),
                                (None, api, docker_id, true),
                            ));
                        }
                        None => {
                            // The output ended: read the exit code of the exec.
                            let exit = match api.exec_inspect(&docker_id).await {
                                Ok(inspected) => inspected
                                    .exit_code
                                    .and_then(|code| i32::try_from(code).ok())
                                    .map_or(ExitStatus::code(0), ExitStatus::code),
                                Err(_) => ExitStatus::code(0),
                            };
                            return Some((Ok(ExecEvent::Exit(exit)), (None, api, docker_id, true)));
                        }
                    }
                }
            },
        );
        ExecEvents::new(Box::pin(events))
    }
}

impl ExecHandleOps for DockerExecOps {
    fn pid(&self) -> u32 {
        0
    }

    fn stdin(&self) -> Option<ExecStdin> {
        None
    }

    fn events(&self) -> ExecEvents {
        self.event_stream()
    }

    fn signal(&self, _signal: i32) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            Err(SandboxError::UnsupportedCapability {
                capability: "exec_signal".to_string(),
            }
            .into())
        })
    }

    fn wait(&self) -> BoxFuture<'_, SandboxResult<ExitStatus>> {
        let mut events = self.event_stream();
        Box::pin(async move {
            let mut status = ExitStatus::code(0);
            while let Some(event) = futures::StreamExt::next(&mut events).await {
                match event? {
                    ExecEvent::Exit(exit) => {
                        status = exit;
                        break;
                    }
                    ExecEvent::Stdout(_) | ExecEvent::Stderr(_) => {}
                }
            }
            *self.exit.lock().unwrap_or_else(PoisonError::into_inner) = Some(status);
            Ok(status)
        })
    }
}

/// The create body of a container control start.
fn control_body(spec: &ContainerSpec) -> foundation_deployment_docker::client::ContainerCreateBody {
    let mut labels: Vec<(String, String)> = spec.labels.clone();
    labels.push((LABEL_CONTAINER_CONTROL.to_string(), "true".to_string()));
    let config = ContainerConfig {
        image: Some(spec.image.clone()),
        entrypoint: if spec.entrypoint.is_empty() {
            None
        } else {
            Some(spec.entrypoint.clone())
        },
        env: if spec.env.is_empty() {
            None
        } else {
            Some(
                spec.env
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect(),
            )
        },
        labels: Some(json!(labels
            .into_iter()
            .collect::<std::collections::BTreeMap<String, String>>())),
        ..ContainerConfig::default()
    };
    let host = HostConfig {
        network_mode: Some(
            match spec.network {
                NetworkMode::Host => "bridge",
                NetworkMode::None => "none",
            }
            .to_string(),
        ),
        security_opt: Some(vec!["no-new-privileges".to_string()]),
        privileged: Some(false),
        auto_remove: Some(false),
        ..HostConfig::default()
    };
    let resources = Resources {
        memory: spec
            .resources
            .memory_mib
            .map(|mib| i64::from(mib) * 1_048_576),
        nano_cpus: spec
            .resources
            .vcpus
            .map(|vcpus| i64::from(vcpus) * 1_000_000_000),
        ..Resources::default()
    };
    create_body(config, host, resources)
}

/// Whether an inspect answer says that the container runs.
fn container_runs(inspected: &ContainerInspectResponse) -> bool {
    inspected
        .state
        .as_ref()
        .and_then(|state| state.running)
        .unwrap_or(false)
}

/// The container info of one inspect answer.
fn info_from_inspect(
    ctx: &Ctx,
    id: &ContainerId,
    inspected: &ContainerInspectResponse,
) -> ContainerInfo {
    ContainerInfo {
        id: id.clone(),
        driver: DriverId::Docker,
        image: inspected
            .config
            .as_ref()
            .and_then(|config| config.image.clone())
            .unwrap_or_default(),
        status: ContainerStatus::Running,
        created_unix_ms: created_unix_ms(ctx),
        resources: ContainerResources::default(),
    }
}

/// The container info of one list entry, or `None` for an entry without an ID.
fn info_from_summary(ctx: &Ctx, summary: &ContainerSummary) -> Option<ContainerInfo> {
    let id = ContainerId::new(summary.id.clone()?).ok()?;
    let created_unix_ms = summary
        .created
        .and_then(|seconds| u64::try_from(seconds).ok())
        .map_or_else(|| created_unix_ms(ctx), |seconds| seconds * 1000);
    Some(ContainerInfo {
        id,
        driver: DriverId::Docker,
        image: summary.image.clone().unwrap_or_default(),
        status: ContainerStatus::Running,
        created_unix_ms,
        resources: ContainerResources::default(),
    })
}

/// The creation time as Unix milliseconds from the injected clock.
fn created_unix_ms(ctx: &Ctx) -> u64 {
    let wall = ctx.clock().wall_now();
    wall.duration_since(foundation_compact::SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

/// The stop grace in whole seconds, at least one.
fn grace_secs(grace: Duration) -> u32 {
    u32::try_from(grace.as_secs()).unwrap_or(u32::MAX).max(1)
}

/// The exit code of one Docker wait answer.
fn exit_code(status: i64) -> Option<i32> {
    i32::try_from(status).ok()
}

/// The exit of one wait answer, with the forced flag of a code 137.
#[must_use]
pub fn exit_of(response: &ContainerWaitResponse) -> SubstrateExit {
    let code = exit_code(response.status_code);
    SubstrateExit {
        code,
        signal: None,
        forced: code == Some(137),
    }
}

/// Removes the run directory of a container, when this host has one.
fn remove_run_dir(ctx: &Ctx, id: &str) -> SandboxResult<()> {
    let Ok(host) = crate::shared::host::SandboxHost::from_ctx(ctx) else {
        return Ok(());
    };
    let dir = host.config.state_dir.join(id);
    if host.fs.exists(&dir) {
        host.fs.remove_dir_all(&dir)?;
    }
    Ok(())
}

/// The last `max` bytes of the container log.
///
/// The log streams as frames and the buffer keeps at most `max` bytes, so a
/// long log never grows the memory use.
pub(crate) async fn read_log_tail(api: &dyn DockerApi, id: &str, max: usize) -> Vec<u8> {
    let options = ContainerLogsOptions {
        tail: Some(LOG_TAIL_LINES.to_string()),
        ..ContainerLogsOptions::default()
    };
    let Ok(mut stream) = api.container_logs(id, &options).await else {
        return Vec::new();
    };
    let mut bytes = Vec::new();
    while let Some(frame) = stream.next().await {
        let Ok(frame) = frame else {
            break;
        };
        match frame {
            LogOutput::StdOut { message }
            | LogOutput::StdErr { message }
            | LogOutput::Console { message } => bytes.extend_from_slice(&message),
            LogOutput::StdIn { .. } => {}
        }
        if bytes.len() > max {
            let remove = bytes.len() - max;
            bytes.drain(..remove);
        }
    }
    bytes
}
