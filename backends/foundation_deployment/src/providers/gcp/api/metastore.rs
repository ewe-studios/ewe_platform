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
    metastore_projects_locations_federations_create_builder, metastore_projects_locations_federations_create_task,
    metastore_projects_locations_federations_delete_builder, metastore_projects_locations_federations_delete_task,
    metastore_projects_locations_federations_patch_builder, metastore_projects_locations_federations_patch_task,
    metastore_projects_locations_federations_set_iam_policy_builder, metastore_projects_locations_federations_set_iam_policy_task,
    metastore_projects_locations_federations_test_iam_permissions_builder, metastore_projects_locations_federations_test_iam_permissions_task,
    metastore_projects_locations_operations_cancel_builder, metastore_projects_locations_operations_cancel_task,
    metastore_projects_locations_operations_delete_builder, metastore_projects_locations_operations_delete_task,
    metastore_projects_locations_services_alter_location_builder, metastore_projects_locations_services_alter_location_task,
    metastore_projects_locations_services_alter_table_properties_builder, metastore_projects_locations_services_alter_table_properties_task,
    metastore_projects_locations_services_cancel_migration_builder, metastore_projects_locations_services_cancel_migration_task,
    metastore_projects_locations_services_complete_migration_builder, metastore_projects_locations_services_complete_migration_task,
    metastore_projects_locations_services_create_builder, metastore_projects_locations_services_create_task,
    metastore_projects_locations_services_delete_builder, metastore_projects_locations_services_delete_task,
    metastore_projects_locations_services_export_metadata_builder, metastore_projects_locations_services_export_metadata_task,
    metastore_projects_locations_services_move_table_to_database_builder, metastore_projects_locations_services_move_table_to_database_task,
    metastore_projects_locations_services_patch_builder, metastore_projects_locations_services_patch_task,
    metastore_projects_locations_services_query_metadata_builder, metastore_projects_locations_services_query_metadata_task,
    metastore_projects_locations_services_restore_builder, metastore_projects_locations_services_restore_task,
    metastore_projects_locations_services_set_iam_policy_builder, metastore_projects_locations_services_set_iam_policy_task,
    metastore_projects_locations_services_start_migration_builder, metastore_projects_locations_services_start_migration_task,
    metastore_projects_locations_services_test_iam_permissions_builder, metastore_projects_locations_services_test_iam_permissions_task,
    metastore_projects_locations_services_backups_create_builder, metastore_projects_locations_services_backups_create_task,
    metastore_projects_locations_services_backups_delete_builder, metastore_projects_locations_services_backups_delete_task,
    metastore_projects_locations_services_backups_set_iam_policy_builder, metastore_projects_locations_services_backups_set_iam_policy_task,
    metastore_projects_locations_services_databases_set_iam_policy_builder, metastore_projects_locations_services_databases_set_iam_policy_task,
    metastore_projects_locations_services_databases_tables_set_iam_policy_builder, metastore_projects_locations_services_databases_tables_set_iam_policy_task,
    metastore_projects_locations_services_metadata_imports_create_builder, metastore_projects_locations_services_metadata_imports_create_task,
    metastore_projects_locations_services_metadata_imports_patch_builder, metastore_projects_locations_services_metadata_imports_patch_task,
    metastore_projects_locations_services_migration_executions_delete_builder, metastore_projects_locations_services_migration_executions_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::metastore::Empty;
use crate::providers::gcp::clients::metastore::Operation;
use crate::providers::gcp::clients::metastore::Policy;
use crate::providers::gcp::clients::metastore::TestIamPermissionsResponse;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsCreateArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsPatchArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsFederationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesAlterLocationArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesAlterTablePropertiesArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsCreateArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesCancelMigrationArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesCompleteMigrationArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesCreateArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesDatabasesSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesDatabasesTablesSetIamPolicyArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesDeleteArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesExportMetadataArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMetadataImportsCreateArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMetadataImportsPatchArgs;
use crate::providers::gcp::clients::metastore::MetastoreProjectsLocationsServicesMigrationExecutionsDeleteArgs;
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
