//! The in-memory trust store.

use std::collections::HashMap;
use std::sync::PoisonError;

use foundation_context::sync::{Mutex, RwLock};
use futures_util::stream;

use super::client::{
    SecretUse, TrustFuture, TrustStore, TrustStoreInfo, TrustedClient, TrustedClientStream,
    UseOutcome,
};

/// One stored client and its certificate. The store keeps the certificate for
/// the record; the trust-store trait never returns it.
struct MemoryClient {
    client: TrustedClient,
    _cert_pem: String,
}

/// One use row.
#[derive(Clone, Copy)]
struct UseRecord {
    uses: u32,
    valid_until: u64,
}

/// One revocation row.
#[derive(Clone, Copy)]
struct RevocationRecord {
    valid_until: u64,
}

/// A trust store that lives in the process memory.
///
/// Use it for a test and for a short-lived process. It does not survive a
/// restart.
#[derive(Default)]
pub struct MemoryTrustStore {
    clients: RwLock<HashMap<String, MemoryClient>>,
    uses: Mutex<HashMap<String, UseRecord>>,
    revoked: Mutex<HashMap<String, RevocationRecord>>,
}

impl std::fmt::Debug for MemoryTrustStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryTrustStore")
            .field("clients", &self.clients.read().map_or(0, |map| map.len()))
            .finish_non_exhaustive()
    }
}

impl MemoryTrustStore {
    /// A new, empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl TrustStore for MemoryTrustStore {
    fn init(&self) -> TrustFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn clients(&self) -> TrustFuture<'_, TrustedClientStream> {
        Box::pin(async move {
            let mut clients: Vec<TrustedClient> = self
                .clients
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .values()
                .map(|stored| stored.client.clone())
                .collect();
            clients.sort_by(|a, b| {
                (a.enrolled_at, &a.fingerprint_sha256).cmp(&(b.enrolled_at, &b.fingerprint_sha256))
            });
            let stream: TrustedClientStream = Box::pin(stream::iter(clients.into_iter().map(Ok)));
            Ok(stream)
        })
    }

    fn client<'a>(&'a self, fingerprint: &'a str) -> TrustFuture<'a, Option<TrustedClient>> {
        Box::pin(async move {
            Ok(self
                .clients
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .get(fingerprint)
                .map(|stored| stored.client.clone()))
        })
    }

    fn record_enrollment<'a>(
        &'a self,
        client: &'a TrustedClient,
        cert_pem: &'a str,
    ) -> TrustFuture<'a, ()> {
        Box::pin(async move {
            let mut clients = self.clients.write().unwrap_or_else(|e| e.into_inner());
            if clients.contains_key(&client.fingerprint_sha256) {
                return Err(super::client::TrustError::Duplicate(
                    client.fingerprint_sha256.clone(),
                ));
            }
            clients.insert(
                client.fingerprint_sha256.clone(),
                MemoryClient {
                    client: client.clone(),
                    _cert_pem: cert_pem.to_string(),
                },
            );
            Ok(())
        })
    }

    fn revoke_client<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, bool> {
        Box::pin(async move {
            let mut clients = self.clients.write().unwrap_or_else(|e| e.into_inner());
            match clients.get_mut(fingerprint) {
                Some(stored) if stored.client.revoked_at.is_none() => {
                    stored.client.revoked_at = Some(at);
                    Ok(true)
                }
                _ => Ok(false),
            }
        })
    }

    fn mark_seen<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, ()> {
        Box::pin(async move {
            if let Some(stored) = self
                .clients
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .get_mut(fingerprint)
            {
                stored.client.last_seen_at = Some(at);
            }
            Ok(())
        })
    }

    fn consume_secret_use<'a>(&'a self, one_use: SecretUse<'a>) -> TrustFuture<'a, UseOutcome> {
        Box::pin(async move {
            if one_use.max_uses == 0 || one_use.valid_until <= one_use.now {
                return Ok(UseOutcome::Spent);
            }
            let mut uses = self.uses.lock().unwrap_or_else(|e| e.into_inner());
            let record = uses
                .entry(one_use.secret_id.to_string())
                .or_insert(UseRecord {
                    uses: 0,
                    valid_until: one_use.valid_until,
                });
            if record.uses >= one_use.max_uses {
                return Ok(UseOutcome::Spent);
            }
            record.uses += 1;
            record.valid_until = one_use.valid_until;
            Ok(UseOutcome::Counted { uses: record.uses })
        })
    }

    fn revoke_secret<'a>(
        &'a self,
        secret_id: &'a str,
        valid_until: u64,
        _at: u64,
    ) -> TrustFuture<'a, bool> {
        Box::pin(async move {
            let mut revoked = self.revoked.lock().unwrap_or_else(|e| e.into_inner());
            if revoked.contains_key(secret_id) {
                return Ok(false);
            }
            revoked.insert(secret_id.to_string(), RevocationRecord { valid_until });
            Ok(true)
        })
    }

    fn is_secret_revoked(&self, secret_id: &str) -> TrustFuture<'_, bool> {
        let secret_id = secret_id.to_string();
        Box::pin(async move {
            Ok(self
                .revoked
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains_key(&secret_id))
        })
    }

    fn prune(&self, cutoff: u64) -> TrustFuture<'_, u64> {
        Box::pin(async move {
            let mut removed = 0_u64;
            {
                let mut uses = self.uses.lock().unwrap_or_else(|e| e.into_inner());
                let before = uses.len();
                uses.retain(|_, record| record.valid_until >= cutoff);
                removed += u64::try_from(before - uses.len()).unwrap_or(0);
            }
            {
                let mut revoked = self.revoked.lock().unwrap_or_else(|e| e.into_inner());
                let before = revoked.len();
                revoked.retain(|_, record| record.valid_until >= cutoff);
                removed += u64::try_from(before - revoked.len()).unwrap_or(0);
            }
            Ok(removed)
        })
    }

    fn info(&self) -> TrustStoreInfo {
        TrustStoreInfo {
            name: "memory",
            atomic_across_processes: false,
        }
    }
}
