//! The trust store plus an in-memory index.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError};

use foundation_compact::Instant;
use foundation_context::sync::{Mutex, RwLock};
use foundation_context::{Clock, Ctx};
use futures_util::StreamExt;

use super::client::{SecretUse, TrustError, TrustStore, TrustedClient, UseOutcome};
use super::config::{IndexMode, TrustConfig};

/// A cheap handle to the in-memory client map, for the synchronous Transport
/// Layer Security (TLS) verifier.
#[derive(Clone, Debug, Default)]
pub struct TrustIndex(Arc<RwLock<HashMap<String, TrustedClient>>>);

impl TrustIndex {
    /// A read lock and a clone. No Input/Output (I/O), no task, no `await`.
    #[must_use]
    pub fn get(&self, fingerprint: &str) -> Option<TrustedClient> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(fingerprint)
            .cloned()
    }

    /// Add or replace a client.
    pub fn insert(&self, client: TrustedClient) {
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(client.fingerprint_sha256.clone(), client);
    }

    /// The number of clients in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.read().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Whether the index holds no client.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn update_last_seen(&self, fingerprint: &str, at: u64) {
        if let Some(client) = self
            .0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(fingerprint)
        {
            client.last_seen_at = Some(at);
        }
    }

    pub(crate) fn set_revoked(&self, fingerprint: &str, at: u64) {
        if let Some(client) = self
            .0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(fingerprint)
        {
            client.revoked_at = Some(at);
        }
    }

    fn replace(&self, clients: HashMap<String, TrustedClient>) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = clients;
    }
}

/// The clients that the Transport Layer Security (TLS) verifier and the auth
/// providers check.
pub struct TrustedClients {
    store: Arc<dyn TrustStore>,
    index: TrustIndex,
    clock: Arc<dyn Clock>,
    config: TrustConfig,
    last_refresh: Mutex<Option<Instant>>,
}

