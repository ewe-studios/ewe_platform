//! ApigatewayProvider - State-aware apigateway API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       apigateway API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::apigateway::{
    apigateway_projects_locations_get_builder, apigateway_projects_locations_get_task,
    apigateway_projects_locations_list_builder, apigateway_projects_locations_list_task,
    apigateway_projects_locations_apis_create_builder, apigateway_projects_locations_apis_create_task,
    apigateway_projects_locations_apis_delete_builder, apigateway_projects_locations_apis_delete_task,
    apigateway_projects_locations_apis_get_builder, apigateway_projects_locations_apis_get_task,
    apigateway_projects_locations_apis_get_iam_policy_builder, apigateway_projects_locations_apis_get_iam_policy_task,
    apigateway_projects_locations_apis_list_builder, apigateway_projects_locations_apis_list_task,
    apigateway_projects_locations_apis_patch_builder, apigateway_projects_locations_apis_patch_task,
    apigateway_projects_locations_apis_set_iam_policy_builder, apigateway_projects_locations_apis_set_iam_policy_task,
    apigateway_projects_locations_apis_test_iam_permissions_builder, apigateway_projects_locations_apis_test_iam_permissions_task,
    apigateway_projects_locations_apis_configs_create_builder, apigateway_projects_locations_apis_configs_create_task,
    apigateway_projects_locations_apis_configs_delete_builder, apigateway_projects_locations_apis_configs_delete_task,
    apigateway_projects_locations_apis_configs_get_builder, apigateway_projects_locations_apis_configs_get_task,
    apigateway_projects_locations_apis_configs_get_iam_policy_builder, apigateway_projects_locations_apis_configs_get_iam_policy_task,
    apigateway_projects_locations_apis_configs_list_builder, apigateway_projects_locations_apis_configs_list_task,
    apigateway_projects_locations_apis_configs_patch_builder, apigateway_projects_locations_apis_configs_patch_task,
    apigateway_projects_locations_apis_configs_set_iam_policy_builder, apigateway_projects_locations_apis_configs_set_iam_policy_task,
    apigateway_projects_locations_apis_configs_test_iam_permissions_builder, apigateway_projects_locations_apis_configs_test_iam_permissions_task,
    apigateway_projects_locations_gateways_create_builder, apigateway_projects_locations_gateways_create_task,
    apigateway_projects_locations_gateways_delete_builder, apigateway_projects_locations_gateways_delete_task,
    apigateway_projects_locations_gateways_get_builder, apigateway_projects_locations_gateways_get_task,
    apigateway_projects_locations_gateways_get_iam_policy_builder, apigateway_projects_locations_gateways_get_iam_policy_task,
    apigateway_projects_locations_gateways_list_builder, apigateway_projects_locations_gateways_list_task,
    apigateway_projects_locations_gateways_patch_builder, apigateway_projects_locations_gateways_patch_task,
    apigateway_projects_locations_gateways_set_iam_policy_builder, apigateway_projects_locations_gateways_set_iam_policy_task,
    apigateway_projects_locations_gateways_test_iam_permissions_builder, apigateway_projects_locations_gateways_test_iam_permissions_task,
    apigateway_projects_locations_operations_cancel_builder, apigateway_projects_locations_operations_cancel_task,
    apigateway_projects_locations_operations_delete_builder, apigateway_projects_locations_operations_delete_task,
    apigateway_projects_locations_operations_get_builder, apigateway_projects_locations_operations_get_task,
    apigateway_projects_locations_operations_list_builder, apigateway_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::apigateway::ApigatewayApi;
