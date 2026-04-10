//! ParametermanagerProvider - State-aware parametermanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       parametermanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::parametermanager::{
    parametermanager_projects_locations_get_builder, parametermanager_projects_locations_get_task,
    parametermanager_projects_locations_list_builder, parametermanager_projects_locations_list_task,
    parametermanager_projects_locations_parameters_create_builder, parametermanager_projects_locations_parameters_create_task,
    parametermanager_projects_locations_parameters_delete_builder, parametermanager_projects_locations_parameters_delete_task,
    parametermanager_projects_locations_parameters_get_builder, parametermanager_projects_locations_parameters_get_task,
    parametermanager_projects_locations_parameters_list_builder, parametermanager_projects_locations_parameters_list_task,
    parametermanager_projects_locations_parameters_patch_builder, parametermanager_projects_locations_parameters_patch_task,
    parametermanager_projects_locations_parameters_versions_create_builder, parametermanager_projects_locations_parameters_versions_create_task,
    parametermanager_projects_locations_parameters_versions_delete_builder, parametermanager_projects_locations_parameters_versions_delete_task,
    parametermanager_projects_locations_parameters_versions_get_builder, parametermanager_projects_locations_parameters_versions_get_task,
    parametermanager_projects_locations_parameters_versions_list_builder, parametermanager_projects_locations_parameters_versions_list_task,
    parametermanager_projects_locations_parameters_versions_patch_builder, parametermanager_projects_locations_parameters_versions_patch_task,
    parametermanager_projects_locations_parameters_versions_render_builder, parametermanager_projects_locations_parameters_versions_render_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::parametermanager::Empty;
use crate::providers::gcp::clients::parametermanager::ListLocationsResponse;
use crate::providers::gcp::clients::parametermanager::ListParameterVersionsResponse;
use crate::providers::gcp::clients::parametermanager::ListParametersResponse;
use crate::providers::gcp::clients::parametermanager::Location;
use crate::providers::gcp::clients::parametermanager::Parameter;
use crate::providers::gcp::clients::parametermanager::ParameterVersion;
use crate::providers::gcp::clients::parametermanager::RenderParameterVersionResponse;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsGetArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsListArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersCreateArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersDeleteArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersGetArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersListArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersPatchArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersVersionsCreateArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersVersionsDeleteArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersVersionsGetArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersVersionsListArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersVersionsPatchArgs;
use crate::providers::gcp::clients::parametermanager::ParametermanagerProjectsLocationsParametersVersionsRenderArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ParametermanagerProvider with automatic state tracking.
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
/// let provider = ParametermanagerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ParametermanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ParametermanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ParametermanagerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Parametermanager projects locations get.
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
    pub fn parametermanager_projects_locations_get(
        &self,
        args: &ParametermanagerProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations list.
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
    pub fn parametermanager_projects_locations_list(
        &self,
        args: &ParametermanagerProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Parameter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn parametermanager_projects_locations_parameters_create(
        &self,
        args: &ParametermanagerProjectsLocationsParametersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Parameter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_create_builder(
            &self.http_client,
            &args.parent,
            &args.parameterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters delete.
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
    pub fn parametermanager_projects_locations_parameters_delete(
        &self,
        args: &ParametermanagerProjectsLocationsParametersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Parameter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn parametermanager_projects_locations_parameters_get(
        &self,
        args: &ParametermanagerProjectsLocationsParametersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Parameter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListParametersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn parametermanager_projects_locations_parameters_list(
        &self,
        args: &ParametermanagerProjectsLocationsParametersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListParametersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Parameter result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn parametermanager_projects_locations_parameters_patch(
        &self,
        args: &ParametermanagerProjectsLocationsParametersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Parameter, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ParameterVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn parametermanager_projects_locations_parameters_versions_create(
        &self,
        args: &ParametermanagerProjectsLocationsParametersVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ParameterVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_versions_create_builder(
            &self.http_client,
            &args.parent,
            &args.parameterVersionId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters versions delete.
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
    pub fn parametermanager_projects_locations_parameters_versions_delete(
        &self,
        args: &ParametermanagerProjectsLocationsParametersVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ParameterVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn parametermanager_projects_locations_parameters_versions_get(
        &self,
        args: &ParametermanagerProjectsLocationsParametersVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ParameterVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_versions_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListParameterVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn parametermanager_projects_locations_parameters_versions_list(
        &self,
        args: &ParametermanagerProjectsLocationsParametersVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListParameterVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ParameterVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn parametermanager_projects_locations_parameters_versions_patch(
        &self,
        args: &ParametermanagerProjectsLocationsParametersVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ParameterVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Parametermanager projects locations parameters versions render.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RenderParameterVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn parametermanager_projects_locations_parameters_versions_render(
        &self,
        args: &ParametermanagerProjectsLocationsParametersVersionsRenderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RenderParameterVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = parametermanager_projects_locations_parameters_versions_render_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = parametermanager_projects_locations_parameters_versions_render_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
