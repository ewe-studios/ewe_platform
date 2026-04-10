//! FirestoreProvider - State-aware firestore API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firestore API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firestore::{
    firestore_projects_databases_bulk_delete_documents_builder, firestore_projects_databases_bulk_delete_documents_task,
    firestore_projects_databases_clone_builder, firestore_projects_databases_clone_task,
    firestore_projects_databases_create_builder, firestore_projects_databases_create_task,
    firestore_projects_databases_delete_builder, firestore_projects_databases_delete_task,
    firestore_projects_databases_export_documents_builder, firestore_projects_databases_export_documents_task,
    firestore_projects_databases_get_builder, firestore_projects_databases_get_task,
    firestore_projects_databases_import_documents_builder, firestore_projects_databases_import_documents_task,
    firestore_projects_databases_list_builder, firestore_projects_databases_list_task,
    firestore_projects_databases_patch_builder, firestore_projects_databases_patch_task,
    firestore_projects_databases_restore_builder, firestore_projects_databases_restore_task,
    firestore_projects_databases_backup_schedules_create_builder, firestore_projects_databases_backup_schedules_create_task,
    firestore_projects_databases_backup_schedules_delete_builder, firestore_projects_databases_backup_schedules_delete_task,
    firestore_projects_databases_backup_schedules_get_builder, firestore_projects_databases_backup_schedules_get_task,
    firestore_projects_databases_backup_schedules_list_builder, firestore_projects_databases_backup_schedules_list_task,
    firestore_projects_databases_backup_schedules_patch_builder, firestore_projects_databases_backup_schedules_patch_task,
    firestore_projects_databases_collection_groups_fields_get_builder, firestore_projects_databases_collection_groups_fields_get_task,
    firestore_projects_databases_collection_groups_fields_list_builder, firestore_projects_databases_collection_groups_fields_list_task,
    firestore_projects_databases_collection_groups_fields_patch_builder, firestore_projects_databases_collection_groups_fields_patch_task,
    firestore_projects_databases_collection_groups_indexes_create_builder, firestore_projects_databases_collection_groups_indexes_create_task,
    firestore_projects_databases_collection_groups_indexes_delete_builder, firestore_projects_databases_collection_groups_indexes_delete_task,
    firestore_projects_databases_collection_groups_indexes_get_builder, firestore_projects_databases_collection_groups_indexes_get_task,
    firestore_projects_databases_collection_groups_indexes_list_builder, firestore_projects_databases_collection_groups_indexes_list_task,
    firestore_projects_databases_documents_batch_get_builder, firestore_projects_databases_documents_batch_get_task,
    firestore_projects_databases_documents_batch_write_builder, firestore_projects_databases_documents_batch_write_task,
    firestore_projects_databases_documents_begin_transaction_builder, firestore_projects_databases_documents_begin_transaction_task,
    firestore_projects_databases_documents_commit_builder, firestore_projects_databases_documents_commit_task,
    firestore_projects_databases_documents_create_document_builder, firestore_projects_databases_documents_create_document_task,
    firestore_projects_databases_documents_delete_builder, firestore_projects_databases_documents_delete_task,
    firestore_projects_databases_documents_execute_pipeline_builder, firestore_projects_databases_documents_execute_pipeline_task,
    firestore_projects_databases_documents_get_builder, firestore_projects_databases_documents_get_task,
    firestore_projects_databases_documents_list_builder, firestore_projects_databases_documents_list_task,
    firestore_projects_databases_documents_list_collection_ids_builder, firestore_projects_databases_documents_list_collection_ids_task,
    firestore_projects_databases_documents_list_documents_builder, firestore_projects_databases_documents_list_documents_task,
    firestore_projects_databases_documents_listen_builder, firestore_projects_databases_documents_listen_task,
    firestore_projects_databases_documents_partition_query_builder, firestore_projects_databases_documents_partition_query_task,
    firestore_projects_databases_documents_patch_builder, firestore_projects_databases_documents_patch_task,
    firestore_projects_databases_documents_rollback_builder, firestore_projects_databases_documents_rollback_task,
    firestore_projects_databases_documents_run_aggregation_query_builder, firestore_projects_databases_documents_run_aggregation_query_task,
    firestore_projects_databases_documents_run_query_builder, firestore_projects_databases_documents_run_query_task,
    firestore_projects_databases_documents_write_builder, firestore_projects_databases_documents_write_task,
    firestore_projects_databases_operations_cancel_builder, firestore_projects_databases_operations_cancel_task,
    firestore_projects_databases_operations_delete_builder, firestore_projects_databases_operations_delete_task,
    firestore_projects_databases_operations_get_builder, firestore_projects_databases_operations_get_task,
    firestore_projects_databases_operations_list_builder, firestore_projects_databases_operations_list_task,
    firestore_projects_databases_user_creds_create_builder, firestore_projects_databases_user_creds_create_task,
    firestore_projects_databases_user_creds_delete_builder, firestore_projects_databases_user_creds_delete_task,
    firestore_projects_databases_user_creds_disable_builder, firestore_projects_databases_user_creds_disable_task,
    firestore_projects_databases_user_creds_enable_builder, firestore_projects_databases_user_creds_enable_task,
    firestore_projects_databases_user_creds_get_builder, firestore_projects_databases_user_creds_get_task,
    firestore_projects_databases_user_creds_list_builder, firestore_projects_databases_user_creds_list_task,
    firestore_projects_databases_user_creds_reset_password_builder, firestore_projects_databases_user_creds_reset_password_task,
    firestore_projects_locations_get_builder, firestore_projects_locations_get_task,
    firestore_projects_locations_list_builder, firestore_projects_locations_list_task,
    firestore_projects_locations_backups_delete_builder, firestore_projects_locations_backups_delete_task,
    firestore_projects_locations_backups_get_builder, firestore_projects_locations_backups_get_task,
    firestore_projects_locations_backups_list_builder, firestore_projects_locations_backups_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firestore::BatchGetDocumentsResponse;
