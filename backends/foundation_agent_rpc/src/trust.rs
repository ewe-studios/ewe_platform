//! The trusted-client store: which clients enrolled, and which may connect.
//!
//! WHY: a certificate that chains to the CA proves a client enrolled once; the
//! server also needs a durable record of who enrolled, a way to revoke one
//! client without rotating the CA, and a single-use guarantee for bootstrap
//! secrets that survives a restart.
//!
//! WHAT: [`TrustedClientStore`] over any `foundation_db` SQL backend
//! ([`AsyncQueryStore`]: Turso/SQLite by default via
//! [`TrustedClientStore::open_sqlite`], libsql, D1). Two tables:
//! - `agent_rpc_trusted_clients` — one row per issued client certificate
//!   (fingerprint, name, enrolled/expiry/last-seen/revoked times);
//! - `agent_rpc_bootstrap_secret_uses` — enrollments per bootstrap secret id,
//!   pruned once the secret expires.
//!
//! HOW: writes go to SQL first, then to an in-memory index loaded at open.
//! The mutual-TLS handshake consults that index synchronously (rustls
//! verifiers cannot await), so trust and revocation decisions never wait on
//! the database. Last-seen times are written in the background.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use foundation_db::{AsyncQueryStore, DataValue, SqlRow};
use foundation_errstacks::ErrorTrace;
use futures::StreamExt;

use crate::error::{AgentRpcError, AgentRpcResult};

/// File name of the default SQLite database inside a state directory.
pub const TRUSTED_CLIENTS_DB_FILE: &str = "trusted-clients.db";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS agent_rpc_trusted_clients (
    fingerprint TEXT PRIMARY KEY,
    client_name TEXT NOT NULL,
    cert_pem TEXT NOT NULL,
    enrolled_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    last_seen_at INTEGER,
    revoked_at INTEGER
);
CREATE TABLE IF NOT EXISTS agent_rpc_bootstrap_secret_uses (
    secret_id TEXT PRIMARY KEY,
    uses INTEGER NOT NULL,
    valid_until INTEGER NOT NULL
);
";

/// One enrolled client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedClient {
    /// Lowercase hex SHA-256 of the client certificate (DER).
    pub fingerprint_sha256: String,
    /// The certificate's common name.
    pub client_name: String,
    /// When the certificate was issued (Unix seconds).
    pub enrolled_at: u64,
    /// When the certificate expires (Unix seconds).
    pub expires_at: u64,
    /// The last successful mutual-TLS handshake (Unix seconds).
    pub last_seen_at: Option<u64>,
    /// When the client was revoked (Unix seconds).
    pub revoked_at: Option<u64>,
}

impl TrustedClient {
    /// Not revoked and not expired at `now` (Unix seconds).
    #[must_use]
    pub fn is_active_at(&self, now: u64) -> bool {
        self.revoked_at.is_none() && now < self.expires_at
    }
}

fn storage_err(what: &str, e: impl std::fmt::Display) -> ErrorTrace<AgentRpcError> {
    ErrorTrace::new(AgentRpcError::Storage(format!("{what}: {e}")))
}

fn int(v: u64) -> DataValue {
    DataValue::Integer(i64::try_from(v).unwrap_or(i64::MAX))
}

fn uint(row: &SqlRow, index: usize, what: &str) -> AgentRpcResult<u64> {
    let v: i64 = row.get(index).map_err(|e| storage_err(what, e))?;
    Ok(u64::try_from(v).unwrap_or(0))
}

fn opt_uint(row: &SqlRow, index: usize, what: &str) -> AgentRpcResult<Option<u64>> {
    let v: Option<i64> = row.get(index).map_err(|e| storage_err(what, e))?;
    Ok(v.map(|v| u64::try_from(v).unwrap_or(0)))
}

/// Durable record of enrolled clients and spent bootstrap secrets.
pub struct TrustedClientStore {
    storage: Arc<dyn AsyncQueryStore>,
    index: RwLock<HashMap<String, TrustedClient>>,
}

