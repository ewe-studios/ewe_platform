//! The key-value trust store.

use std::sync::Arc;

use foundation_db::KeyValueStore;
use futures_util::lock::Mutex;
use serde::{Deserialize, Serialize};

use super::client::{
    ClientKeyRecord, SecretUse, TrustError, TrustFuture, TrustStore, TrustStoreInfo, TrustedClient,
    TrustedClientStream, UseOutcome,
};

const CLIENT_PREFIX: &str = "trust/v1/client/";
const CLIENT_ID_PREFIX: &str = "trust/v1/client_id/";
const CLIENT_KEY_PREFIX: &str = "trust/v1/client_key/";
const REFRESH_NONCE_PREFIX: &str = "trust/v1/refresh/";
const USE_PREFIX: &str = "trust/v1/use/";
const REVOKED_PREFIX: &str = "trust/v1/revoked/";

/// One stored client and its certificate.
#[derive(Serialize, Deserialize)]
struct KvClient {
    client: TrustedClient,
    cert_pem: String,
}

/// One use row.
#[derive(Serialize, Deserialize)]
struct KvUse {
    uses: u32,
    valid_until: u64,
}

/// One revocation row.
#[derive(Serialize, Deserialize)]
struct KvRevoked {
    valid_until: u64,
}

/// One used refresh nonce.
#[derive(Serialize, Deserialize)]
struct KvNonce {
    seen_at: u64,
}

/// A trust store over any [`KeyValueStore`].
///
/// Use it for a backend without Structured Query Language (SQL): the in-memory
/// store and a JSON file store. Never use it over Cloudflare Workers key-value
/// storage (KV): that store is eventually consistent, so a secret could be
/// spent twice. A Worker uses a Data Manipulation Language (DML) store built on
/// D1 instead.
pub struct KvTrustStore<K: KeyValueStore + ?Sized + 'static> {
    storage: Arc<K>,
    /// Serializes the read-modify-write of one use count inside this process.
    lock: Mutex<()>,
}

impl<K: KeyValueStore + ?Sized + 'static> std::fmt::Debug for KvTrustStore<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KvTrustStore").finish_non_exhaustive()
    }
}

impl<K: KeyValueStore + ?Sized + 'static> KvTrustStore<K> {
    /// A store over `storage`.
    #[must_use]
    pub fn new(storage: Arc<K>) -> Self {
        Self {
            storage,
            lock: Mutex::new(()),
        }
    }

    fn client_key(fingerprint: &str) -> String {
        format!("{CLIENT_PREFIX}{fingerprint}")
    }

    fn client_id_key(client_id: &str) -> String {
        format!("{CLIENT_ID_PREFIX}{client_id}")
    }

    fn use_key(secret_id: &str) -> String {
        format!("{USE_PREFIX}{secret_id}")
    }

    fn client_key_key(client_id: &str) -> String {
        format!("{CLIENT_KEY_PREFIX}{client_id}")
    }

    fn refresh_nonce_key(client_id: &str, nonce: &[u8]) -> String {
        let mut message = client_id.as_bytes().to_vec();
        message.push(0);
        message.extend_from_slice(nonce);
        format!(
            "{REFRESH_NONCE_PREFIX}{}",
            crate::fingerprint::public_fingerprint_hex(&message)
        )
    }

    fn revoked_key(secret_id: &str) -> String {
        format!("{REVOKED_PREFIX}{secret_id}")
    }

    fn storage_error(e: impl std::fmt::Display) -> TrustError {
        TrustError::Storage(e.to_string())
    }
}

