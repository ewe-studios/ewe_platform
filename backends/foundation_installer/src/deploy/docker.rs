//! The `docker` deploy target: one container per entry.
//!
//! Every call goes through the injected [`DockerApi`], and the host
//! implementation uses `foundation_deployment_docker` with its generated
//! types. The installer builds no `docker` command string.

use async_trait::async_trait;
use foundation_deployment_docker::client::{ContainerCreateBody, ContainerHostConfig};
use foundation_deployment_docker::generated::shared::MaximumRetryCountNameObject;
use serde_json::json;

use crate::context::InstallContext;
use crate::deploy::{DeployTargetAdapter, Deployed};
use crate::error::InstallError;
use crate::manifest::{DockerRestart, Entry};
use crate::source::Artifact;

/// The `docker` deploy adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct DockerDeploy;

#[async_trait]
impl DeployTargetAdapter for DockerDeploy {
    fn kind(&self) -> &'static str {
        "docker"
    }

    async fn apply(
        &self,
        entry: &Entry,
        artifact: &Artifact,
        previous: Option<&Deployed>,
        ctx: &InstallContext,
    ) -> Result<Deployed, InstallError> {
        let Artifact::Image(image) = artifact else {
            return Err(InstallError::Incompatible {
                entry: entry.name.to_string(),
                deploy: self.kind().to_string(),
                artifact: artifact.class().to_string(),
            });
        };
        let crate::manifest::DeploySpec::Docker { .. } = &entry.deploy else {
            return Err(InstallError::Incompatible {
                entry: entry.name.to_string(),
                deploy: entry.deploy.kind().to_string(),
                artifact: artifact.class().to_string(),
            });
        };

        let docker = ctx.docker()?.clone();
        let target_name = entry.name.as_str().to_string();
        let next_name = format!("{target_name}-next");
        remove_if_present(&next_name, ctx).await;

        let body = build_body(entry, image);

        let created = docker
            .create_container(&body, &next_name)
            .await
            .map_err(|err| InstallError::Apply {
                entry: entry.name.to_string(),
                reason: format!("cannot create the container {next_name}: {err}"),
            })?;
        let id = created.id;

        if let Err(err) = start_and_wait(entry, &id, ctx).await {
            discard(&docker, &id, ctx).await;
            return Err(err);
        }

        let replace_error = match previous {
            Some(Deployed::Container { id: old_id, .. }) => {
                replace_old_container(entry, old_id, ctx).await.err()
            }
            _ => None,
        };
        if let Some(err) = replace_error {
            discard(&docker, &id, ctx).await;
            return Err(err);
        }

        if let Err(err) = docker.rename_container(&id, &target_name).await {
            discard(&docker, &id, ctx).await;
            return Err(InstallError::Apply {
                entry: entry.name.to_string(),
                reason: format!("cannot rename {next_name} to {target_name}: {err}"),
            });
        }

        Ok(Deployed::Container {
            name: target_name,
            id,
            image: image.clone(),
            version: entry.version.clone(),
        })
    }

    async fn exists(
        &self,
        _entry: &Entry,
        deployed: &Deployed,
        ctx: &InstallContext,
    ) -> Result<bool, InstallError> {
        let Deployed::Container { name, .. } = deployed else {
            return Ok(false);
        };
        let docker = ctx.docker()?;
        Ok(docker.inspect_container(name).await.is_ok())
    }
}

/// Remove a container that the failed step made.
async fn discard(
    docker: &std::sync::Arc<dyn crate::context::DockerApi>,
    id: &str,
    ctx: &InstallContext,
) {
    let _ = docker
        .stop_container(id, Some(ctx.config().stop_container_timeout_secs))
        .await;
    let _ = docker.remove_container(id, true).await;
}