use crate::providers::gcp::clients::apigateway::ApigatewayApiConfig;
use crate::providers::gcp::clients::apigateway::ApigatewayGateway;
use crate::providers::gcp::clients::apigateway::ApigatewayListApiConfigsResponse;
use crate::providers::gcp::clients::apigateway::ApigatewayListApisResponse;
use crate::providers::gcp::clients::apigateway::ApigatewayListGatewaysResponse;
use crate::providers::gcp::clients::apigateway::ApigatewayListLocationsResponse;
use crate::providers::gcp::clients::apigateway::ApigatewayListOperationsResponse;
use crate::providers::gcp::clients::apigateway::ApigatewayLocation;
use crate::providers::gcp::clients::apigateway::ApigatewayOperation;
use crate::providers::gcp::clients::apigateway::ApigatewayPolicy;
use crate::providers::gcp::clients::apigateway::ApigatewayTestIamPermissionsResponse;
use crate::providers::gcp::clients::apigateway::Empty;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisConfigsCreateArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisConfigsDeleteArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisConfigsGetArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisConfigsGetIamPolicyArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisConfigsListArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisConfigsPatchArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisConfigsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisConfigsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisCreateArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisDeleteArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisGetArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisGetIamPolicyArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisListArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisPatchArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisSetIamPolicyArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsApisTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGatewaysCreateArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGatewaysDeleteArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGatewaysGetArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGatewaysGetIamPolicyArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGatewaysListArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGatewaysPatchArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGatewaysSetIamPolicyArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGatewaysTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsGetArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsListArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::apigateway::ApigatewayProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ApigatewayProvider with automatic state tracking.
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
/// let provider = ApigatewayProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ApigatewayProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ApigatewayProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ApigatewayProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Apigateway projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_get(
        &self,
        args: &ApigatewayProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayLocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_list(
        &self,
        args: &ApigatewayProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_apis_create(
        &self,
        args: &ApigatewayProjectsLocationsApisCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_apis_delete(
        &self,
        args: &ApigatewayProjectsLocationsApisDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayApi result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_apis_get(
        &self,
        args: &ApigatewayProjectsLocationsApisGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayApi, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_apis_get_iam_policy(
        &self,
        args: &ApigatewayProjectsLocationsApisGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayListApisResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_apis_list(
        &self,
        args: &ApigatewayProjectsLocationsApisListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayListApisResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_apis_patch(
        &self,
        args: &ApigatewayProjectsLocationsApisPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_apis_set_iam_policy(
        &self,
        args: &ApigatewayProjectsLocationsApisSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayTestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_apis_test_iam_permissions(
        &self,
        args: &ApigatewayProjectsLocationsApisTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayTestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_apis_configs_create(
        &self,
        args: &ApigatewayProjectsLocationsApisConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis configs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_apis_configs_delete(
        &self,
        args: &ApigatewayProjectsLocationsApisConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayApiConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_apis_configs_get(
        &self,
        args: &ApigatewayProjectsLocationsApisConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayApiConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_configs_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis configs get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_apis_configs_get_iam_policy(
        &self,
        args: &ApigatewayProjectsLocationsApisConfigsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_configs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_configs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayListApiConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_apis_configs_list(
        &self,
        args: &ApigatewayProjectsLocationsApisConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayListApiConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_apis_configs_patch(
        &self,
        args: &ApigatewayProjectsLocationsApisConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis configs set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_apis_configs_set_iam_policy(
        &self,
        args: &ApigatewayProjectsLocationsApisConfigsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_configs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_configs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations apis configs test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayTestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_apis_configs_test_iam_permissions(
        &self,
        args: &ApigatewayProjectsLocationsApisConfigsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayTestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_apis_configs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_apis_configs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations gateways create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_gateways_create(
        &self,
        args: &ApigatewayProjectsLocationsGatewaysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_gateways_create_builder(
            &self.http_client,
            &args.parent,
            &args.gatewayId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_gateways_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations gateways delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_gateways_delete(
        &self,
        args: &ApigatewayProjectsLocationsGatewaysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_gateways_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_gateways_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations gateways get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayGateway result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_gateways_get(
        &self,
        args: &ApigatewayProjectsLocationsGatewaysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayGateway, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_gateways_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_gateways_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations gateways get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_gateways_get_iam_policy(
        &self,
        args: &ApigatewayProjectsLocationsGatewaysGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_gateways_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_gateways_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations gateways list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayListGatewaysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_gateways_list(
        &self,
        args: &ApigatewayProjectsLocationsGatewaysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayListGatewaysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_gateways_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_gateways_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations gateways patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_gateways_patch(
        &self,
        args: &ApigatewayProjectsLocationsGatewaysPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_gateways_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_gateways_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations gateways set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigateway_projects_locations_gateways_set_iam_policy(
        &self,
        args: &ApigatewayProjectsLocationsGatewaysSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_gateways_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_gateways_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations gateways test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayTestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_gateways_test_iam_permissions(
        &self,
        args: &ApigatewayProjectsLocationsGatewaysTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayTestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_gateways_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_gateways_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations operations cancel.
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
    pub fn apigateway_projects_locations_operations_cancel(
        &self,
        args: &ApigatewayProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations operations delete.
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
    pub fn apigateway_projects_locations_operations_delete(
        &self,
        args: &ApigatewayProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_operations_get(
        &self,
        args: &ApigatewayProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigateway projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApigatewayListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apigateway_projects_locations_operations_list(
        &self,
        args: &ApigatewayProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApigatewayListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigateway_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = apigateway_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