impl<K: KeyValueStore + ?Sized + 'static> TrustStore for KvTrustStore<K> {
    fn init(&self) -> TrustFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn clients(&self) -> TrustFuture<'_, TrustedClientStream> {
        Box::pin(async move {
            let keys = self
                .storage
                .list_keys(Some(CLIENT_PREFIX))
                .await
                .map_err(Self::storage_error)?
                .collect_all()
                .await
                .map_err(Self::storage_error)?;
            let mut clients = Vec::with_capacity(keys.len());
            for key in keys {
                if let Some(record) = self
                    .storage
                    .get::<KvClient>(&key)
                    .await
                    .map_err(Self::storage_error)?
                {
                    clients.push(record.client);
                }
            }
            clients.sort_by(|a, b| {
                (a.enrolled_at, &a.fingerprint_sha256).cmp(&(b.enrolled_at, &b.fingerprint_sha256))
            });
            let stream: TrustedClientStream =
                Box::pin(futures_util::stream::iter(clients.into_iter().map(Ok)));
            Ok(stream)
        })
    }

    fn client<'a>(&'a self, fingerprint: &'a str) -> TrustFuture<'a, Option<TrustedClient>> {
        Box::pin(async move {
            Ok(self
                .storage
                .get::<KvClient>(&Self::client_key(fingerprint))
                .await
                .map_err(Self::storage_error)?
                .map(|record| record.client))
        })
    }

    fn client_by_id<'a>(&'a self, client_id: &'a str) -> TrustFuture<'a, Option<TrustedClient>> {
        Box::pin(async move {
            let Some(fingerprint) = self
                .storage
                .get::<String>(&Self::client_id_key(client_id))
                .await
                .map_err(Self::storage_error)?
            else {
                return Ok(None);
            };
            Ok(self
                .storage
                .get::<KvClient>(&Self::client_key(&fingerprint))
                .await
                .map_err(Self::storage_error)?
                .map(|record| record.client))
        })
    }

    fn record_enrollment<'a>(
        &'a self,
        client: &'a TrustedClient,
        cert_pem: &'a str,
    ) -> TrustFuture<'a, ()> {
        Box::pin(async move {
            let key = Self::client_key(&client.fingerprint_sha256);
            if self
                .storage
                .exists(&key)
                .await
                .map_err(Self::storage_error)?
            {
                return Err(TrustError::Duplicate(client.fingerprint_sha256.clone()));
            }
            let id_key = if let Some(client_id) = &client.client_id {
                let id_key = Self::client_id_key(client_id);
                if self
                    .storage
                    .exists(&id_key)
                    .await
                    .map_err(Self::storage_error)?
                {
                    return Err(TrustError::Duplicate(client_id.clone()));
                }
                self.storage
                    .set(&id_key, client.fingerprint_sha256.clone())
                    .await
                    .map_err(Self::storage_error)?;
                Some(id_key)
            } else {
                None
            };
            let stored = self
                .storage
                .set(
                    &key,
                    KvClient {
                        client: client.clone(),
                        cert_pem: cert_pem.to_string(),
                    },
                )
                .await;
            if let Err(error) = stored {
                // Do not leave an ID that names no client.
                if let Some(id_key) = &id_key {
                    self.storage
                        .delete(id_key)
                        .await
                        .map_err(Self::storage_error)?;
                }
                return Err(Self::storage_error(error));
            }
            Ok(())
        })
    }

    fn record_client_key<'a>(
        &'a self,
        client_id: &'a str,
        key: &'a ClientKeyRecord,
    ) -> TrustFuture<'a, ()> {
        Box::pin(async move {
            self.storage
                .set(&Self::client_key_key(client_id), key.clone())
                .await
                .map_err(Self::storage_error)?;
            Ok(())
        })
    }

    fn client_key<'a>(&'a self, client_id: &'a str) -> TrustFuture<'a, Option<ClientKeyRecord>> {
        Box::pin(async move {
            self.storage
                .get::<ClientKeyRecord>(&Self::client_key_key(client_id))
                .await
                .map_err(Self::storage_error)
        })
    }

    fn consume_refresh_nonce<'a>(
        &'a self,
        client_id: &'a str,
        nonce: &'a [u8],
        at: u64,
    ) -> TrustFuture<'a, bool> {
        Box::pin(async move {
            let _guard = self.lock.lock().await;
            let key = Self::refresh_nonce_key(client_id, nonce);
            if self
                .storage
                .exists(&key)
                .await
                .map_err(Self::storage_error)?
            {
                return Ok(false);
            }
            self.storage
                .set(&key, KvNonce { seen_at: at })
                .await
                .map_err(Self::storage_error)?;
            Ok(true)
        })
    }

    fn revoke_client<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, bool> {
        Box::pin(async move {
            let key = Self::client_key(fingerprint);
            let Some(mut record) = self
                .storage
                .get::<KvClient>(&key)
                .await
                .map_err(Self::storage_error)?
            else {
                return Ok(false);
            };
            if record.client.revoked_at.is_some() {
                return Ok(false);
            }
            record.client.revoked_at = Some(at);
            self.storage
                .set(&key, record)
                .await
                .map_err(Self::storage_error)?;
            Ok(true)
        })
    }

    fn mark_seen<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, ()> {
        Box::pin(async move {
            let key = Self::client_key(fingerprint);
            if let Some(mut record) = self
                .storage
                .get::<KvClient>(&key)
                .await
                .map_err(Self::storage_error)?
            {
                record.client.last_seen_at = Some(at);
                self.storage
                    .set(&key, record)
                    .await
                    .map_err(Self::storage_error)?;
            }
            Ok(())
        })
    }

    fn consume_secret_use<'a>(&'a self, one_use: SecretUse<'a>) -> TrustFuture<'a, UseOutcome> {
        Box::pin(async move {
            if one_use.max_uses == 0 || one_use.valid_until <= one_use.now {
                return Ok(UseOutcome::Spent);
            }
            let _guard = self.lock.lock().await;
            let key = Self::use_key(one_use.secret_id);
            let mut record = self
                .storage
                .get::<KvUse>(&key)
                .await
                .map_err(Self::storage_error)?
                .unwrap_or(KvUse {
                    uses: 0,
                    valid_until: one_use.valid_until,
                });
            if record.uses >= one_use.max_uses {
                return Ok(UseOutcome::Spent);
            }
            record.uses += 1;
            record.valid_until = one_use.valid_until;
            let uses = record.uses;
            self.storage
                .set(&key, record)
                .await
                .map_err(Self::storage_error)?;
            Ok(UseOutcome::Counted { uses })
        })
    }

    fn revoke_secret<'a>(
        &'a self,
        secret_id: &'a str,
        valid_until: u64,
        _at: u64,
    ) -> TrustFuture<'a, bool> {
        Box::pin(async move {
            let key = Self::revoked_key(secret_id);
            if self
                .storage
                .exists(&key)
                .await
                .map_err(Self::storage_error)?
            {
                return Ok(false);
            }
            self.storage
                .set(&key, KvRevoked { valid_until })
                .await
                .map_err(Self::storage_error)?;
            Ok(true)
        })
    }

    fn is_secret_revoked(&self, secret_id: &str) -> TrustFuture<'_, bool> {
        let secret_id = secret_id.to_string();
        Box::pin(async move {
            self.storage
                .exists(&Self::revoked_key(&secret_id))
                .await
                .map_err(Self::storage_error)
        })
    }

    fn prune(&self, cutoff: u64) -> TrustFuture<'_, u64> {
        Box::pin(async move {
            let mut removed = 0_u64;
            for (prefix, is_revoked) in [(USE_PREFIX, false), (REVOKED_PREFIX, true)] {
                let keys = self
                    .storage
                    .list_keys(Some(prefix))
                    .await
                    .map_err(Self::storage_error)?
                    .collect_all()
                    .await
                    .map_err(Self::storage_error)?;
                for key in keys {
                    let valid_until = if is_revoked {
                        self.storage
                            .get::<KvRevoked>(&key)
                            .await
                            .map_err(Self::storage_error)?
                            .map(|record| record.valid_until)
                    } else {
                        self.storage
                            .get::<KvUse>(&key)
                            .await
                            .map_err(Self::storage_error)?
                            .map(|record| record.valid_until)
                    };
                    if valid_until.is_some_and(|until| until < cutoff) {
                        self.storage
                            .delete(&key)
                            .await
                            .map_err(Self::storage_error)?;
                        removed += 1;
                    }
                }
            }
            let keys = self
                .storage
                .list_keys(Some(REFRESH_NONCE_PREFIX))
                .await
                .map_err(Self::storage_error)?
                .collect_all()
                .await
                .map_err(Self::storage_error)?;
            for key in keys {
                let seen = self
                    .storage
                    .get::<KvNonce>(&key)
                    .await
                    .map_err(Self::storage_error)?
                    .map(|record| record.seen_at);
                if seen.is_some_and(|seen_at| seen_at < cutoff) {
                    self.storage
                        .delete(&key)
                        .await
                        .map_err(Self::storage_error)?;
                    removed += 1;
                }
            }
            Ok(removed)
        })
    }

    fn info(&self) -> TrustStoreInfo {
        TrustStoreInfo {
            name: "key-value",
            atomic_across_processes: false,
        }
    }
}
