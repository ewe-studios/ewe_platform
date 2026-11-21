//! The Structured Query Language (SQL) trust store.

use std::path::Path;
use std::sync::Arc;

use foundation_db::{DataValue, QueryStore, SqlRow};
use futures_util::StreamExt;

use super::client::{
    SecretUse, TrustError, TrustFuture, TrustStore, TrustStoreInfo, TrustedClient,
    TrustedClientStream, UseOutcome,
};

/// The default table name prefix. It reads the databases that the agent RPC
/// already wrote, so a running host keeps its enrolled clients.
pub const DEFAULT_TABLE_PREFIX: &str = "agent_rpc_";

/// A trust store over any [`QueryStore`]: Turso, SQLite, libSQL or D1.
pub struct SqlTrustStore<Q: QueryStore + ?Sized + 'static> {
    storage: Arc<Q>,
    prefix: String,
}

impl<Q: QueryStore + ?Sized + 'static> std::fmt::Debug for SqlTrustStore<Q> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlTrustStore")
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

impl<Q: QueryStore + ?Sized + 'static> SqlTrustStore<Q> {
    /// A store over `storage` with a table name prefix.
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the prefix holds a byte outside
    /// `[A-Za-z0-9_]`.
    pub fn new(storage: Arc<Q>, prefix: impl Into<String>) -> Result<Self, TrustError> {
        let prefix = prefix.into();
        if !prefix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(TrustError::Storage(
                "a table name prefix must be [A-Za-z0-9_]".to_string(),
            ));
        }
        Ok(Self { storage, prefix })
    }

    /// The table name prefix.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    fn table(&self, name: &str) -> String {
        format!("{}{name}", self.prefix)
    }

    fn schema(&self) -> String {
        let clients = self.table("trusted_clients");
        let uses = self.table("bootstrap_secret_uses");
        let revoked = self.table("revoked_secrets");
        format!(
            "CREATE TABLE IF NOT EXISTS {clients} (
                fingerprint TEXT PRIMARY KEY,
                client_name TEXT NOT NULL,
                cert_pem TEXT NOT NULL,
                enrolled_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                last_seen_at INTEGER,
                revoked_at INTEGER
            );
            CREATE TABLE IF NOT EXISTS {uses} (
                secret_id TEXT PRIMARY KEY,
                uses INTEGER NOT NULL,
                valid_until INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS {revoked} (
                secret_id TEXT PRIMARY KEY,
                revoked_at INTEGER NOT NULL,
                valid_until INTEGER NOT NULL
            );"
        )
    }

    fn storage_error(e: impl std::fmt::Display) -> TrustError {
        TrustError::Storage(e.to_string())
    }

    fn row_client(row: &SqlRow) -> Result<TrustedClient, TrustError> {
        Ok(TrustedClient {
            fingerprint_sha256: row.get(0).map_err(Self::storage_error)?,
            client_name: row.get(1).map_err(Self::storage_error)?,
            enrolled_at: uint(row, 2)?,
            expires_at: uint(row, 3)?,
            last_seen_at: opt_uint(row, 4)?,
            revoked_at: opt_uint(row, 5)?,
        })
    }

    async fn uses(&self, secret_id: &str) -> Result<u32, TrustError> {
        let sql = format!(
            "SELECT uses FROM {} WHERE secret_id = ?",
            self.table("bootstrap_secret_uses")
        );
        let mut rows = self
            .storage
            .query(&sql, &[DataValue::Text(secret_id.to_string())])
            .await
            .map_err(Self::storage_error)?;
        match rows.next().await {
            Some(row) => {
                let row = row.map_err(Self::storage_error)?;
                let uses: i64 = row.get(0).map_err(Self::storage_error)?;
                Ok(u32::try_from(uses).unwrap_or(u32::MAX))
            }
            None => Ok(0),
        }
    }
}

