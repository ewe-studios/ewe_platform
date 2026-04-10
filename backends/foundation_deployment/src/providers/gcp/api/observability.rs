//! ObservabilityProvider - State-aware observability API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       observability API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::observability::{
    observability_folders_locations_get_builder, observability_folders_locations_get_task,
    observability_folders_locations_get_settings_builder, observability_folders_locations_get_settings_task,
    observability_folders_locations_list_builder, observability_folders_locations_list_task,
    observability_folders_locations_update_settings_builder, observability_folders_locations_update_settings_task,
    observability_folders_locations_operations_cancel_builder, observability_folders_locations_operations_cancel_task,
    observability_folders_locations_operations_delete_builder, observability_folders_locations_operations_delete_task,
    observability_folders_locations_operations_get_builder, observability_folders_locations_operations_get_task,
    observability_folders_locations_operations_list_builder, observability_folders_locations_operations_list_task,
    observability_organizations_locations_get_builder, observability_organizations_locations_get_task,
    observability_organizations_locations_get_settings_builder, observability_organizations_locations_get_settings_task,
    observability_organizations_locations_list_builder, observability_organizations_locations_list_task,
    observability_organizations_locations_update_settings_builder, observability_organizations_locations_update_settings_task,
    observability_organizations_locations_operations_cancel_builder, observability_organizations_locations_operations_cancel_task,
    observability_organizations_locations_operations_delete_builder, observability_organizations_locations_operations_delete_task,
    observability_organizations_locations_operations_get_builder, observability_organizations_locations_operations_get_task,
    observability_organizations_locations_operations_list_builder, observability_organizations_locations_operations_list_task,
    observability_projects_locations_get_builder, observability_projects_locations_get_task,
    observability_projects_locations_get_settings_builder, observability_projects_locations_get_settings_task,
    observability_projects_locations_list_builder, observability_projects_locations_list_task,
    observability_projects_locations_update_settings_builder, observability_projects_locations_update_settings_task,
    observability_projects_locations_buckets_get_builder, observability_projects_locations_buckets_get_task,
    observability_projects_locations_buckets_list_builder, observability_projects_locations_buckets_list_task,
    observability_projects_locations_buckets_datasets_get_builder, observability_projects_locations_buckets_datasets_get_task,
    observability_projects_locations_buckets_datasets_list_builder, observability_projects_locations_buckets_datasets_list_task,
    observability_projects_locations_buckets_datasets_links_create_builder, observability_projects_locations_buckets_datasets_links_create_task,
    observability_projects_locations_buckets_datasets_links_delete_builder, observability_projects_locations_buckets_datasets_links_delete_task,
    observability_projects_locations_buckets_datasets_links_get_builder, observability_projects_locations_buckets_datasets_links_get_task,
    observability_projects_locations_buckets_datasets_links_list_builder, observability_projects_locations_buckets_datasets_links_list_task,
    observability_projects_locations_buckets_datasets_links_patch_builder, observability_projects_locations_buckets_datasets_links_patch_task,
    observability_projects_locations_buckets_datasets_views_get_builder, observability_projects_locations_buckets_datasets_views_get_task,
    observability_projects_locations_buckets_datasets_views_list_builder, observability_projects_locations_buckets_datasets_views_list_task,
    observability_projects_locations_operations_cancel_builder, observability_projects_locations_operations_cancel_task,
    observability_projects_locations_operations_delete_builder, observability_projects_locations_operations_delete_task,
    observability_projects_locations_operations_get_builder, observability_projects_locations_operations_get_task,
    observability_projects_locations_operations_list_builder, observability_projects_locations_operations_list_task,
    observability_projects_locations_scopes_get_builder, observability_projects_locations_scopes_get_task,
    observability_projects_locations_scopes_patch_builder, observability_projects_locations_scopes_patch_task,
    observability_projects_locations_trace_scopes_create_builder, observability_projects_locations_trace_scopes_create_task,
    observability_projects_locations_trace_scopes_delete_builder, observability_projects_locations_trace_scopes_delete_task,
    observability_projects_locations_trace_scopes_get_builder, observability_projects_locations_trace_scopes_get_task,
    observability_projects_locations_trace_scopes_list_builder, observability_projects_locations_trace_scopes_list_task,
    observability_projects_locations_trace_scopes_patch_builder, observability_projects_locations_trace_scopes_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::observability::Bucket;
