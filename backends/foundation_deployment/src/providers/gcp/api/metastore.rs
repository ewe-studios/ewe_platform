//! MetastoreProvider - State-aware metastore API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       metastore API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::metastore::{
    metastore_projects_locations_get_builder, metastore_projects_locations_get_task,
    metastore_projects_locations_list_builder, metastore_projects_locations_list_task,
    metastore_projects_locations_federations_create_builder, metastore_projects_locations_federations_create_task,
    metastore_projects_locations_federations_delete_builder, metastore_projects_locations_federations_delete_task,
    metastore_projects_locations_federations_get_builder, metastore_projects_locations_federations_get_task,
    metastore_projects_locations_federations_get_iam_policy_builder, metastore_projects_locations_federations_get_iam_policy_task,
    metastore_projects_locations_federations_list_builder, metastore_projects_locations_federations_list_task,
    metastore_projects_locations_federations_patch_builder, metastore_projects_locations_federations_patch_task,
    metastore_projects_locations_federations_set_iam_policy_builder, metastore_projects_locations_federations_set_iam_policy_task,
    metastore_projects_locations_federations_test_iam_permissions_builder, metastore_projects_locations_federations_test_iam_permissions_task,
    metastore_projects_locations_operations_cancel_builder, metastore_projects_locations_operations_cancel_task,
    metastore_projects_locations_operations_delete_builder, metastore_projects_locations_operations_delete_task,
    metastore_projects_locations_operations_get_builder, metastore_projects_locations_operations_get_task,
    metastore_projects_locations_operations_list_builder, metastore_projects_locations_operations_list_task,
    metastore_projects_locations_services_alter_location_builder, metastore_projects_locations_services_alter_location_task,
    metastore_projects_locations_services_alter_table_properties_builder, metastore_projects_locations_services_alter_table_properties_task,
    metastore_projects_locations_services_cancel_migration_builder, metastore_projects_locations_services_cancel_migration_task,
    metastore_projects_locations_services_complete_migration_builder, metastore_projects_locations_services_complete_migration_task,
    metastore_projects_locations_services_create_builder, metastore_projects_locations_services_create_task,
    metastore_projects_locations_services_delete_builder, metastore_projects_locations_services_delete_task,
    metastore_projects_locations_services_export_metadata_builder, metastore_projects_locations_services_export_metadata_task,
    metastore_projects_locations_services_get_builder, metastore_projects_locations_services_get_task,
    metastore_projects_locations_services_get_iam_policy_builder, metastore_projects_locations_services_get_iam_policy_task,
    metastore_projects_locations_services_list_builder, metastore_projects_locations_services_list_task,
    metastore_projects_locations_services_move_table_to_database_builder, metastore_projects_locations_services_move_table_to_database_task,
    metastore_projects_locations_services_patch_builder, metastore_projects_locations_services_patch_task,
    metastore_projects_locations_services_query_metadata_builder, metastore_projects_locations_services_query_metadata_task,
    metastore_projects_locations_services_restore_builder, metastore_projects_locations_services_restore_task,
    metastore_projects_locations_services_set_iam_policy_builder, metastore_projects_locations_services_set_iam_policy_task,
    metastore_projects_locations_services_start_migration_builder, metastore_projects_locations_services_start_migration_task,
    metastore_projects_locations_services_test_iam_permissions_builder, metastore_projects_locations_services_test_iam_permissions_task,
    metastore_projects_locations_services_backups_create_builder, metastore_projects_locations_services_backups_create_task,
    metastore_projects_locations_services_backups_delete_builder, metastore_projects_locations_services_backups_delete_task,
    metastore_projects_locations_services_backups_get_builder, metastore_projects_locations_services_backups_get_task,
    metastore_projects_locations_services_backups_get_iam_policy_builder, metastore_projects_locations_services_backups_get_iam_policy_task,
    metastore_projects_locations_services_backups_list_builder, metastore_projects_locations_services_backups_list_task,
    metastore_projects_locations_services_backups_set_iam_policy_builder, metastore_projects_locations_services_backups_set_iam_policy_task,
    metastore_projects_locations_services_databases_get_iam_policy_builder, metastore_projects_locations_services_databases_get_iam_policy_task,
    metastore_projects_locations_services_databases_set_iam_policy_builder, metastore_projects_locations_services_databases_set_iam_policy_task,
    metastore_projects_locations_services_databases_tables_get_iam_policy_builder, metastore_projects_locations_services_databases_tables_get_iam_policy_task,
    metastore_projects_locations_services_databases_tables_set_iam_policy_builder, metastore_projects_locations_services_databases_tables_set_iam_policy_task,
    metastore_projects_locations_services_metadata_imports_create_builder, metastore_projects_locations_services_metadata_imports_create_task,
    metastore_projects_locations_services_metadata_imports_get_builder, metastore_projects_locations_services_metadata_imports_get_task,
    metastore_projects_locations_services_metadata_imports_list_builder, metastore_projects_locations_services_metadata_imports_list_task,
    metastore_projects_locations_services_metadata_imports_patch_builder, metastore_projects_locations_services_metadata_imports_patch_task,
    metastore_projects_locations_services_migration_executions_delete_builder, metastore_projects_locations_services_migration_executions_delete_task,
    metastore_projects_locations_services_migration_executions_get_builder, metastore_projects_locations_services_migration_executions_get_task,
    metastore_projects_locations_services_migration_executions_list_builder, metastore_projects_locations_services_migration_executions_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::metastore::Backup;
