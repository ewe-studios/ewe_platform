//! [`HetznerServer`] — a VPS as a [`Deployable`].
//!
//! **WHY:** the rest of the platform deploys things through one trait. A Hetzner
//! box should be no different from a container: `deploy` it, get an address,
//! `destroy` it when done.
//!
//! **WHAT:** creates (or finds) a server, waits for it to build, and persists
//! `{ id, public_ip }` — the output the provider-agnostic deployables
//! (`SshHardening`, `DockerBootstrap`, feature 05/06) take as a field.
//!
//! **HOW:** *create-or-find*, per spec-56 decision 03 §3. A duplicate VPS costs
//! real money, so an accidental second create is worse than a no-op:
//!
//! | Recorded state | The instance | `deploy` does |
//! |---|---|---|
//! | `{ id: 42 }` | alive | **returns it** — no create, no second bill |
//! | `{ id: 42 }` | gone | logs plainly, creates a replacement |
//! | none | — | looks Hetzner up by name first, then creates |
//!
//! Note "alive" means *alive*, not "the API answered": a `deleting` server still
//! resolves, and returning one would hand the caller a corpse.
//!
//! **What this deployable does not do:** harden the box, or install anything.
//! Those are provider-agnostic and live in their own deployables (decision 03
//! §1), which is what lets them be tested against a local sshd fixture with no
//! cloud account at all.

use foundation_db::core::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::traits::{BoxFuture, Deployable};
use foundation_netio::shared::client::dns::SystemDnsResolver;
use serde::{Deserialize, Serialize};

use crate::client::HetznerClient;
use crate::types::{CreateServerRequest, HetznerError, Server, ServerStatus};

/// What a deployed server is, and where to reach it.
///
/// This is the input to everything downstream — `SshHardening { host: out.host() }`
/// — which is what makes composition-by-field work (decision 03 §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerDeployOutput {
    /// Hetzner's server id.
    pub id: i64,
    /// The name it was created with.
    pub name: String,
    /// Its public IPv4 address.
    pub public_ip: String,
}

impl ServerDeployOutput {
    /// The host the next deployable connects to.
    #[must_use]
    pub fn host(&self) -> String {
        self.public_ip.clone()
    }
}

/// A Hetzner Cloud server.
#[derive(Debug, Clone)]
pub struct HetznerServer {
    /// The client — carries the token and the transport.
    ///
    /// Held as a field rather than built from the environment inside `deploy`, so
    /// a test can inject a mock and a caller can use its own secret store
    /// (decision 02 §5).
    pub client: HetznerClient,
    /// The server's name. Also how `deploy` finds an existing box.
    pub name: String,
    /// Hetzner's server type, e.g. `cx22`.
    pub server_type: String,
    /// The image to boot, e.g. `ubuntu-24.04`.
    pub image: String,
    /// Location, e.g. `nbg1`. `None` lets Hetzner choose.
    pub location: Option<String>,
    /// SSH key ids permitted to log in on first boot.
    pub ssh_keys: Vec<i64>,
    /// cloud-init user-data (feature 04).
    pub user_data: Option<String>,
    /// Keep a created server when a later step fails, instead of destroying it.
    ///
    /// Default `false` — decision 03 §5, mirroring `force_rm` in spec-53: a
    /// stranded VPS bills. Set it when the box's state is the thing you need to
    /// debug.
    pub keep_on_failure: bool,
}

impl HetznerServer {
    /// A server declaration with Hetzner's defaults.
    #[must_use]
    pub fn new(client: HetznerClient, name: impl Into<String>) -> Self {
        Self {
            client,
            name: name.into(),
            // Verified against the live API 2026-07-17. Hetzner retires server
            // types (`cx22`, this default until today, no longer exists), so a
            // stale default here is a create that fails at the vendor. The error
            // is clear when it happens — `invalid_input` naming the type — but
            // prefer to check this when regenerating.
            server_type: "cx23".to_string(),
            image: "ubuntu-24.04".to_string(),
            location: None,
            ssh_keys: Vec::new(),
            user_data: None,
            keep_on_failure: false,
        }
    }

    /// Set the server type.
    #[must_use]
    pub fn server_type(mut self, server_type: impl Into<String>) -> Self {
        self.server_type = server_type.into();
        self
    }

    /// Set the image.
    #[must_use]
    pub fn image(mut self, image: impl Into<String>) -> Self {
        self.image = image.into();
        self
    }

    /// Set the location.
    #[must_use]
    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Permit these SSH key ids on first boot.
    #[must_use]
    pub fn ssh_keys(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.ssh_keys = ids.into_iter().collect();
        self
    }

    /// Set the cloud-init user-data.
    #[must_use]
    pub fn user_data(mut self, user_data: impl Into<String>) -> Self {
        self.user_data = Some(user_data.into());
        self
    }

    /// Keep the box when a later step fails, for debugging (decision 03 §5).
    #[must_use]
    pub fn keep_on_failure(mut self, keep: bool) -> Self {
        self.keep_on_failure = keep;
        self
    }

    /// Whether a server is usable, as opposed to merely present.
    ///
    /// A `deleting` server still answers `GET /servers/{id}`. Treating "the API
    /// returned something" as "alive" would have `deploy` hand back a corpse
    /// (decision 03 §3).
    fn is_usable(server: &Server) -> bool {
        matches!(server.status, ServerStatus::Running | ServerStatus::Starting)
            || server.status == ServerStatus::Initializing
    }