impl std::fmt::Debug for TrustedClientStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let clients = self.index.read().map_or(0, |i| i.len());
        f.debug_struct("TrustedClientStore")
            .field("clients", &clients)
            .finish_non_exhaustive()
    }
}

impl TrustedClientStore {
    /// Create the tables if needed and load the client index.
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the schema or the load fails.
    pub async fn open(storage: Arc<dyn AsyncQueryStore>) -> AgentRpcResult<Arc<Self>> {
        storage
            .execute_batch_async(SCHEMA)
            .await
            .map_err(|e| storage_err("creating the trusted-client tables", e))?;
        let mut index = HashMap::new();
        let mut rows = storage
            .query_async(
                "SELECT fingerprint, client_name, enrolled_at, expires_at, last_seen_at, revoked_at \
                 FROM agent_rpc_trusted_clients",
                &[],
            )
            .await
            .map_err(|e| storage_err("loading trusted clients", e))?;
        while let Some(row) = rows.next().await {
            let row = row.map_err(|e| storage_err("reading a trusted client", e))?;
            let client = TrustedClient {
                fingerprint_sha256: row.get(0).map_err(|e| storage_err("fingerprint", e))?,
                client_name: row.get(1).map_err(|e| storage_err("client_name", e))?,
                enrolled_at: uint(&row, 2, "enrolled_at")?,
                expires_at: uint(&row, 3, "expires_at")?,
                last_seen_at: opt_uint(&row, 4, "last_seen_at")?,
                revoked_at: opt_uint(&row, 5, "revoked_at")?,
            };
            index.insert(client.fingerprint_sha256.clone(), client);
        }
        Ok(Arc::new(Self {
            storage,
            index: RwLock::new(index),
        }))
    }