#[cfg(feature = "turso")]
impl SqlTrustStore<foundation_db::TursoStorage> {
    /// Open (creating when needed) a SQLite database at `path` through the
    /// Turso backend, with [`DEFAULT_TABLE_PREFIX`].
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the directory or the database cannot be
    /// opened.
    pub async fn open_sqlite(
        path: impl AsRef<Path>,
        ctx: foundation_context::Ctx,
    ) -> Result<Self, TrustError> {
        let path = path.as_ref();
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)
                .map_err(|e| TrustError::Storage(format!("creating {}: {e}", dir.display())))?;
        }
        let url = path
            .to_str()
            .ok_or_else(|| TrustError::Storage("the database path is not UTF-8".to_string()))?;
        let storage = foundation_db::TursoStorage::open(url, ctx)
            .await
            .map_err(Self::storage_error)?;
        Self::new(Arc::new(storage), DEFAULT_TABLE_PREFIX)
    }
}

impl<Q: QueryStore + ?Sized + 'static> TrustStore for SqlTrustStore<Q> {
    fn init(&self) -> TrustFuture<'_, ()> {
        Box::pin(async move {
            self.storage
                .execute_batch(&self.schema())
                .await
                .map_err(Self::storage_error)
        })
    }

    fn clients(&self) -> TrustFuture<'_, TrustedClientStream> {
        Box::pin(async move {
            let sql = format!(
                "SELECT fingerprint, client_name, enrolled_at, expires_at, last_seen_at, revoked_at \
                 FROM {} ORDER BY enrolled_at, fingerprint",
                self.table("trusted_clients")
            );
            let rows = self
                .storage
                .query(&sql, &[])
                .await
                .map_err(Self::storage_error)?;
            let stream: TrustedClientStream = Box::pin(rows.map(|row| {
                let row = row.map_err(Self::storage_error)?;
                Self::row_client(&row)
            }));
            Ok(stream)
        })
    }

    fn client<'a>(&'a self, fingerprint: &'a str) -> TrustFuture<'a, Option<TrustedClient>> {
        Box::pin(async move {
            let sql = format!(
                "SELECT fingerprint, client_name, enrolled_at, expires_at, last_seen_at, revoked_at \
                 FROM {} WHERE fingerprint = ?",
                self.table("trusted_clients")
            );
            let mut rows = self
                .storage
                .query(&sql, &[DataValue::Text(fingerprint.to_string())])
                .await
                .map_err(Self::storage_error)?;
            match rows.next().await {
                Some(row) => {
                    let row = row.map_err(Self::storage_error)?;
                    Ok(Some(Self::row_client(&row)?))
                }
                None => Ok(None),
            }
        })
    }

    fn record_enrollment<'a>(
        &'a self,
        client: &'a TrustedClient,
        cert_pem: &'a str,
    ) -> TrustFuture<'a, ()> {
        Box::pin(async move {
            let sql = format!(
                "INSERT INTO {} \
                 (fingerprint, client_name, cert_pem, enrolled_at, expires_at, last_seen_at, revoked_at) \
                 VALUES (?, ?, ?, ?, ?, NULL, NULL)",
                self.table("trusted_clients")
            );
            let result = self
                .storage
                .execute(
                    &sql,
                    &[
                        DataValue::Text(client.fingerprint_sha256.clone()),
                        DataValue::Text(client.client_name.clone()),
                        DataValue::Text(cert_pem.to_string()),
                        int(client.enrolled_at),
                        int(client.expires_at),
                    ],
                )
                .await;
            match result {
                Ok(_) => Ok(()),
                Err(e) => {
                    if self.client(&client.fingerprint_sha256).await?.is_some() {
                        Err(TrustError::Duplicate(client.fingerprint_sha256.clone()))
                    } else {
                        Err(Self::storage_error(e))
                    }
                }
            }
        })
    }

    fn revoke_client<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, bool> {
        Box::pin(async move {
            let sql = format!(
                "UPDATE {} SET revoked_at = ? WHERE fingerprint = ? AND revoked_at IS NULL",
                self.table("trusted_clients")
            );
            let changed = self
                .storage
                .execute(&sql, &[int(at), DataValue::Text(fingerprint.to_string())])
                .await
                .map_err(Self::storage_error)?;
            Ok(changed > 0)
        })
    }

    fn mark_seen<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, ()> {
        Box::pin(async move {
            let sql = format!(
                "UPDATE {} SET last_seen_at = ? WHERE fingerprint = ?",
                self.table("trusted_clients")
            );
            self.storage
                .execute(&sql, &[int(at), DataValue::Text(fingerprint.to_string())])
                .await
                .map_err(Self::storage_error)?;
            Ok(())
        })
    }

    fn consume_secret_use<'a>(&'a self, one_use: SecretUse<'a>) -> TrustFuture<'a, UseOutcome> {
        Box::pin(async move {
            if one_use.max_uses == 0 || one_use.valid_until <= one_use.now {
                return Ok(UseOutcome::Spent);
            }
            // One statement, so two concurrent calls with the last free use
            // cannot both take it. This is atomic on SQLite, Turso, libSQL and
            // D1.
            let sql = format!(
                "INSERT INTO {} (secret_id, uses, valid_until) VALUES (?, 1, ?) \
                 ON CONFLICT(secret_id) DO UPDATE SET uses = uses + 1 WHERE uses < ?",
                self.table("bootstrap_secret_uses")
            );
            let changed = self
                .storage
                .execute(
                    &sql,
                    &[
                        DataValue::Text(one_use.secret_id.to_string()),
                        int(one_use.valid_until),
                        DataValue::Integer(i64::from(one_use.max_uses)),
                    ],
                )
                .await
                .map_err(Self::storage_error)?;
            if changed == 0 {
                return Ok(UseOutcome::Spent);
            }
            Ok(UseOutcome::Counted {
                uses: self.uses(one_use.secret_id).await?,
            })
        })
    }

    fn revoke_secret<'a>(
        &'a self,
        secret_id: &'a str,
        valid_until: u64,
        at: u64,
    ) -> TrustFuture<'a, bool> {
        Box::pin(async move {
            let sql = format!(
                "INSERT INTO {} (secret_id, revoked_at, valid_until) VALUES (?, ?, ?) \
                 ON CONFLICT(secret_id) DO NOTHING",
                self.table("revoked_secrets")
            );
            let changed = self
                .storage
                .execute(
                    &sql,
                    &[
                        DataValue::Text(secret_id.to_string()),
                        int(at),
                        int(valid_until),
                    ],
                )
                .await
                .map_err(Self::storage_error)?;
            Ok(changed > 0)
        })
    }

    fn is_secret_revoked(&self, secret_id: &str) -> TrustFuture<'_, bool> {
        let secret_id = secret_id.to_string();
        Box::pin(async move {
            let sql = format!(
                "SELECT secret_id FROM {} WHERE secret_id = ?",
                self.table("revoked_secrets")
            );
            let mut rows = self
                .storage
                .query(&sql, &[DataValue::Text(secret_id)])
                .await
                .map_err(Self::storage_error)?;
            Ok(rows.next().await.is_some())
        })
    }

    fn prune(&self, cutoff: u64) -> TrustFuture<'_, u64> {
        Box::pin(async move {
            let mut removed = 0_u64;
            for table in ["bootstrap_secret_uses", "revoked_secrets"] {
                let sql = format!("DELETE FROM {} WHERE valid_until < ?", self.table(table));
                removed += self
                    .storage
                    .execute(&sql, &[int(cutoff)])
                    .await
                    .map_err(Self::storage_error)?;
            }
            Ok(removed)
        })
    }

    fn info(&self) -> TrustStoreInfo {
        TrustStoreInfo {
            name: "sql",
            atomic_across_processes: true,
        }
    }
}

fn int(value: u64) -> DataValue {
    DataValue::Integer(i64::try_from(value).unwrap_or(i64::MAX))
}

fn uint(row: &SqlRow, index: usize) -> Result<u64, TrustError> {
    let value: i64 = row
        .get(index)
        .map_err(SqlTrustStore::<dyn QueryStore>::storage_error)?;
    Ok(u64::try_from(value).unwrap_or(0))
}

fn opt_uint(row: &SqlRow, index: usize) -> Result<Option<u64>, TrustError> {
    let value: Option<i64> = row
        .get(index)
        .map_err(SqlTrustStore::<dyn QueryStore>::storage_error)?;
    Ok(value.map(|value| u64::try_from(value).unwrap_or(0)))
}