impl std::fmt::Debug for TrustedClients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrustedClients")
            .field("store", &self.store.info().name)
            .field("index", &self.index)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl TrustedClients {
    /// Initialize the store and fill the index, one row at a time.
    ///
    /// # Errors
    /// [`TrustError::NotAtomic`] when a shared store cannot count uses safely
    /// across processes, and [`TrustError::Storage`] when the store fails.
    pub async fn open(
        store: Arc<dyn TrustStore>,
        ctx: &Ctx,
        config: TrustConfig,
    ) -> Result<Arc<Self>, TrustError> {
        if config.shared_store && !store.info().atomic_across_processes {
            return Err(TrustError::NotAtomic);
        }
        store.init().await?;
        let trusted = Arc::new(Self {
            store,
            index: TrustIndex::default(),
            clock: ctx.clock(),
            config,
            last_refresh: Mutex::new(None),
        });
        if trusted.config.index != IndexMode::StoreOnly {
            trusted.reload().await?;
        }
        Ok(trusted)
    }

    /// The index handle for the Transport Layer Security (TLS) verifier.
    #[must_use]
    pub fn index(&self) -> TrustIndex {
        self.index.clone()
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &TrustConfig {
        &self.config
    }

    /// The store.
    #[must_use]
    pub fn store(&self) -> &Arc<dyn TrustStore> {
        &self.store
    }

    /// Reload the whole index from the store.
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the store fails.
    pub async fn reload(&self) -> Result<(), TrustError> {
        let mut clients = HashMap::new();
        let mut stream = self.store.clients().await?;
        while let Some(client) = stream.next().await {
            let client = client?;
            clients.insert(client.fingerprint_sha256.clone(), client);
        }
        drop(stream);
        self.index.replace(clients);
        *self
            .last_refresh
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(self.clock.now());
        Ok(())
    }

    /// Every enrolled client, oldest enrollment first. A snapshot for an
    /// operator view; the trust decision uses [`TrustedClients::get`].
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the store read fails.
    pub async fn clients(&self) -> Result<Vec<TrustedClient>, TrustError> {
        if self.config.index == IndexMode::StoreOnly {
            let mut clients = Vec::new();
            let mut stream = self.store.clients().await?;
            while let Some(client) = stream.next().await {
                clients.push(client?);
            }
            clients.sort_by(|a, b| {
                (a.enrolled_at, &a.fingerprint_sha256).cmp(&(b.enrolled_at, &b.fingerprint_sha256))
            });
            return Ok(clients);
        }
        let mut clients: Vec<TrustedClient> = self
            .index
            .0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        clients.sort_by(|a, b| {
            (a.enrolled_at, &a.fingerprint_sha256).cmp(&(b.enrolled_at, &b.fingerprint_sha256))
        });
        Ok(clients)
    }

    /// The client, from the index or (in [`IndexMode::StoreOnly`]) from the
    /// store.
    ///
    /// # Errors
    /// [`TrustError::Storage`] when a store read fails.
    pub async fn get(&self, fingerprint: &str) -> Result<Option<TrustedClient>, TrustError> {
        match self.config.index {
            IndexMode::StoreOnly => self.store.client(fingerprint).await,
            IndexMode::Refresh => {
                if self.is_stale(self.config.cache_ttl) {
                    self.reload().await?;
                }
                Ok(self.index.get(fingerprint))
            }
            IndexMode::Preload => {
                if self.config.shared_store && self.is_stale(self.config.revocation_ttl) {
                    self.reload().await?;
                }
                Ok(self.index.get(fingerprint))
            }
        }
    }

    /// Record a newly issued client certificate. The store is written first;
    /// the index changes only after the write succeeds.
    ///
    /// # Errors
    /// [`TrustError::Storage`] and [`TrustError::Duplicate`].
    pub async fn record_enrollment(
        &self,
        client: TrustedClient,
        cert_pem: &str,
    ) -> Result<(), TrustError> {
        self.store.record_enrollment(&client, cert_pem).await?;
        self.index.insert(client);
        Ok(())
    }

    /// Revoke a client: the store first, then the index.
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the store fails.
    pub async fn revoke_client(&self, fingerprint: &str) -> Result<bool, TrustError> {
        let now = self.now();
        let changed = self.store.revoke_client(fingerprint, now).await?;
        if changed {
            self.index.set_revoked(fingerprint, now);
        }
        Ok(changed)
    }

    /// Record a seen client at most once per [`TrustConfig::touch_interval`].
    ///
    /// Store errors reach the caller; the method never only logs them.
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the store fails.
    pub async fn touch(&self, fingerprint: &str) -> Result<(), TrustError> {
        let now = self.now();
        let due = self.index.get(fingerprint).is_none_or(|client| {
            client
                .last_seen_at
                .is_none_or(|seen| now.saturating_sub(seen) >= self.config.touch_interval.as_secs())
        });
        if !due {
            return Ok(());
        }
        self.store.mark_seen(fingerprint, now).await?;
        self.index.update_last_seen(fingerprint, now);
        Ok(())
    }

    /// Spend one use of a verified secret.
    ///
    /// # Errors
    /// [`TrustError::Spent`] when no use is left, and [`TrustError::Storage`]
    /// when the store fails.
    pub async fn spend_secret(
        &self,
        secret_id: &str,
        valid_until: u64,
        max_uses: u32,
    ) -> Result<(), TrustError> {
        let outcome = self
            .store
            .consume_secret_use(SecretUse {
                secret_id,
                valid_until,
                max_uses,
                now: self.now(),
            })
            .await?;
        match outcome {
            UseOutcome::Counted { .. } => Ok(()),
            UseOutcome::Spent => Err(TrustError::Spent),
        }
    }

    /// Check that the secret is not revoked.
    ///
    /// # Errors
    /// [`TrustError::SecretRevoked`] when it is, and [`TrustError::Storage`]
    /// when the store fails.
    pub async fn check_secret_not_revoked(&self, secret_id: &str) -> Result<(), TrustError> {
        if self.store.is_secret_revoked(secret_id).await? {
            return Err(TrustError::SecretRevoked);
        }
        Ok(())
    }

    /// Revoke a secret until its validity window ends.
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the store fails.
    pub async fn revoke_secret(
        &self,
        secret_id: &str,
        valid_until: u64,
    ) -> Result<bool, TrustError> {
        self.store
            .revoke_secret(secret_id, valid_until, self.now())
            .await
    }

    /// Delete use and revocation rows whose secret expired before the grace
    /// period. A host runs it on [`TrustConfig::prune_interval`].
    ///
    /// # Errors
    /// [`TrustError::Storage`] when the store fails.
    pub async fn prune(&self) -> Result<u64, TrustError> {
        let cutoff = self
            .now()
            .saturating_sub(self.config.expired_grace.as_secs());
        self.store.prune(cutoff).await
    }

    fn now(&self) -> u64 {
        u64::try_from(foundation_compact::time::unix_seconds(
            self.clock.wall_now(),
        ))
        .unwrap_or(0)
    }

    fn is_stale(&self, age: foundation_compact::Duration) -> bool {
        let last = *self
            .last_refresh
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        last.is_none_or(|last| last.elapsed() >= age)
    }
}
