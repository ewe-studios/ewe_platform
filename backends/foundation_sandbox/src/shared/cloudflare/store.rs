//! The record store of one Durable Object (DO).
//!
//! The truth is the DO's own storage. The index is a view: an index error is
//! logged and does not fail the call.

use std::sync::Arc;

use crate::shared::cloudflare::{DoAlarm, DoStorage, SandboxIndex};
use crate::shared::control::{RecordStore, SandboxInfo};
use crate::shared::driver::StoredRecord;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::ids::SandboxId;
use crate::shared::BoxFuture;

/// The storage key of the one sandbox record.
pub const SANDBOX_KEY: &str = "sandbox";

/// A record store over the DO key-value store and the Worker-wide index.
#[derive(Clone)]
pub struct DoRecordStore {
    storage: Arc<dyn DoStorage>,
    index: Arc<dyn SandboxIndex>,
    alarm: Arc<dyn DoAlarm>,
}

impl DoRecordStore {
    /// A store over the DO storage, the index and the DO alarm.
    #[must_use]
    pub fn new(
        storage: Arc<dyn DoStorage>,
        index: Arc<dyn SandboxIndex>,
        alarm: Arc<dyn DoAlarm>,
    ) -> Self {
        Self {
            storage,
            index,
            alarm,
        }
    }

    /// The index entry of one stored record.
    fn info(record: &StoredRecord) -> SandboxInfo {
        SandboxInfo {
            id: record.id.to_string(),
            driver: record.driver,
            state: record.state,
            created_unix_ms: record.created_unix_ms,
            expires_at_unix_ms: record.expires_at_unix_ms,
            slave: None,
        }
    }
}

impl RecordStore for DoRecordStore {
    fn put(&self, record: StoredRecord) -> BoxFuture<'_, SandboxResult<()>> {
        let storage = Arc::clone(&self.storage);
        let index = Arc::clone(&self.index);
        Box::pin(async move {
            let value = serde_json::to_vec(&record).map_err(|error| SandboxError::Io {
                op: "do storage".to_string(),
                message: error.to_string(),
            })?;
            storage.put(SANDBOX_KEY, value).await?;
            if let Err(error) = index.put(Self::info(&record)).await {
                tracing::warn!(
                    error = %error,
                    id = %record.id,
                    "the index did not keep the sandbox"
                );
            }
            Ok(())
        })
    }

    fn remove(&self, id: &SandboxId) -> BoxFuture<'_, SandboxResult<()>> {
        let storage = Arc::clone(&self.storage);
        let index = Arc::clone(&self.index);
        let alarm = Arc::clone(&self.alarm);
        let id = id.clone();
        Box::pin(async move {
            storage.delete(SANDBOX_KEY).await?;
            if let Err(error) = index.remove(&id).await {
                tracing::warn!(error = %error, id = %id, "the index did not remove the sandbox");
            }
            alarm.clear().await?;
            Ok(())
        })
    }

    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<StoredRecord>>> {
        let storage = Arc::clone(&self.storage);
        Box::pin(async move {
            let Some(value) = storage.get(SANDBOX_KEY).await? else {
                return Ok(Vec::new());
            };
            let record = serde_json::from_slice::<StoredRecord>(&value).map_err(|error| {
                SandboxError::Io {
                    op: "do storage".to_string(),
                    message: error.to_string(),
                }
            })?;
            Ok(vec![record])
        })
    }
}