    /// Turn a settled server into the output, or say why it cannot be one.
    fn output_for(server: &Server) -> Result<ServerDeployOutput, HetznerError> {
        let Some(public_ip) = server.public_ipv4.clone() else {
            return Err(HetznerError::Api {
                status: 200,
                code: "no_public_ipv4".to_string(),
                message: format!(
                    "server {} ({}) has no public IPv4 address — nothing can reach it over SSH",
                    server.id, server.name
                ),
            });
        };
        if server.status != ServerStatus::Running {
            return Err(HetznerError::Api {
                status: 200,
                code: "not_running".to_string(),
                message: format!(
                    "server {} settled as {:?}, not running — it will not accept connections",
                    server.id, server.status
                ),
            });
        }
        Ok(ServerDeployOutput {
            id: server.id,
            name: server.name.clone(),
            public_ip,
        })
    }
}

impl Deployable for HetznerServer {
    const NAMESPACE: &'static str = "hetzner/cloud/servers";

    type DeployOutput = ServerDeployOutput;
    type DestroyOutput = ();
    type Error = HetznerError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> BoxFuture<'static, Result<Self::DeployOutput, Self::Error>> {
        // Hetzner speaks HTTPS with a bearer token, so the client is ours; the
        // ProviderClient is here for the namespaced store (built now — it needs
        // `&self`/`&client` — and moved into the future).
        let hetzner = self.client.clone();
        let store = self.store(&client);
        let request = CreateServerRequest {
            name: self.name.clone(),
            server_type: self.server_type.clone(),
            image: self.image.clone(),
            location: self.location.clone(),
            ssh_keys: self.ssh_keys.clone(),
            user_data: self.user_data.clone(),
        };
        let keep_on_failure = self.keep_on_failure;
        let name = self.name.clone();

        Box::pin(async move {
            let key = instance_id.to_string();

            // 1. What we think we already made.
            let recorded: Option<ServerDeployOutput> = store
                .get_typed(&key)
                .map_err(|e| HetznerError::Transport(format!("load deploy state: {e}")))?;

            if let Some(recorded) = &recorded {
                match hetzner.get_server(recorded.id).await? {
                    Some(server) if HetznerServer::is_usable(&server) => {
                        tracing::debug!(id = server.id, %name, "server already exists; reusing it");
                        let settled = hetzner.await_running(server.id).await?;
                        return HetznerServer::output_for(&settled);
                    }
                    // Deleted out of band, or dying. Recreating is the recovery
                    // case and usually what the caller wanted — but nobody asked
                    // for the charge, so say so rather than letting a new bill
                    // appear silently (decision 03 §3).
                    found => {
                        tracing::warn!(
                            id = recorded.id,
                            %name,
                            status = ?found.map(|s| s.status),
                            "recorded server no longer usable; creating a replacement (this bills a new instance)"
                        );
                    }
                }
            }

            // 2. Hetzner may have one we never recorded — a crash between create
            //    and persist. Finding it by name is what stops that from
            //    becoming a second bill.
            if let Some(existing) = hetzner.find_server_by_name(&name).await? {
                if HetznerServer::is_usable(&existing) {
                    tracing::debug!(
                        id = existing.id,
                        %name,
                        "found an unrecorded server with this name; adopting it rather than creating a second"
                    );
                    let settled = hetzner.await_running(existing.id).await?;
                    let output = HetznerServer::output_for(&settled)?;
                    store
                        .store_typed(&key, &output)
                        .map_err(|e| HetznerError::Transport(format!("persist deploy state: {e}")))?;
                    return Ok(output);
                }
            }

            // 3. Create.
            let created = hetzner.create_server(&request).await?;

            // Everything past this point owns a billing instance. A failure that
            // leaves it running is the mirror of spec-53's stranded container,
            // except this one costs money — so unwind unless told to keep it
            // (decision 03 §5).
            let finish = async {
                let settled = hetzner.await_running(created.id).await?;
                let output = HetznerServer::output_for(&settled)?;
                store
                    .store_typed(&key, &output)
                    .map_err(|e| HetznerError::Transport(format!("persist deploy state: {e}")))?;
                Ok::<_, HetznerError>(output)
            }
            .await;

            match finish {
                Ok(output) => Ok(output),
                Err(e) if keep_on_failure => {
                    tracing::warn!(
                        id = created.id,
                        error = %e,
                        "deploy failed; keeping the server because keep_on_failure is set — it is still billing"
                    );
                    Err(e)
                }
                Err(e) => {
                    tracing::warn!(id = created.id, error = %e, "deploy failed; destroying the server");
                    if let Err(cleanup) = hetzner.delete_server(created.id).await {
                        // Report the original failure, not the cleanup's — the
                        // first one is why we are here. But a leaked instance
                        // bills, so it cannot be silent.
                        tracing::error!(
                            id = created.id,
                            error = %cleanup,
                            "could not destroy the server after a failed deploy — IT IS STILL BILLING"
                        );
                    }
                    let _ = store.remove(&key);
                    Err(e)
                }
            }
        })
    }

    fn destroy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> BoxFuture<'static, Result<Self::DestroyOutput, Self::Error>> {
        let hetzner = self.client.clone();
        let store = self.store(&client);
        let name = self.name.clone();

        Box::pin(async move {
            let key = instance_id.to_string();
            let recorded: Option<ServerDeployOutput> = store
                .get_typed(&key)
                .map_err(|e| HetznerError::Transport(format!("load deploy state: {e}")))?;

            // Nothing recorded is not a failure: destroy promises the server does
            // not exist, and it does not. Erroring would make teardown in a
            // `finally` unusable.
            let Some(recorded) = recorded else {
                tracing::debug!(%name, instance_id, "no recorded server; nothing to destroy");
                return Ok(());
            };

            hetzner.delete_server(recorded.id).await?;
            store
                .remove(&key)
                .map_err(|e| HetznerError::Transport(format!("clear deploy state: {e}")))?;
            tracing::debug!(id = recorded.id, %name, "destroyed");
            Ok(())
        })
    }
}
