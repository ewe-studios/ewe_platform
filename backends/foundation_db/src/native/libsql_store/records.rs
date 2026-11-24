//! Apply key-value record changes in one database transaction.
use super::{DataValue, LibsqlStore, QueryStore, StorageError, StorageResult};
use serde::Serialize;
use std::collections::BTreeMap;

pub(super) const UPSERT_RECORD: &str = "INSERT INTO kv_store (key, value, created_at, updated_at)
    VALUES (?, ?, ?, ?) ON CONFLICT(key) DO UPDATE
    SET value = excluded.value, updated_at = excluded.updated_at";

impl LibsqlStore {
    /// Set the time SQLite waits for a write lock on another connection.
    ///
    /// # Errors
    /// Return an error for an invalid duration or database connection.
    pub async fn set_busy_timeout(&self, timeout: std::time::Duration) -> StorageResult<()> {
        let milliseconds = i32::try_from(timeout.as_millis()).map_err(|_| {
            StorageError::Backend("SQLite lock timeout exceeds the supported range".into())
        })?;
        self.execute_batch(&format!("PRAGMA busy_timeout={milliseconds}"))
            .await
    }

    /// Read typed records from one database snapshot in key order.
    ///
    /// # Errors
    /// Return query errors before or during iteration. An invalid record produces a decoding error.
    pub async fn records<V: serde::de::DeserializeOwned>(
        &self,
        prefix: &str,
    ) -> StorageResult<impl futures_core::Stream<Item = StorageResult<(String, V)>> + Unpin> {
        use futures_util::StreamExt;
        let rows = self
            .query(
                "SELECT key,value FROM kv_store WHERE substr(key,1,length(?))=? ORDER BY key",
                &[
                    DataValue::Text(prefix.into()),
                    DataValue::Text(prefix.into()),
                ],
            )
            .await?;
        let store = self.clone();
        Ok(rows.map(move |row| {
            let row = row?;
            let key = row.get(0)?;
            let stored = store.maybe_decrypt(&row.get::<String>(1)?)?;
            let value = serde_json::from_str(&stored).map_err(StorageError::Json)?;
            Ok((key, value))
        }))
    }

    /// Add new records, update existing records, and remove confirmed records.
    ///
    /// # Errors
    /// Return the encoding or transaction error. A failed batch keeps the previous records.
    pub async fn change_records<V: Serialize>(
        &self,
        prefix: &str,
        inserts: &BTreeMap<&str, &V>,
        updates: &BTreeMap<&str, &V>,
        removals: &[&str],
    ) -> StorageResult<()> {
        let mut batch = Vec::new();
        let now = DataValue::Integer(self.gate.now_millis()?);
        for (id, value) in inserts {
            batch.push((
                UPSERT_RECORD.into(),
                vec![
                    DataValue::Text(format!("{prefix}{id}")),
                    self.record_value(value)?,
                    now.clone(),
                    now.clone(),
                ],
            ));
        }
        for (id, value) in updates {
            batch.push((
                "UPDATE kv_store SET value=?, updated_at=? WHERE key=?".into(),
                vec![
                    self.record_value(value)?,
                    now.clone(),
                    DataValue::Text(format!("{prefix}{id}")),
                ],
            ));
        }
        for id in removals {
            batch.push((
                "DELETE FROM kv_store WHERE key=?".into(),
                vec![DataValue::Text(format!("{prefix}{id}"))],
            ));
        }
        self.execute_batch_statements(&batch).await
    }

    /// Import each original record once. Keep its receipt after the application removes the record.
    ///
    /// # Errors
    /// Return the encoding or transaction error. A failed import keeps the previous records and receipts.
    pub async fn import_once<V: Serialize>(
        &self,
        destination: &str,
        receipts: &str,
        records: &BTreeMap<String, V>,
    ) -> StorageResult<()> {
        if destination.starts_with(receipts) || receipts.starts_with(destination) {
            return Err(StorageError::Migration(
                "Overlapping record and receipt prefixes".into(),
            ));
        }
        if records.is_empty() {
            return Ok(());
        }
        let mut batch = Vec::new();
        let now = DataValue::Integer(self.gate.now_millis()?);
        let imported = self.record_value(&true)?;
        for (id, value) in records {
            let receipt = DataValue::Text(format!("{receipts}{id}"));
            batch.push((
                "INSERT INTO kv_store (key, value, created_at, updated_at) SELECT ?, ?, ?, ?
                 WHERE NOT EXISTS (SELECT 1 FROM kv_store WHERE key = ?)
                 ON CONFLICT(key) DO NOTHING"
                    .into(),
                vec![
                    DataValue::Text(format!("{destination}{id}")),
                    self.record_value(value)?,
                    now.clone(),
                    now.clone(),
                    receipt.clone(),
                ],
            ));
            batch.push((
                "INSERT INTO kv_store (key, value, created_at, updated_at) VALUES (?, ?, ?, ?)
                 ON CONFLICT(key) DO NOTHING"
                    .into(),
                vec![receipt, imported.clone(), now.clone(), now.clone()],
            ));
        }
        self.execute_batch_statements(&batch).await
    }

    fn record_value<V: Serialize>(&self, value: &V) -> StorageResult<DataValue> {
        let encoded = serde_json::to_string(value).map_err(StorageError::Json)?;
        self.maybe_encrypt(&encoded).map(DataValue::Text)
    }
}
