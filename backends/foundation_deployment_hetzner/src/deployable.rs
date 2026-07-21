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

/// What this deployable was asked to build.
///
/// **Why the state carries the declaration and not just the outputs:** a completed
/// deploy's state is the *authoritative* record of what exists — that is the whole
/// reason for writing it. But authority is only worth anything if the record is
/// complete enough to check. The store key is `instance_id` alone; nothing in it
/// ties a record to the declaration that produced it. Change `name` from `web-1`
/// to `web-2` and redeploy instance 0, and a record holding only outputs looks
/// perfectly valid — so you get `web-1` back, silently, and `web-2` is never
/// built.
///
/// Recording what was *asked for* alongside what was *made* is what lets
/// [`HetznerServer::deploy`] tell "this record is mine" from "this record is for a
/// different declaration".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerDeclaration {
    /// The name asked for.
    pub name: String,
    /// The server type asked for.
    pub server_type: String,
    /// The image asked for.
    pub image: String,
    /// The location asked for, if any.
    pub location: Option<String>,
}

/// What a deployed server is, and where to reach it.
///
/// This is the input to everything downstream — `SshHardening { host: out.host() }`
/// — which is what makes composition-by-field work (decision 03 §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerDeployOutput {
    /// Which provider this record is about.
    ///
    /// A record that does not say what it belongs to cannot be acted on by
    /// anything but the code that wrote it.
    #[serde(default = "hetzner_provider")]
    pub provider: String,
    /// Hetzner's server id.
    ///
    /// **A convenience, not the identity.** Hetzner assigns it, it means nothing
    /// on its own, and nothing about holding one proves it still points at our
    /// instance. [`ServerDeployOutput::identity`] is the identity.
    pub id: i64,
    /// The name it was created with.
    pub name: String,
    /// Its public IPv4 address.
    pub public_ip: String,
    /// The label we stamped on the machine, as `key=value`.
    ///
    /// **This is what makes the record authoritative.** It is written into the
    /// create request, so it is on the server whether or not we ever read the
    /// response; it survives a rename; and it lets us *enumerate* our instances
    /// from the provider rather than hoping an id still resolves.
    #[serde(default)]
    pub identity: Option<String>,
    /// What this server was asked to be.
    ///
    /// `Option` so a record written before this field existed still loads — an
    /// old record simply cannot be checked, which is the status quo, not a
    /// regression.
    #[serde(default)]
    pub declared: Option<ServerDeclaration>,
}

fn hetzner_provider() -> String {
    "hetzner".to_string()
}

/// The label key every server this deployable creates carries.
///
/// A DNS-subdomain prefix, which Hetzner's label grammar allows on **keys** (a
/// prefix, a `/`, then the name). That namespaces us against a user's own labels
/// and tells anyone in the Hetzner console who owns the machine.
///
/// Hetzner reserves the `hetzner.cloud/` prefix; `ewe.dev/` is ours.
pub const IDENTITY_LABEL: &str = "ewe.dev/deployment";