    /// Open (creating if needed) a SQLite database at `path` through
    /// `foundation_db`'s Turso backend, creating parent directories.
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the directory or database cannot be
    /// created or opened.
    pub async fn open_sqlite(path: impl AsRef<Path>) -> AgentRpcResult<Arc<Self>> {
        let path = path.as_ref();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)
                .map_err(|e| storage_err(&format!("creating {}", dir.display()), e))?;
        }
        let url = path
            .to_str()
            .ok_or_else(|| storage_err("database path", "not valid UTF-8"))?;
        let db = foundation_db::TursoStorage::open(url)
            .await
            .map_err(|e| storage_err(&format!("opening {}", path.display()), e))?;
        Self::open(Arc::new(db)).await
    }

    /// [`TRUSTED_CLIENTS_DB_FILE`] inside `state_dir`.
    #[must_use]
    pub fn default_path(state_dir: impl AsRef<Path>) -> PathBuf {
        state_dir.as_ref().join(TRUSTED_CLIENTS_DB_FILE)
    }

    fn read_index(&self) -> std::sync::RwLockReadGuard<'_, HashMap<String, TrustedClient>> {
        self.index
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_index(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<String, TrustedClient>> {
        self.index
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Record a newly issued client certificate.
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the insert fails (including a
    /// duplicate fingerprint).
    pub async fn record_enrollment(&self, client: TrustedClient, cert_pem: &str) -> AgentRpcResult<()> {
        self.storage
            .execute_async(
                "INSERT INTO agent_rpc_trusted_clients \
                 (fingerprint, client_name, cert_pem, enrolled_at, expires_at, last_seen_at, revoked_at) \
                 VALUES (?, ?, ?, ?, ?, NULL, NULL)",
                &[
                    DataValue::Text(client.fingerprint_sha256.clone()),
                    DataValue::Text(client.client_name.clone()),
                    DataValue::Text(cert_pem.to_string()),
                    int(client.enrolled_at),
                    int(client.expires_at),
                ],
            )
            .await
            .map_err(|e| storage_err("recording an enrolled client", e))?;
        self.write_index()
            .insert(client.fingerprint_sha256.clone(), client);
        Ok(())
    }

    /// The client with this certificate fingerprint.
    #[must_use]
    pub fn get(&self, fingerprint_sha256: &str) -> Option<TrustedClient> {
        self.read_index().get(fingerprint_sha256).cloned()
    }

    /// Every recorded client, oldest enrollment first.
    #[must_use]
    pub fn list(&self) -> Vec<TrustedClient> {
        let mut clients: Vec<_> = self.read_index().values().cloned().collect();
        clients.sort_by(|a, b| {
            (a.enrolled_at, &a.fingerprint_sha256).cmp(&(b.enrolled_at, &b.fingerprint_sha256))
        });
        clients
    }

    /// Whether the certificate with this fingerprint was issued here and is
    /// neither revoked nor expired at `now` (Unix seconds).
    #[must_use]
    pub fn is_trusted_at(&self, fingerprint_sha256: &str, now: u64) -> bool {
        self.read_index()
            .get(fingerprint_sha256)
            .is_some_and(|c| c.is_active_at(now))
    }

    /// Revoke a client: its certificate is refused from the next handshake
    /// on. Returns whether a client with this fingerprint was found (and not
    /// already revoked).
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the update fails.
    pub async fn revoke(&self, fingerprint_sha256: &str, now: u64) -> AgentRpcResult<bool> {
        let changed = self
            .storage
            .execute_async(
                "UPDATE agent_rpc_trusted_clients SET revoked_at = ? \
                 WHERE fingerprint = ? AND revoked_at IS NULL",
                &[int(now), DataValue::Text(fingerprint_sha256.to_string())],
            )
            .await
            .map_err(|e| storage_err("revoking a client", e))?;
        if changed > 0 {
            if let Some(client) = self.write_index().get_mut(fingerprint_sha256) {
                client.revoked_at = Some(now);
            }
        }
        Ok(changed > 0)
    }

    /// Record a successful handshake at `now` (Unix seconds).
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the update fails.
    pub async fn mark_seen(&self, fingerprint_sha256: &str, now: u64) -> AgentRpcResult<()> {
        if let Some(client) = self.write_index().get_mut(fingerprint_sha256) {
            client.last_seen_at = Some(now);
        }
        self.storage
            .execute_async(
                "UPDATE agent_rpc_trusted_clients SET last_seen_at = ? WHERE fingerprint = ?",
                &[int(now), DataValue::Text(fingerprint_sha256.to_string())],
            )
            .await
            .map_err(|e| storage_err("recording a client handshake", e))?;
        Ok(())
    }

    /// Spend one enrollment of the bootstrap secret `secret_id`, allowing at
    /// most `max_uses` in total. Returns `false` when the secret is spent.
    /// Rows of secrets that expired before `now` are pruned first.
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the database fails.
    pub async fn consume_secret_use(
        &self,
        secret_id: &str,
        valid_until: u64,
        max_uses: u32,
        now: u64,
    ) -> AgentRpcResult<bool> {
        if max_uses == 0 {
            return Ok(false);
        }
        self.storage
            .execute_async(
                "DELETE FROM agent_rpc_bootstrap_secret_uses WHERE valid_until <= ?",
                &[int(now)],
            )
            .await
            .map_err(|e| storage_err("pruning spent bootstrap secrets", e))?;
        // One statement, so concurrent enrollments with the same secret
        // cannot both take the last use.
        let changed = self
            .storage
            .execute_async(
                "INSERT INTO agent_rpc_bootstrap_secret_uses (secret_id, uses, valid_until) \
                 VALUES (?, 1, ?) \
                 ON CONFLICT(secret_id) DO UPDATE SET uses = uses + 1 WHERE uses < ?",
                &[
                    DataValue::Text(secret_id.to_string()),
                    int(valid_until),
                    DataValue::Integer(i64::from(max_uses)),
                ],
            )
            .await
            .map_err(|e| storage_err("spending a bootstrap secret", e))?;
        Ok(changed > 0)
    }
}
