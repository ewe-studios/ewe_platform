//! `Deployable` implementation for Docker containers (Decision 05).
//!
//! WHY: `foundation_deployment_docker` is a `foundation_deployment` provider —
//! infrastructure is declared as a Rust type implementing [`Deployable`], and
//! the runtime deploys/destroys it. This is the crate's stated purpose
//! (requirements §Summary); the HTTP/BuildKit client surface is what it builds
//! on.
//!
//! WHAT: [`ContainerDeployment`] describes a container (image, command, env) and
//! implements [`Deployable`]: `deploy` creates + starts the container and
//! persists its id; `destroy` reads the id back and stops + removes it.
//!
//! HOW: `deploy`/`destroy` are plain async — each returns
//! `Box::pin(async move { … })` (a [`BoxFuture`]). Docker speaks over a Unix
//! socket, which does not fit the `ProviderClient`'s TCP+DNS HTTP client, so the
//! futures build their **own** [`DockerClient`] from the socket path and use the
//! `ProviderClient` only for **state persistence** (via the `Deployable::store`
//! namespaced store) — the canonical "unique underlying mechanics" case for a
//! `Deployable`.

use std::path::PathBuf;

use foundation_db::core::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::traits::{BoxFuture, Deployable};
use foundation_netio::shared::client::dns::SystemDnsResolver;
use serde::{Deserialize, Serialize};

use crate::client::{DockerClient, DEFAULT_DOCKER_SOCKET};
use crate::error::DockerError;

/// A container to deploy: an image plus optional command, environment, and name.
///
/// `deploy` creates and starts it; `destroy` stops and removes it. The
/// container id is persisted between the two under the deployable's namespace.
#[derive(Debug, Clone)]
pub struct ContainerDeployment {
    /// Image reference, e.g. `alpine:latest` (must already be pullable/present).
    pub image: String,
    /// Optional container name (Docker's `?name=`).
    pub name: Option<String>,
    /// Command override (`Cmd`); empty uses the image default.
    pub cmd: Vec<String>,
    /// Environment variables as `KEY=VALUE` (`Env`).
    pub env: Vec<String>,
    /// Docker daemon socket (defaults to [`DEFAULT_DOCKER_SOCKET`]).
    pub socket_path: PathBuf,
}

impl ContainerDeployment {
    /// A deployment of `image` against the default Docker socket.
    #[must_use]
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            name: None,
            cmd: Vec::new(),
            env: Vec::new(),
            socket_path: PathBuf::from(DEFAULT_DOCKER_SOCKET),
        }
    }

    /// Set the container name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the command override.
    #[must_use]
    pub fn with_cmd<I, S>(mut self, cmd: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.cmd = cmd.into_iter().map(Into::into).collect();
        self
    }

    /// Add a `KEY=VALUE` environment entry.
    #[must_use]
    pub fn with_env(mut self, entry: impl Into<String>) -> Self {
        self.env.push(entry.into());
        self
    }

    /// Deploy against a non-default Docker socket.
    #[must_use]
    pub fn with_socket(mut self, socket_path: impl Into<PathBuf>) -> Self {
        self.socket_path = socket_path.into();
        self
    }

    /// The JSON create body Docker expects (`Image`/`Cmd`/`Env`).
    fn create_body(&self) -> serde_json::Value {
        let mut body = serde_json::json!({ "Image": self.image });
        if !self.cmd.is_empty() {
            body["Cmd"] = serde_json::json!(self.cmd);
        }
        if !self.env.is_empty() {
            body["Env"] = serde_json::json!(self.env);
        }
        body
    }
}

/// Output of a successful container deploy — the created container's id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerDeployOutput {
    /// The Docker container id (also persisted for `destroy`).
    pub container_id: String,
}

impl Deployable for ContainerDeployment {
    const NAMESPACE: &'static str = "docker/containers";

    type DeployOutput = ContainerDeployOutput;
    type DestroyOutput = ();
    type Error = DockerError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> BoxFuture<'static, Result<Self::DeployOutput, Self::Error>> {
        // Docker dials its own Unix socket; ProviderClient is used only for the
        // namespaced state store (built here — needs `&self`/`&client` — and
        // moved into the future so `destroy` can read the container id back).
        let docker = DockerClient::connect_unix(&self.socket_path);
        let body = self.create_body();
        let name = self.name.clone();
        let store = self.store(&client);

        Box::pin(async move {
            let created = docker.create_container(&body, name.as_deref()).await?;
            docker.start_container(&created.id).await?;

            let output = ContainerDeployOutput { container_id: created.id };
            store
                .store_typed(&instance_id.to_string(), &output)
                .map_err(|e| DockerError::Unavailable(format!("persist deploy state: {e}")))?;
            Ok(output)
        })
    }

    fn destroy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> BoxFuture<'static, Result<Self::DestroyOutput, Self::Error>> {
        let docker = DockerClient::connect_unix(&self.socket_path);
        let store = self.store(&client);

        Box::pin(async move {
            let key = instance_id.to_string();
            let output: ContainerDeployOutput = store
                .get_typed(&key)
                .map_err(|e| DockerError::Unavailable(format!("load deploy state: {e}")))?
                .ok_or_else(|| {
                    DockerError::Unavailable(format!(
                        "no recorded container for instance {instance_id} — nothing to destroy"
                    ))
                })?;

            docker.stop_container(&output.container_id, Some(10)).await?;
            docker.remove_container(&output.container_id, true).await?;
            store
                .remove(&key)
                .map_err(|e| DockerError::Unavailable(format!("clear deploy state: {e}")))?;
            Ok(())
        })
    }
}