/// Turn a namespace + instance into a legal Hetzner label **value**.
///
/// The grammar, from Hetzner's own spec: a value is at most 63 characters, must
/// begin and end with `[a-z0-9A-Z]`, and may contain dashes, underscores, dots and
/// alphanumerics between. **No slashes** — those are only legal in a *key*, to
/// separate the prefix.
///
/// This matters because `NAMESPACE` is `"hetzner/cloud/servers"`, and the obvious
/// `format!("{NAMESPACE}/{instance_id}")` produces `hetzner/cloud/servers/0` —
/// which Hetzner rejects with `invalid label_selector: value contains invalid
/// characters or is malformed`. Every mock in this crate accepted it happily; the
/// vendor did not.
fn label_value(namespace: &str, instance_id: usize) -> String {
    let mut value: String = format!("{namespace}.{instance_id}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '.'
            }
        })
        .collect();

    // 63 is the ceiling, and it must still end alphanumeric after truncating.
    value.truncate(63);

    // Both ends, not just the tail: a namespace beginning with punctuation (or a
    // multi-byte char, which maps to '.') would otherwise open with a dot — the
    // same class of invalid this function exists to prevent.
    let start = value
        .find(|c: char| c.is_ascii_alphanumeric())
        .unwrap_or(value.len());
    value.drain(..start);
    while value
        .chars()
        .next_back()
        .is_some_and(|c| !c.is_ascii_alphanumeric())
    {
        value.pop();
    }
    value
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

    /// The identity this deployable stamps on the instance for `instance_id`.
    ///
    /// `<namespace>/<instance_id>` — the deployment slot, which is what the record
    /// is *about*. Not the name (a user edits it) and not the id (Hetzner assigns
    /// it, and we may never learn it).
    fn identity(instance_id: usize) -> String {
        label_value(Self::NAMESPACE, instance_id)
    }

    /// What this deployable is asking for — recorded alongside what gets built.
    fn declaration(&self) -> ServerDeclaration {
        ServerDeclaration {
            name: self.name.clone(),
            server_type: self.server_type.clone(),
            image: self.image.clone(),
            location: self.location.clone(),
        }
    }

    /// Turn a settled server into the output, or say why it cannot be one.
    fn output_for(
        server: &Server,
        declared: &ServerDeclaration,
        identity: &str,
    ) -> Result<ServerDeployOutput, HetznerError> {
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
            provider: hetzner_provider(),
            id: server.id,
            name: server.name.clone(),
            public_ip,
            identity: Some(format!("{IDENTITY_LABEL}={identity}")),
            declared: Some(declared.clone()),
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
            // Stamped in the CREATE REQUEST, which is the whole point: the machine
            // carries our identity whether or not the response ever reaches us.
            labels: [(IDENTITY_LABEL.to_string(), Self::identity(instance_id))]
                .into_iter()
                .collect(),
        };
        let keep_on_failure = self.keep_on_failure;
        let name = self.name.clone();
        let declaration = self.declaration();
        let identity = Self::identity(instance_id);

        Box::pin(async move {
            let key = instance_id.to_string();

            // 1. What we think we already made.
            let recorded: Option<ServerDeployOutput> = store
                .get_typed(&key)
                .map_err(|e| HetznerError::Transport(format!("load deploy state: {e}")))?;

            if let Some(recorded) = &recorded {
                // A completed deploy's record is AUTHORITATIVE — that is the
                // whole reason for writing it. So the question is never "is this
                // record still true?" (we do not go hunting behind its back), it
                // is "is this record MINE?".
                //
                // The store key is `instance_id` alone, so nothing ties a record
                // to the declaration that made it. Change `name` from web-1 to
                // web-2 and redeploy instance 0: a record holding only outputs
                // looks perfectly valid, you get web-1 back, and web-2 is never
                // built. Same for server_type, image, location.
                //
                // A record that is not mine is a state that has gone stale
                // against the code, and the only safe answer is to say so.
                // Reusing it ignores the declaration; creating alongside it leaves
                // the old machine billing unnoticed; destroying it to make room
                // throws away something nobody asked us to remove.
                if let Some(declared) = &recorded.declared {
                    if declared != &declaration {
                        return Err(HetznerError::Api {
                            status: 409,
                            code: "state_declaration_mismatch".to_string(),
                            message: format!(
                                "instance {instance_id}'s recorded state is stale against this \
                                 declaration: it holds server {} built as {declared:?}, but the \
                                 code now asks for {declaration:?}. Destroy instance \
                                 {instance_id} first (which removes server {}), or deploy under \
                                 a different instance_id — otherwise the recorded server would \
                                 keep billing unnoticed.",
                                recorded.id, recorded.id
                            ),
                        });
                    }
                }

                match hetzner.get_server(recorded.id).await? {
                    Some(server) if HetznerServer::is_usable(&server) => {
                        tracing::debug!(id = server.id, %name, "server already exists; reusing it");
                        let settled = hetzner.await_running(server.id).await?;
                        return HetznerServer::output_for(&settled, &declaration, &identity);
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
            //    and persist, or a create whose response we could not read.
            //
            //    Enumerated BY OUR LABEL, not by name: the label is what we
            //    stamped on the machine for this slot, so it identifies the
            //    instance even if the declaration has since been renamed, and it
            //    is present regardless of whether we ever saw the id.
            let ours = hetzner.find_servers_by_label(IDENTITY_LABEL, &identity).await?;
            if let Some(existing) = ours.iter().find(|s| HetznerServer::is_usable(s)) {
                {
                    tracing::debug!(
                        id = existing.id,
                        %identity,
                        "found an unrecorded server carrying this slot's label; adopting it \
                         rather than creating a second"
                    );
                    let settled = hetzner.await_running(existing.id).await?;
                    let output = HetznerServer::output_for(&settled, &declaration, &identity)?;
                    store
                        .store_typed(&key, &output)
                        .map_err(|e| HetznerError::Transport(format!("persist deploy state: {e}")))?;
                    return Ok(output);
                }
            }

            // 3. Create.
            //
            // **The create's response is a hint, not the record.** Everything that
            // identifies this instance — the name, and the slot's label — went out
            // in the REQUEST, so the machine carries them whether or not the reply
            // ever reaches us. The authoritative state is therefore read back from
            // Hetzner below, never taken from what create happened to return.
            //
            // Which makes a failure here recoverable rather than fatal: a decode
            // error, a dropped connection or a timeout after `POST /servers` means
            // "we do not know", not "it does not exist". Ask.
            let created_id = match hetzner.create_server(&request).await {
                Ok(created) => created.id,
                Err(e) => {
                    let ours = hetzner
                        .find_servers_by_label(IDENTITY_LABEL, &identity)
                        .await
                        .map_err(|lookup| {
                            tracing::error!(
                                %identity,
                                create_error = %e,
                                lookup_error = %lookup,
                                "create failed AND we cannot check whether it landed — check the \
                                 console; a server may be billing"
                            );
                            e.clone()
                        })?;

                    match ours.first() {
                        // It exists. We asked for it, it carries our label, and it
                        // is the server this slot wanted — destroying it and
                        // returning an error would throw away exactly what the
                        // caller requested because we could not read a receipt.
                        Some(server) => {
                            tracing::warn!(
                                id = server.id,
                                %identity,
                                error = %e,
                                "create's response was unusable, but the server exists and \
                                 carries this slot's label — adopting it"
                            );
                            server.id
                        }
                        // Nothing there: the create genuinely did not land.
                        None => return Err(e),
                    }
                }
            };

            // Everything past this point owns a billing instance. A failure that
            // leaves it running is the mirror of spec-53's stranded container,
            // except this one costs money — so unwind unless told to keep it
            // (decision 03 §5).
            let finish = async {
                // Read the resource back. `await_running` polls `GET /servers/{id}`
                // until it settles, so `settled` is Hetzner's account of the
                // machine — not ours, and not the create response's. THAT is what
                // becomes the state, which is what makes the record something we
                // can zero in on the instance with later.
                let settled = hetzner.await_running(created_id).await?;
                let output = HetznerServer::output_for(&settled, &declaration, &identity)?;
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
                        id = created_id,
                        error = %e,
                        "deploy failed; keeping the server because keep_on_failure is set — it is still billing"
                    );
                    Err(e)
                }
                Err(e) => {
                    tracing::warn!(id = created_id, error = %e, "deploy failed; destroying the server");
                    if let Err(cleanup) = hetzner.delete_server(created_id).await {
                        // Report the original failure, not the cleanup's — the
                        // first one is why we are here. But a leaked instance
                        // bills, so it cannot be silent.
                        tracing::error!(
                            id = created_id,
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
        let identity = Self::identity(instance_id);

        Box::pin(async move {
            let key = instance_id.to_string();
            let recorded: Option<ServerDeployOutput> = store
                .get_typed(&key)
                .map_err(|e| HetznerError::Transport(format!("load deploy state: {e}")))?;

            // A record from a completed deploy is AUTHORITATIVE — it says what we
            // built, and it is the reason we bothered writing it. Delete what it
            // names and do not go second-guessing it by name: a lookup that
            // overrode a good record with a guess would make the record
            // pointless.
            if let Some(recorded) = &recorded {
                hetzner.delete_server(recorded.id).await?;
                // State clears last: a failed delete above keeps the record, so a
                // retry still knows what to chase.
                store
                    .remove(&key)
                    .map_err(|e| HetznerError::Transport(format!("clear deploy state: {e}")))?;
                tracing::debug!(id = recorded.id, %name, "destroyed");
                return Ok(());
            }

            // No record — which is NOT "nothing to destroy", and is the one case
            // where asking Hetzner is right, because there is no authority to
            // contradict.
            //
            // A create whose response we failed to read leaves exactly this: the
            // vendor built the machine, our decode failed, `deploy` returned an
            // error having stored nothing. Bailing out on an empty store is how a
            // live server was stranded while destroy reported success (observed
            // 2026-07-17, billing throughout).
            //
            // Enumerated by the slot's label — which the create request carried,
            // so it is on the machine regardless of what we managed to read back,
            // and it does not care what the server is called. `deploy` adopts by
            // the same label, so destroy must remove by it or deploy would adopt
            // what destroy refuses to delete.
            let orphans = hetzner.find_servers_by_label(IDENTITY_LABEL, &identity).await?;
            if orphans.is_empty() {
                // Genuinely absent, which is what destroy promises. Not an error:
                // erroring would make teardown in a `finally` unusable.
                tracing::debug!(%identity, instance_id, "nothing recorded and nothing at Hetzner");
                return Ok(());
            }
            for orphan in orphans {
                tracing::warn!(
                    id = orphan.id,
                    %identity,
                    "nothing recorded, but Hetzner has a server with this slot's label — \
                     deleting it. Usually means a create succeeded and we failed to read the \
                     response."
                );
                hetzner.delete_server(orphan.id).await?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{label_value, HetznerServer, IDENTITY_LABEL};

    /// `label_value` is private; this belongs here rather than in `tests/`.
    #[test]
    fn an_identity_is_a_legal_hetzner_label_value() {
        // The rule, from Hetzner's spec: <=63 chars, begins and ends
        // [a-z0-9A-Z], with -_. and alphanumerics between. No slashes — they are
        // only legal in a KEY, separating the prefix.
        //
        // NAMESPACE is "hetzner/cloud/servers", so the obvious
        // format!("{NAMESPACE}/{id}") gives "hetzner/cloud/servers/0", which the
        // real API rejects: `invalid label_selector: value contains invalid
        // characters or is malformed`. Every mock accepted it.
        let value = HetznerServer::identity(0);
        assert_eq!(value, "hetzner.cloud.servers.0");
        assert_valid(&value);

        // The key may carry a prefix — that is where a slash is allowed.
        assert!(IDENTITY_LABEL.contains('/'), "the key is prefixed: {IDENTITY_LABEL}");
        assert!(
            !IDENTITY_LABEL.starts_with("hetzner.cloud/"),
            "that prefix is reserved by Hetzner"
        );
    }

    #[test]
    fn a_hostile_namespace_still_yields_a_legal_value() {
        for (ns, id) in [
            ("a/b/c", 0),
            ("with spaces", 3),
            ("sym+bols!", 9),
            ("trailing/", 1),
            ("ünicode", 2),
        ] {
            let value = label_value(ns, id);
            assert_valid(&value);
        }
    }

    #[test]
    fn an_over_long_namespace_is_truncated_and_still_ends_alphanumeric() {
        // Truncating at 63 can leave a trailing '.', which is itself invalid —
        // the fix must not create the bug it is preventing.
        let value = label_value(&"x".repeat(80), 0);
        assert!(value.len() <= 63, "{} chars", value.len());
        assert_valid(&value);

        let value = label_value(&format!("{}.", "y".repeat(62)), 0);
        assert_valid(&value);
    }

    fn assert_valid(value: &str) {
        assert!(!value.is_empty(), "empty");
        assert!(value.len() <= 63, "{value:?} is {} chars", value.len());
        assert!(
            value.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()),
            "{value:?} must begin alphanumeric"
        );
        assert!(
            value.chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric()),
            "{value:?} must end alphanumeric"
        );
        assert!(
            value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'),
            "{value:?} has an illegal character"
        );
    }
}
