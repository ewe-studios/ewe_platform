use foundation_core::valtron::Stream;
use foundation_db::traits::{QueryStore, StorageItemStream};
use foundation_db::{DataValue, SqlRow, StorageError};

use crate::core::errors::CedarError;
use super::traits::PolicyStore;

fn collect_first_row(stream: StorageItemStream<'_, SqlRow>) -> Result<Option<SqlRow>, StorageError> {
    for item in stream {
        if let Stream::Next(result) = item {
            return result.map(Some);
        }
    }
    Ok(None)
}

pub const CREATE_TABLE_SQL: &str = "\
CREATE TABLE IF NOT EXISTS cedar_policies (
    namespace TEXT PRIMARY KEY,
    policy_text TEXT NOT NULL,
    schema_text TEXT NOT NULL,
    entities_json TEXT,
    version TEXT NOT NULL DEFAULT '1'
)";

pub struct SqlPolicyStore<'a> {
    store: &'a dyn QueryStore,
    namespace: String,
}

impl<'a> SqlPolicyStore<'a> {
    #[must_use]
    pub fn new(store: &'a dyn QueryStore, namespace: &str) -> Self {
        Self {
            store,
            namespace: namespace.to_string(),
        }
    }

    pub fn ensure_table(&self) -> Result<(), CedarError> {
        let stream = self
            .store
            .execute_batch(CREATE_TABLE_SQL)
            .map_err(|e| CedarError::PolicyParse(format!("SQL table creation failed: {e}")))?;
        for item in stream {
            if let Stream::Next(result) = item {
                result.map_err(|e| CedarError::PolicyParse(format!("SQL exec error: {e}")))?;
            }
        }
        Ok(())
    }

    fn load_row(&self) -> Result<SqlRow, CedarError> {
        let stream = self
            .store
            .query(
                "SELECT policy_text, schema_text, entities_json, version FROM cedar_policies WHERE namespace = ?",
                &[DataValue::Text(self.namespace.clone())],
            )
            .map_err(|e| CedarError::PolicyParse(format!("SQL query error: {e}")))?;

        collect_first_row(stream)
            .map_err(|e| CedarError::PolicyParse(format!("SQL read error: {e}")))?
            .ok_or_else(|| {
                CedarError::PolicyParse(format!(
                    "No policies found for namespace '{}'",
                    self.namespace
                ))
            })
    }
}

impl PolicyStore for SqlPolicyStore<'_> {
    fn load_policies(&self) -> Result<String, CedarError> {
        let row = self.load_row()?;
        row.get_by_name::<String>("policy_text")
            .map_err(|e| CedarError::PolicyParse(format!("Column read error: {e}")))
    }

    fn load_schema(&self) -> Result<String, CedarError> {
        let row = self.load_row()?;
        row.get_by_name::<String>("schema_text")
            .map_err(|e| CedarError::PolicyParse(format!("Column read error: {e}")))
    }

    fn load_entities(&self) -> Result<Option<String>, CedarError> {
        let row = self.load_row()?;
        let val: String = row
            .get_by_name::<String>("entities_json")
            .map_err(|e| CedarError::PolicyParse(format!("Column read error: {e}")))?;
        if val.is_empty() {
            Ok(None)
        } else {
            Ok(Some(val))
        }
    }

    fn version(&self) -> Result<String, CedarError> {
        let row = self.load_row()?;
        row.get_by_name::<String>("version")
            .map_err(|e| CedarError::PolicyParse(format!("Column read error: {e}")))
    }
}

impl core::fmt::Debug for SqlPolicyStore<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SqlPolicyStore")
            .field("namespace", &self.namespace)
            .finish()
    }
}