use crate::providers::gcp::clients::metastore::Empty;
use crate::providers::gcp::clients::metastore::Federation;
use crate::providers::gcp::clients::metastore::ListBackupsResponse;
use crate::providers::gcp::clients::metastore::ListFederationsResponse;
use crate::providers::gcp::clients::metastore::ListLocationsResponse;
use crate::providers::gcp::clients::metastore::ListMetadataImportsResponse;
use crate::providers::gcp::clients::metastore::ListMigrationExecutionsResponse;
use crate::providers::gcp::clients::metastore::ListOperationsResponse;
use crate::providers::gcp::clients::metastore::ListServicesResponse;
use crate::providers::gcp::clients::metastore::Location;
use crate::providers::gcp::clients::metastore::MetadataImport;
use crate::providers::gcp::clients::metastore::MigrationExecution;
use crate::providers::gcp::clients::metastore::Operation;
use crate::providers::gcp::clients::metastore::Policy;
use crate::providers::gcp::clients::metastore::Service;
use crate::providers::gcp::clients::metastore::TestIamPermissionsResponse;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsCreateArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsGetArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsGetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsListArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsPatchArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsGetArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsListArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesAlterLocationArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesAlterTablePropertiesArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsCreateArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsGetArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsGetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsListArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesCancelMigrationArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesCompleteMigrationArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesCreateArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesDatabasesGetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesDatabasesSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesDatabasesTablesGetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesDatabasesTablesSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesExportMetadataArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesGetArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesGetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesListArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMetadataImportsCreateArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMetadataImportsGetArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMetadataImportsListArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMetadataImportsPatchArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMigrationExecutionsDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMigrationExecutionsGetArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMigrationExecutionsListArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMoveTableToDatabaseArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesPatchArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesQueryMetadataArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesRestoreArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesStartMigrationArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MetastoreProvider with automatic state tracking.
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
/// let provider = MetastoreProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MetastoreProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MetastoreProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MetastoreProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Metastore projects locations get.
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
    pub fn metastore_projects_locations_get(
        &self,
        args: &MetastoreProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations list.
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
    pub fn metastore_projects_locations_list(
        &self,
        args: &MetastoreProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations federations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_federations_create(
        &self,
        args: &MetastoreProjectsLocationsFederationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_federations_create_builder(
            &self.http_client,
            &args.parent,
            &args.federationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_federations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations federations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_federations_delete(
        &self,
        args: &MetastoreProjectsLocationsFederationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_federations_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_federations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations federations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Federation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_federations_get(
        &self,
        args: &MetastoreProjectsLocationsFederationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Federation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_federations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_federations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations federations get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_federations_get_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsFederationsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_federations_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_federations_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations federations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFederationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_federations_list(
        &self,
        args: &MetastoreProjectsLocationsFederationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFederationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_federations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_federations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations federations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_federations_patch(
        &self,
        args: &MetastoreProjectsLocationsFederationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_federations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_federations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations federations set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_federations_set_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsFederationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_federations_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_federations_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations federations test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_federations_test_iam_permissions(
        &self,
        args: &MetastoreProjectsLocationsFederationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_federations_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_federations_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations operations cancel.
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
    pub fn metastore_projects_locations_operations_cancel(
        &self,
        args: &MetastoreProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations operations delete.
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
    pub fn metastore_projects_locations_operations_delete(
        &self,
        args: &MetastoreProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_operations_get(
        &self,
        args: &MetastoreProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_operations_list(
        &self,
        args: &MetastoreProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services alter location.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_alter_location(
        &self,
        args: &MetastoreProjectsLocationsServicesAlterLocationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_alter_location_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_alter_location_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services alter table properties.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_alter_table_properties(
        &self,
        args: &MetastoreProjectsLocationsServicesAlterTablePropertiesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_alter_table_properties_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_alter_table_properties_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services cancel migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_cancel_migration(
        &self,
        args: &MetastoreProjectsLocationsServicesCancelMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_cancel_migration_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_cancel_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services complete migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_complete_migration(
        &self,
        args: &MetastoreProjectsLocationsServicesCompleteMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_complete_migration_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_complete_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_create(
        &self,
        args: &MetastoreProjectsLocationsServicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_delete(
        &self,
        args: &MetastoreProjectsLocationsServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services export metadata.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_export_metadata(
        &self,
        args: &MetastoreProjectsLocationsServicesExportMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_export_metadata_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_export_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_get(
        &self,
        args: &MetastoreProjectsLocationsServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_get_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsServicesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_list(
        &self,
        args: &MetastoreProjectsLocationsServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services move table to database.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_move_table_to_database(
        &self,
        args: &MetastoreProjectsLocationsServicesMoveTableToDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_move_table_to_database_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_move_table_to_database_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_patch(
        &self,
        args: &MetastoreProjectsLocationsServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services query metadata.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_query_metadata(
        &self,
        args: &MetastoreProjectsLocationsServicesQueryMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_query_metadata_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_query_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_restore(
        &self,
        args: &MetastoreProjectsLocationsServicesRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_restore_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_set_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsServicesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services start migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_start_migration(
        &self,
        args: &MetastoreProjectsLocationsServicesStartMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_start_migration_builder(
            &self.http_client,
            &args.service,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_start_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_test_iam_permissions(
        &self,
        args: &MetastoreProjectsLocationsServicesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services backups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_backups_create(
        &self,
        args: &MetastoreProjectsLocationsServicesBackupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_backups_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_backups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services backups delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_backups_delete(
        &self,
        args: &MetastoreProjectsLocationsServicesBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_backups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services backups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Backup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_backups_get(
        &self,
        args: &MetastoreProjectsLocationsServicesBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services backups get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_backups_get_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsServicesBackupsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_backups_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_backups_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services backups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_backups_list(
        &self,
        args: &MetastoreProjectsLocationsServicesBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services backups set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_backups_set_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsServicesBackupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_backups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_backups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services databases get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_databases_get_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsServicesDatabasesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_databases_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_databases_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services databases set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_databases_set_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsServicesDatabasesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_databases_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_databases_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services databases tables get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_databases_tables_get_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsServicesDatabasesTablesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_databases_tables_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_databases_tables_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services databases tables set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_databases_tables_set_iam_policy(
        &self,
        args: &MetastoreProjectsLocationsServicesDatabasesTablesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_databases_tables_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_databases_tables_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services metadata imports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_metadata_imports_create(
        &self,
        args: &MetastoreProjectsLocationsServicesMetadataImportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_metadata_imports_create_builder(
            &self.http_client,
            &args.parent,
            &args.metadataImportId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_metadata_imports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services metadata imports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MetadataImport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_metadata_imports_get(
        &self,
        args: &MetastoreProjectsLocationsServicesMetadataImportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MetadataImport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_metadata_imports_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_metadata_imports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services metadata imports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMetadataImportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_metadata_imports_list(
        &self,
        args: &MetastoreProjectsLocationsServicesMetadataImportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMetadataImportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_metadata_imports_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_metadata_imports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services metadata imports patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_metadata_imports_patch(
        &self,
        args: &MetastoreProjectsLocationsServicesMetadataImportsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_metadata_imports_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_metadata_imports_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services migration executions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn metastore_projects_locations_services_migration_executions_delete(
        &self,
        args: &MetastoreProjectsLocationsServicesMigrationExecutionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_migration_executions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_migration_executions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services migration executions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MigrationExecution result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_migration_executions_get(
        &self,
        args: &MetastoreProjectsLocationsServicesMigrationExecutionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MigrationExecution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_migration_executions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_migration_executions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Metastore projects locations services migration executions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMigrationExecutionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn metastore_projects_locations_services_migration_executions_list(
        &self,
        args: &MetastoreProjectsLocationsServicesMigrationExecutionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMigrationExecutionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = metastore_projects_locations_services_migration_executions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = metastore_projects_locations_services_migration_executions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
