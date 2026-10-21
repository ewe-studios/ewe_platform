use foundation_db::{AsyncQueryStore, DataValue, SqlRow};
use futures_lite::StreamExt;

use super::traits::PolicyStore;
use crate::core::errors::CedarError;

pub const CREATE_TABLE_SQL: &str = "\
CREATE TABLE IF NOT EXISTS cedar_policies (
    namespace TEXT PRIMARY KEY,
    policy_text TEXT NOT NULL,
    schema_text TEXT NOT NULL,
    entities_json TEXT,
    version TEXT NOT NULL DEFAULT '1'
)";

/// Policies, schema, entities and version kept in one `cedar_policies` row
/// per namespace, read through an async SQL store.
pub struct SqlPolicyStore<'a> {
    store: &'a dyn AsyncQueryStore,
    namespace: String,
}

impl<'a> SqlPolicyStore<'a> {
    #[must_use]
    pub fn new(store: &'a dyn AsyncQueryStore, namespace: &str) -> Self {
        Self {
            store,
            namespace: namespace.to_string(),
        }
    }

    /// Create the `cedar_policies` table if it doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns [`CedarError`] if the statement fails.
    pub async fn ensure_table(&self) -> Result<(), CedarError> {
        self.store
            .execute_batch_async(CREATE_TABLE_SQL)
            .await
            .map_err(|e| CedarError::PolicyParse(format!("SQL table creation failed: {e}")))
    }

    async fn load_row(&self) -> Result<SqlRow, CedarError> {
        let mut stream = self
            .store
            .query_async(
                "SELECT policy_text, schema_text, entities_json, version FROM cedar_policies WHERE namespace = ?",
                &[DataValue::Text(self.namespace.clone())],
            )
            .await
            .map_err(|e| CedarError::PolicyParse(format!("SQL query error: {e}")))?;

        match stream.next().await {
            Some(row) => row.map_err(|e| CedarError::PolicyParse(format!("SQL read error: {e}"))),
            None => Err(CedarError::PolicyParse(format!(
                "No policies found for namespace '{}'",
                self.namespace
            ))),
        }
    }

    async fn load_column(&self, column: &str) -> Result<String, CedarError> {
        self.load_row()
            .await?
            .get_by_name::<String>(column)
            .map_err(|e| CedarError::PolicyParse(format!("Column read error: {e}")))
    }
}

#[async_trait::async_trait]
impl PolicyStore for SqlPolicyStore<'_> {
    async fn load_policies(&self) -> Result<String, CedarError> {
        self.load_column("policy_text").await
    }

    async fn load_schema(&self) -> Result<String, CedarError> {
        self.load_column("schema_text").await
    }

    async fn load_entities(&self) -> Result<Option<String>, CedarError> {
        // `entities_json` is nullable; NULL and the empty string both mean
        // "no entities".
        let val: Option<String> = self
            .load_row()
            .await?
            .get_by_name::<Option<String>>("entities_json")
            .map_err(|e| CedarError::PolicyParse(format!("Column read error: {e}")))?;
        Ok(val.filter(|v| !v.is_empty()))
    }

    async fn version(&self) -> Result<String, CedarError> {
        self.load_column("version").await
    }
}

impl core::fmt::Debug for SqlPolicyStore<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SqlPolicyStore")
            .field("namespace", &self.namespace)
            .finish()
    }
}