use crate::providers::gcp::clients::observability::Dataset;
use crate::providers::gcp::clients::observability::Empty;
use crate::providers::gcp::clients::observability::Link;
use crate::providers::gcp::clients::observability::ListBucketsResponse;
use crate::providers::gcp::clients::observability::ListDatasetsResponse;
use crate::providers::gcp::clients::observability::ListLinksResponse;
use crate::providers::gcp::clients::observability::ListLocationsResponse;
use crate::providers::gcp::clients::observability::ListOperationsResponse;
use crate::providers::gcp::clients::observability::ListTraceScopesResponse;
use crate::providers::gcp::clients::observability::ListViewsResponse;
use crate::providers::gcp::clients::observability::Location;
use crate::providers::gcp::clients::observability::Operation;
use crate::providers::gcp::clients::observability::Scope;
use crate::providers::gcp::clients::observability::Settings;
use crate::providers::gcp::clients::observability::TraceScope;
use crate::providers::gcp::clients::observability::View;
use crate::providers::gcp::clients::observability::ObservabilityFoldersLocationsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityFoldersLocationsGetSettingsArgs;
use crate::providers::gcp::clients::observability::ObservabilityFoldersLocationsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityFoldersLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::observability::ObservabilityFoldersLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::observability::ObservabilityFoldersLocationsOperationsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityFoldersLocationsOperationsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityFoldersLocationsUpdateSettingsArgs;
use crate::providers::gcp::clients::observability::ObservabilityOrganizationsLocationsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityOrganizationsLocationsGetSettingsArgs;
use crate::providers::gcp::clients::observability::ObservabilityOrganizationsLocationsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityOrganizationsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::observability::ObservabilityOrganizationsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::observability::ObservabilityOrganizationsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityOrganizationsLocationsOperationsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityOrganizationsLocationsUpdateSettingsArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsLinksCreateArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsLinksDeleteArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsLinksGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsLinksListArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsLinksPatchArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsViewsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsDatasetsViewsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsBucketsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsGetSettingsArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsScopesGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsScopesPatchArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsTraceScopesCreateArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsTraceScopesDeleteArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsTraceScopesGetArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsTraceScopesListArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsTraceScopesPatchArgs;
use crate::providers::gcp::clients::observability::ObservabilityProjectsLocationsUpdateSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ObservabilityProvider with automatic state tracking.
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
/// let provider = ObservabilityProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ObservabilityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ObservabilityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ObservabilityProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Observability folders locations get.
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
    pub fn observability_folders_locations_get(
        &self,
        args: &ObservabilityFoldersLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_folders_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_folders_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability folders locations get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_folders_locations_get_settings(
        &self,
        args: &ObservabilityFoldersLocationsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_folders_locations_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_folders_locations_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability folders locations list.
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
    pub fn observability_folders_locations_list(
        &self,
        args: &ObservabilityFoldersLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_folders_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_folders_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability folders locations update settings.
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
    pub fn observability_folders_locations_update_settings(
        &self,
        args: &ObservabilityFoldersLocationsUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_folders_locations_update_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_folders_locations_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability folders locations operations cancel.
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
    pub fn observability_folders_locations_operations_cancel(
        &self,
        args: &ObservabilityFoldersLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_folders_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_folders_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability folders locations operations delete.
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
    pub fn observability_folders_locations_operations_delete(
        &self,
        args: &ObservabilityFoldersLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_folders_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_folders_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability folders locations operations get.
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
    pub fn observability_folders_locations_operations_get(
        &self,
        args: &ObservabilityFoldersLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_folders_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_folders_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability folders locations operations list.
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
    pub fn observability_folders_locations_operations_list(
        &self,
        args: &ObservabilityFoldersLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_folders_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_folders_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability organizations locations get.
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
    pub fn observability_organizations_locations_get(
        &self,
        args: &ObservabilityOrganizationsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_organizations_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_organizations_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability organizations locations get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_organizations_locations_get_settings(
        &self,
        args: &ObservabilityOrganizationsLocationsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_organizations_locations_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_organizations_locations_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability organizations locations list.
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
    pub fn observability_organizations_locations_list(
        &self,
        args: &ObservabilityOrganizationsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_organizations_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_organizations_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability organizations locations update settings.
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
    pub fn observability_organizations_locations_update_settings(
        &self,
        args: &ObservabilityOrganizationsLocationsUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_organizations_locations_update_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_organizations_locations_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability organizations locations operations cancel.
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
    pub fn observability_organizations_locations_operations_cancel(
        &self,
        args: &ObservabilityOrganizationsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_organizations_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_organizations_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability organizations locations operations delete.
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
    pub fn observability_organizations_locations_operations_delete(
        &self,
        args: &ObservabilityOrganizationsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_organizations_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_organizations_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability organizations locations operations get.
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
    pub fn observability_organizations_locations_operations_get(
        &self,
        args: &ObservabilityOrganizationsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_organizations_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_organizations_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability organizations locations operations list.
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
    pub fn observability_organizations_locations_operations_list(
        &self,
        args: &ObservabilityOrganizationsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_organizations_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_organizations_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations get.
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
    pub fn observability_projects_locations_get(
        &self,
        args: &ObservabilityProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_get_settings(
        &self,
        args: &ObservabilityProjectsLocationsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations list.
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
    pub fn observability_projects_locations_list(
        &self,
        args: &ObservabilityProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations update settings.
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
    pub fn observability_projects_locations_update_settings(
        &self,
        args: &ObservabilityProjectsLocationsUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_update_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Bucket result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_buckets_get(
        &self,
        args: &ObservabilityProjectsLocationsBucketsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Bucket, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBucketsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_buckets_list(
        &self,
        args: &ObservabilityProjectsLocationsBucketsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBucketsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_buckets_datasets_get(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatasetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_buckets_datasets_list(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatasetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets links create.
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
    pub fn observability_projects_locations_buckets_datasets_links_create(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_links_create_builder(
            &self.http_client,
            &args.parent,
            &args.linkId,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets links delete.
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
    pub fn observability_projects_locations_buckets_datasets_links_delete(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets links get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Link result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_buckets_datasets_links_get(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsLinksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Link, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_links_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_links_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_buckets_datasets_links_list(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets links patch.
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
    pub fn observability_projects_locations_buckets_datasets_links_patch(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsLinksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_links_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_links_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the View result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_buckets_datasets_views_get(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<View, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations buckets datasets views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_buckets_datasets_views_list(
        &self,
        args: &ObservabilityProjectsLocationsBucketsDatasetsViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_buckets_datasets_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_buckets_datasets_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations operations cancel.
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
    pub fn observability_projects_locations_operations_cancel(
        &self,
        args: &ObservabilityProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations operations delete.
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
    pub fn observability_projects_locations_operations_delete(
        &self,
        args: &ObservabilityProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations operations get.
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
    pub fn observability_projects_locations_operations_get(
        &self,
        args: &ObservabilityProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations operations list.
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
    pub fn observability_projects_locations_operations_list(
        &self,
        args: &ObservabilityProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations scopes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Scope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_scopes_get(
        &self,
        args: &ObservabilityProjectsLocationsScopesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Scope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_scopes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_scopes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations scopes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Scope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn observability_projects_locations_scopes_patch(
        &self,
        args: &ObservabilityProjectsLocationsScopesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Scope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_scopes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_scopes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations trace scopes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TraceScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn observability_projects_locations_trace_scopes_create(
        &self,
        args: &ObservabilityProjectsLocationsTraceScopesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TraceScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_trace_scopes_create_builder(
            &self.http_client,
            &args.parent,
            &args.traceScopeId,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_trace_scopes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations trace scopes delete.
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
    pub fn observability_projects_locations_trace_scopes_delete(
        &self,
        args: &ObservabilityProjectsLocationsTraceScopesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_trace_scopes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_trace_scopes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations trace scopes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TraceScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_trace_scopes_get(
        &self,
        args: &ObservabilityProjectsLocationsTraceScopesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TraceScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_trace_scopes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_trace_scopes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations trace scopes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTraceScopesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn observability_projects_locations_trace_scopes_list(
        &self,
        args: &ObservabilityProjectsLocationsTraceScopesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTraceScopesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_trace_scopes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_trace_scopes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Observability projects locations trace scopes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TraceScope result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn observability_projects_locations_trace_scopes_patch(
        &self,
        args: &ObservabilityProjectsLocationsTraceScopesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TraceScope, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = observability_projects_locations_trace_scopes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = observability_projects_locations_trace_scopes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
