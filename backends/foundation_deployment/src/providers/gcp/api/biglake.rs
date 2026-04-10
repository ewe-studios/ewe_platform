//! BiglakeProvider - State-aware biglake API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       biglake API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::biglake::{
    biglake_projects_catalogs_get_iam_policy_builder, biglake_projects_catalogs_get_iam_policy_task,
    biglake_projects_catalogs_set_iam_policy_builder, biglake_projects_catalogs_set_iam_policy_task,
    biglake_projects_catalogs_test_iam_permissions_builder, biglake_projects_catalogs_test_iam_permissions_task,
    biglake_projects_catalogs_namespaces_get_iam_policy_builder, biglake_projects_catalogs_namespaces_get_iam_policy_task,
    biglake_projects_catalogs_namespaces_set_iam_policy_builder, biglake_projects_catalogs_namespaces_set_iam_policy_task,
    biglake_projects_catalogs_namespaces_test_iam_permissions_builder, biglake_projects_catalogs_namespaces_test_iam_permissions_task,
    biglake_projects_catalogs_namespaces_tables_get_iam_policy_builder, biglake_projects_catalogs_namespaces_tables_get_iam_policy_task,
    biglake_projects_catalogs_namespaces_tables_set_iam_policy_builder, biglake_projects_catalogs_namespaces_tables_set_iam_policy_task,
    biglake_projects_catalogs_namespaces_tables_test_iam_permissions_builder, biglake_projects_catalogs_namespaces_tables_test_iam_permissions_task,
    biglake_projects_locations_catalogs_create_builder, biglake_projects_locations_catalogs_create_task,
    biglake_projects_locations_catalogs_delete_builder, biglake_projects_locations_catalogs_delete_task,
    biglake_projects_locations_catalogs_get_builder, biglake_projects_locations_catalogs_get_task,
    biglake_projects_locations_catalogs_list_builder, biglake_projects_locations_catalogs_list_task,
    biglake_projects_locations_catalogs_databases_create_builder, biglake_projects_locations_catalogs_databases_create_task,
    biglake_projects_locations_catalogs_databases_delete_builder, biglake_projects_locations_catalogs_databases_delete_task,
    biglake_projects_locations_catalogs_databases_get_builder, biglake_projects_locations_catalogs_databases_get_task,
    biglake_projects_locations_catalogs_databases_list_builder, biglake_projects_locations_catalogs_databases_list_task,
    biglake_projects_locations_catalogs_databases_patch_builder, biglake_projects_locations_catalogs_databases_patch_task,
    biglake_projects_locations_catalogs_databases_tables_create_builder, biglake_projects_locations_catalogs_databases_tables_create_task,
    biglake_projects_locations_catalogs_databases_tables_delete_builder, biglake_projects_locations_catalogs_databases_tables_delete_task,
    biglake_projects_locations_catalogs_databases_tables_get_builder, biglake_projects_locations_catalogs_databases_tables_get_task,
    biglake_projects_locations_catalogs_databases_tables_list_builder, biglake_projects_locations_catalogs_databases_tables_list_task,
    biglake_projects_locations_catalogs_databases_tables_patch_builder, biglake_projects_locations_catalogs_databases_tables_patch_task,
    biglake_projects_locations_catalogs_databases_tables_rename_builder, biglake_projects_locations_catalogs_databases_tables_rename_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::biglake::Catalog;