/// Remove a leftover container of a failed earlier run, if it exists.
async fn remove_if_present(name: &str, ctx: &InstallContext) {
    let Ok(docker) = ctx.docker() else {
        return;
    };
    if docker.inspect_container(name).await.is_ok() {
        let _ = docker
            .stop_container(name, Some(ctx.config().stop_container_timeout_secs))
            .await;
        let _ = docker.remove_container(name, true).await;
    }
}

/// Build the create body from the manifest fields and the container marks.
fn build_body(entry: &Entry, image: &crate::source::ImageRef) -> ContainerCreateBody {
    let crate::manifest::DeploySpec::Docker {
        ports,
        env,
        volumes,
        restart,
        command,
    } = &entry.deploy
    else {
        return ContainerCreateBody::new(image.reference());
    };
    let restart = restart.unwrap_or(DockerRestart::UnlessStopped);
    let mut body = ContainerCreateBody::new(image.reference())
        .with_env(
            env.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        )
        .with_labels(json!({
            "dev.ewe.entry": entry.name.as_str(),
            "dev.ewe.version": entry.version,
        }));

    if let Some(command) = command {
        body = body.with_cmd(command.clone());
    }

    let mut exposed_ports = serde_json::Map::new();
    let mut bindings = serde_json::Map::new();
    for port in ports {
        let key = format!("{}/{}", port.container, port.protocol.as_str());
        exposed_ports.insert(key.clone(), json!({}));
        bindings.insert(
            key,
            json!([{ "HostIp": "", "HostPort": port.host.to_string() }]),
        );
    }
    if !ports.is_empty() {
        body = body.with_exposed_ports(serde_json::Value::Object(exposed_ports));
    }

    let mut host = ContainerHostConfig::default();
    host.base.restart_policy = Some(MaximumRetryCountNameObject {
        maximum_retry_count: None,
        name: Some(restart.as_str().to_string()),
    });
    if !ports.is_empty() {
        host.base.port_bindings = Some(serde_json::Value::Object(bindings));
    }
    if !volumes.is_empty() {
        host.base.binds = Some(
            volumes
                .iter()
                .map(|volume| format!("{}:{}", volume.host.display(), volume.container.display()))
                .collect(),
        );
    }
    body.with_host_config(host)
}

async fn start_and_wait(entry: &Entry, id: &str, ctx: &InstallContext) -> Result<(), InstallError> {
    let docker = ctx.docker()?;
    docker
        .start_container(id)
        .await
        .map_err(|err| InstallError::Apply {
            entry: entry.name.to_string(),
            reason: format!("cannot start the container: {err}"),
        })?;

    let deadline = ctx.clock().now() + ctx.config().start_timeout;
    loop {
        let inspect = match docker.inspect_container(id).await {
            Ok(inspect) => inspect,
            Err(err) => {
                return Err(InstallError::Apply {
                    entry: entry.name.to_string(),
                    reason: format!("cannot inspect the container: {err}"),
                });
            }
        };
        if inspect
            .state
            .and_then(|state| state.running)
            .unwrap_or(false)
        {
            return Ok(());
        }
        if ctx.clock().now() >= deadline {
            return Err(InstallError::StartTimeout {
                entry: entry.name.to_string(),
                timeout: ctx.config().start_timeout,
            });
        }
        ctx.sleep(ctx.config().start_poll_interval).await;
    }
}

async fn replace_old_container(
    entry: &Entry,
    old_id: &str,
    ctx: &InstallContext,
) -> Result<(), InstallError> {
    let docker = ctx.docker()?;
    if let Err(err) = docker
        .stop_container(old_id, Some(ctx.config().stop_container_timeout_secs))
        .await
    {
        return Err(InstallError::Apply {
            entry: entry.name.to_string(),
            reason: format!("cannot stop the old container: {err}"),
        });
    }
    docker
        .remove_container(old_id, false)
        .await
        .map_err(|err| InstallError::Apply {
            entry: entry.name.to_string(),
            reason: format!("cannot remove the old container: {err}"),
        })
}