use crate::providers::gcp::clients::firestore::BatchWriteResponse;
use crate::providers::gcp::clients::firestore::BeginTransactionResponse;
use crate::providers::gcp::clients::firestore::CommitResponse;
use crate::providers::gcp::clients::firestore::Document;
use crate::providers::gcp::clients::firestore::Empty;
use crate::providers::gcp::clients::firestore::ExecutePipelineResponse;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1Backup;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1BackupSchedule;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1Database;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1Field;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1Index;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1ListBackupSchedulesResponse;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1ListBackupsResponse;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1ListDatabasesResponse;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1ListFieldsResponse;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1ListIndexesResponse;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1ListUserCredsResponse;
use crate::providers::gcp::clients::firestore::GoogleFirestoreAdminV1UserCreds;
use crate::providers::gcp::clients::firestore::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::firestore::GoogleLongrunningOperation;
use crate::providers::gcp::clients::firestore::ListCollectionIdsResponse;
use crate::providers::gcp::clients::firestore::ListDocumentsResponse;
use crate::providers::gcp::clients::firestore::ListLocationsResponse;
use crate::providers::gcp::clients::firestore::ListenResponse;
use crate::providers::gcp::clients::firestore::Location;
use crate::providers::gcp::clients::firestore::PartitionQueryResponse;
use crate::providers::gcp::clients::firestore::RunAggregationQueryResponse;
use crate::providers::gcp::clients::firestore::RunQueryResponse;
use crate::providers::gcp::clients::firestore::WriteResponse;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesBackupSchedulesCreateArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesBackupSchedulesDeleteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesBackupSchedulesGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesBackupSchedulesListArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesBackupSchedulesPatchArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesBulkDeleteDocumentsArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCloneArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCollectionGroupsFieldsGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCollectionGroupsFieldsListArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCollectionGroupsFieldsPatchArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCollectionGroupsIndexesCreateArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCollectionGroupsIndexesDeleteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCollectionGroupsIndexesGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCollectionGroupsIndexesListArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesCreateArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDeleteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsBatchGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsBatchWriteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsBeginTransactionArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsCommitArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsCreateDocumentArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsDeleteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsExecutePipelineArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsListArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsListCollectionIdsArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsListDocumentsArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsListenArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsPartitionQueryArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsPatchArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsRollbackArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsRunAggregationQueryArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsRunQueryArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesDocumentsWriteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesExportDocumentsArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesImportDocumentsArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesListArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesOperationsCancelArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesOperationsDeleteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesOperationsGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesOperationsListArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesPatchArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesRestoreArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesUserCredsCreateArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesUserCredsDeleteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesUserCredsDisableArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesUserCredsEnableArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesUserCredsGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesUserCredsListArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsDatabasesUserCredsResetPasswordArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsLocationsBackupsDeleteArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsLocationsBackupsGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsLocationsBackupsListArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsLocationsGetArgs;
use crate::providers::gcp::clients::firestore::FirestoreProjectsLocationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirestoreProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let client = ProviderClient::new("my-project", "dev", state_store);
/// let http_client = SimpleHttpClient::new(...);
/// let provider = FirestoreProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirestoreProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirestoreProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirestoreProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firestore projects databases bulk delete documents.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_bulk_delete_documents(
        &self,
        args: &FirestoreProjectsDatabasesBulkDeleteDocumentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_bulk_delete_documents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_bulk_delete_documents_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases clone.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_clone(
        &self,
        args: &FirestoreProjectsDatabasesCloneArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_clone_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_clone_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_create(
        &self,
        args: &FirestoreProjectsDatabasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_create_builder(
            &self.http_client,
            &args.parent,
            &args.databaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_delete(
        &self,
        args: &FirestoreProjectsDatabasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases export documents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_export_documents(
        &self,
        args: &FirestoreProjectsDatabasesExportDocumentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_export_documents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_export_documents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1Database result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_get(
        &self,
        args: &FirestoreProjectsDatabasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1Database, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases import documents.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_import_documents(
        &self,
        args: &FirestoreProjectsDatabasesImportDocumentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_import_documents_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_import_documents_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1ListDatabasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_list(
        &self,
        args: &FirestoreProjectsDatabasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1ListDatabasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_list_builder(
            &self.http_client,
            &args.parent,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_patch(
        &self,
        args: &FirestoreProjectsDatabasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_restore(
        &self,
        args: &FirestoreProjectsDatabasesRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_restore_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases backup schedules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1BackupSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_backup_schedules_create(
        &self,
        args: &FirestoreProjectsDatabasesBackupSchedulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1BackupSchedule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_backup_schedules_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_backup_schedules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases backup schedules delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_backup_schedules_delete(
        &self,
        args: &FirestoreProjectsDatabasesBackupSchedulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_backup_schedules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_backup_schedules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases backup schedules get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1BackupSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_backup_schedules_get(
        &self,
        args: &FirestoreProjectsDatabasesBackupSchedulesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1BackupSchedule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_backup_schedules_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_backup_schedules_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases backup schedules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1ListBackupSchedulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_backup_schedules_list(
        &self,
        args: &FirestoreProjectsDatabasesBackupSchedulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1ListBackupSchedulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_backup_schedules_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_backup_schedules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases backup schedules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1BackupSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_backup_schedules_patch(
        &self,
        args: &FirestoreProjectsDatabasesBackupSchedulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1BackupSchedule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_backup_schedules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_backup_schedules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases collection groups fields get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1Field result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_collection_groups_fields_get(
        &self,
        args: &FirestoreProjectsDatabasesCollectionGroupsFieldsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1Field, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_collection_groups_fields_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_collection_groups_fields_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases collection groups fields list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1ListFieldsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_collection_groups_fields_list(
        &self,
        args: &FirestoreProjectsDatabasesCollectionGroupsFieldsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1ListFieldsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_collection_groups_fields_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_collection_groups_fields_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases collection groups fields patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_collection_groups_fields_patch(
        &self,
        args: &FirestoreProjectsDatabasesCollectionGroupsFieldsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_collection_groups_fields_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_collection_groups_fields_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases collection groups indexes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_collection_groups_indexes_create(
        &self,
        args: &FirestoreProjectsDatabasesCollectionGroupsIndexesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_collection_groups_indexes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_collection_groups_indexes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases collection groups indexes delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_collection_groups_indexes_delete(
        &self,
        args: &FirestoreProjectsDatabasesCollectionGroupsIndexesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_collection_groups_indexes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_collection_groups_indexes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases collection groups indexes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1Index result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_collection_groups_indexes_get(
        &self,
        args: &FirestoreProjectsDatabasesCollectionGroupsIndexesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1Index, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_collection_groups_indexes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_collection_groups_indexes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases collection groups indexes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1ListIndexesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_collection_groups_indexes_list(
        &self,
        args: &FirestoreProjectsDatabasesCollectionGroupsIndexesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1ListIndexesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_collection_groups_indexes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_collection_groups_indexes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetDocumentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_batch_get(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetDocumentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_batch_get_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents batch write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchWriteResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_batch_write(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsBatchWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchWriteResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_batch_write_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_batch_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents begin transaction.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BeginTransactionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_begin_transaction(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsBeginTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BeginTransactionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_begin_transaction_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_begin_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents commit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommitResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_commit(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsCommitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommitResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_commit_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_commit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents create document.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Document result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_create_document(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsCreateDocumentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Document, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_create_document_builder(
            &self.http_client,
            &args.parent,
            &args.collectionId,
            &args.documentId,
            &args.mask.fieldPaths,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_create_document_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_delete(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_delete_builder(
            &self.http_client,
            &args.name,
            &args.currentDocument.exists,
            &args.currentDocument.updateTime,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents execute pipeline.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecutePipelineResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_execute_pipeline(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsExecutePipelineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecutePipelineResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_execute_pipeline_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_execute_pipeline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Document result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_get(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Document, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_get_builder(
            &self.http_client,
            &args.name,
            &args.mask.fieldPaths,
            &args.readTime,
            &args.transaction,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDocumentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_list(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDocumentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_list_builder(
            &self.http_client,
            &args.parent,
            &args.collectionId,
            &args.mask.fieldPaths,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.readTime,
            &args.showMissing,
            &args.transaction,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents list collection ids.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCollectionIdsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_list_collection_ids(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsListCollectionIdsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCollectionIdsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_list_collection_ids_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_list_collection_ids_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents list documents.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDocumentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_list_documents(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsListDocumentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDocumentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_list_documents_builder(
            &self.http_client,
            &args.parent,
            &args.collectionId,
            &args.mask.fieldPaths,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.readTime,
            &args.showMissing,
            &args.transaction,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_list_documents_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents listen.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_listen(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsListenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_listen_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_listen_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents partition query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PartitionQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_partition_query(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsPartitionQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PartitionQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_partition_query_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_partition_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Document result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_patch(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Document, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_patch_builder(
            &self.http_client,
            &args.name,
            &args.currentDocument.exists,
            &args.currentDocument.updateTime,
            &args.mask.fieldPaths,
            &args.updateMask.fieldPaths,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents rollback.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_rollback(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_rollback_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents run aggregation query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RunAggregationQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_run_aggregation_query(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsRunAggregationQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RunAggregationQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_run_aggregation_query_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_run_aggregation_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents run query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RunQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_documents_run_query(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsRunQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RunQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_run_query_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_run_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases documents write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_documents_write(
        &self,
        args: &FirestoreProjectsDatabasesDocumentsWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_documents_write_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_documents_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_operations_cancel(
        &self,
        args: &FirestoreProjectsDatabasesOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases operations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_operations_delete(
        &self,
        args: &FirestoreProjectsDatabasesOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_operations_get(
        &self,
        args: &FirestoreProjectsDatabasesOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_operations_list(
        &self,
        args: &FirestoreProjectsDatabasesOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases user creds create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1UserCreds result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_user_creds_create(
        &self,
        args: &FirestoreProjectsDatabasesUserCredsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1UserCreds, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_user_creds_create_builder(
            &self.http_client,
            &args.parent,
            &args.userCredsId,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_user_creds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases user creds delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_user_creds_delete(
        &self,
        args: &FirestoreProjectsDatabasesUserCredsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_user_creds_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_user_creds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases user creds disable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1UserCreds result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_user_creds_disable(
        &self,
        args: &FirestoreProjectsDatabasesUserCredsDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1UserCreds, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_user_creds_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_user_creds_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases user creds enable.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1UserCreds result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_user_creds_enable(
        &self,
        args: &FirestoreProjectsDatabasesUserCredsEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1UserCreds, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_user_creds_enable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_user_creds_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases user creds get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1UserCreds result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_user_creds_get(
        &self,
        args: &FirestoreProjectsDatabasesUserCredsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1UserCreds, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_user_creds_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_user_creds_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases user creds list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1ListUserCredsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_databases_user_creds_list(
        &self,
        args: &FirestoreProjectsDatabasesUserCredsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1ListUserCredsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_user_creds_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_user_creds_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects databases user creds reset password.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1UserCreds result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_databases_user_creds_reset_password(
        &self,
        args: &FirestoreProjectsDatabasesUserCredsResetPasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1UserCreds, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_databases_user_creds_reset_password_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_databases_user_creds_reset_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_locations_get(
        &self,
        args: &FirestoreProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_locations_list(
        &self,
        args: &FirestoreProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects locations backups delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firestore_projects_locations_backups_delete(
        &self,
        args: &FirestoreProjectsLocationsBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_locations_backups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_locations_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects locations backups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1Backup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_locations_backups_get(
        &self,
        args: &FirestoreProjectsLocationsBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_locations_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_locations_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firestore projects locations backups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleFirestoreAdminV1ListBackupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firestore_projects_locations_backups_list(
        &self,
        args: &FirestoreProjectsLocationsBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleFirestoreAdminV1ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firestore_projects_locations_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
        )
        .map_err(ProviderError::Api)?;

        let task = firestore_projects_locations_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