use crate::providers::gcp::clients::biglake::Database;
use crate::providers::gcp::clients::biglake::ListCatalogsResponse;
use crate::providers::gcp::clients::biglake::ListDatabasesResponse;
use crate::providers::gcp::clients::biglake::ListTablesResponse;
use crate::providers::gcp::clients::biglake::Policy;
use crate::providers::gcp::clients::biglake::Table;
use crate::providers::gcp::clients::biglake::TestIamPermissionsResponse;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsGetIamPolicyArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsNamespacesGetIamPolicyArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsNamespacesSetIamPolicyArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsNamespacesTablesGetIamPolicyArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsNamespacesTablesSetIamPolicyArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsNamespacesTablesTestIamPermissionsArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsNamespacesTestIamPermissionsArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsSetIamPolicyArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsCatalogsTestIamPermissionsArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsCreateArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesCreateArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesDeleteArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesGetArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesListArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesPatchArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesTablesCreateArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesTablesDeleteArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesTablesGetArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesTablesListArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesTablesPatchArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDatabasesTablesRenameArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsDeleteArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsGetArgs;
use crate::providers::gcp::clients::biglake::BiglakeProjectsLocationsCatalogsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BiglakeProvider with automatic state tracking.
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
/// let provider = BiglakeProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BiglakeProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BiglakeProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BiglakeProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Biglake projects catalogs get iam policy.
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
    pub fn biglake_projects_catalogs_get_iam_policy(
        &self,
        args: &BiglakeProjectsCatalogsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects catalogs set iam policy.
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
    pub fn biglake_projects_catalogs_set_iam_policy(
        &self,
        args: &BiglakeProjectsCatalogsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects catalogs test iam permissions.
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
    pub fn biglake_projects_catalogs_test_iam_permissions(
        &self,
        args: &BiglakeProjectsCatalogsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects catalogs namespaces get iam policy.
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
    pub fn biglake_projects_catalogs_namespaces_get_iam_policy(
        &self,
        args: &BiglakeProjectsCatalogsNamespacesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_namespaces_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_namespaces_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects catalogs namespaces set iam policy.
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
    pub fn biglake_projects_catalogs_namespaces_set_iam_policy(
        &self,
        args: &BiglakeProjectsCatalogsNamespacesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_namespaces_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_namespaces_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects catalogs namespaces test iam permissions.
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
    pub fn biglake_projects_catalogs_namespaces_test_iam_permissions(
        &self,
        args: &BiglakeProjectsCatalogsNamespacesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_namespaces_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_namespaces_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects catalogs namespaces tables get iam policy.
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
    pub fn biglake_projects_catalogs_namespaces_tables_get_iam_policy(
        &self,
        args: &BiglakeProjectsCatalogsNamespacesTablesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_namespaces_tables_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_namespaces_tables_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects catalogs namespaces tables set iam policy.
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
    pub fn biglake_projects_catalogs_namespaces_tables_set_iam_policy(
        &self,
        args: &BiglakeProjectsCatalogsNamespacesTablesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_namespaces_tables_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_namespaces_tables_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects catalogs namespaces tables test iam permissions.
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
    pub fn biglake_projects_catalogs_namespaces_tables_test_iam_permissions(
        &self,
        args: &BiglakeProjectsCatalogsNamespacesTablesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_catalogs_namespaces_tables_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_catalogs_namespaces_tables_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Catalog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_create(
        &self,
        args: &BiglakeProjectsLocationsCatalogsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Catalog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_create_builder(
            &self.http_client,
            &args.parent,
            &args.catalogId,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Catalog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_delete(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Catalog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Catalog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn biglake_projects_locations_catalogs_get(
        &self,
        args: &BiglakeProjectsLocationsCatalogsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Catalog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCatalogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn biglake_projects_locations_catalogs_list(
        &self,
        args: &BiglakeProjectsLocationsCatalogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCatalogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Database result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_databases_create(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Database, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_create_builder(
            &self.http_client,
            &args.parent,
            &args.databaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Database result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_databases_delete(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Database, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Database result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn biglake_projects_locations_catalogs_databases_get(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Database, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatabasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn biglake_projects_locations_catalogs_databases_list(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatabasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Database result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_databases_patch(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Database, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases tables create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_databases_tables_create(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesTablesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_tables_create_builder(
            &self.http_client,
            &args.parent,
            &args.tableId,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_tables_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases tables delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_databases_tables_delete(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesTablesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_tables_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_tables_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases tables get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn biglake_projects_locations_catalogs_databases_tables_get(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesTablesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_tables_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_tables_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases tables list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTablesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn biglake_projects_locations_catalogs_databases_tables_list(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesTablesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTablesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_tables_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_tables_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases tables patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_databases_tables_patch(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesTablesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_tables_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_tables_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Biglake projects locations catalogs databases tables rename.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn biglake_projects_locations_catalogs_databases_tables_rename(
        &self,
        args: &BiglakeProjectsLocationsCatalogsDatabasesTablesRenameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = biglake_projects_locations_catalogs_databases_tables_rename_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = biglake_projects_locations_catalogs_databases_tables_rename_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
