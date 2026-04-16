//! ServicemanagementProvider - State-aware servicemanagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       servicemanagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::servicemanagement::{
    servicemanagement_operations_get_builder, servicemanagement_operations_get_task,
    servicemanagement_operations_list_builder, servicemanagement_operations_list_task,
    servicemanagement_services_create_builder, servicemanagement_services_create_task,
    servicemanagement_services_delete_builder, servicemanagement_services_delete_task,
    servicemanagement_services_generate_config_report_builder, servicemanagement_services_generate_config_report_task,
    servicemanagement_services_get_builder, servicemanagement_services_get_task,
    servicemanagement_services_get_config_builder, servicemanagement_services_get_config_task,
    servicemanagement_services_get_iam_policy_builder, servicemanagement_services_get_iam_policy_task,
    servicemanagement_services_list_builder, servicemanagement_services_list_task,
    servicemanagement_services_set_iam_policy_builder, servicemanagement_services_set_iam_policy_task,
    servicemanagement_services_test_iam_permissions_builder, servicemanagement_services_test_iam_permissions_task,
    servicemanagement_services_undelete_builder, servicemanagement_services_undelete_task,
    servicemanagement_services_configs_create_builder, servicemanagement_services_configs_create_task,
    servicemanagement_services_configs_get_builder, servicemanagement_services_configs_get_task,
    servicemanagement_services_configs_list_builder, servicemanagement_services_configs_list_task,
    servicemanagement_services_configs_submit_builder, servicemanagement_services_configs_submit_task,
    servicemanagement_services_consumers_get_iam_policy_builder, servicemanagement_services_consumers_get_iam_policy_task,
    servicemanagement_services_consumers_set_iam_policy_builder, servicemanagement_services_consumers_set_iam_policy_task,
    servicemanagement_services_consumers_test_iam_permissions_builder, servicemanagement_services_consumers_test_iam_permissions_task,
    servicemanagement_services_rollouts_create_builder, servicemanagement_services_rollouts_create_task,
    servicemanagement_services_rollouts_get_builder, servicemanagement_services_rollouts_get_task,
    servicemanagement_services_rollouts_list_builder, servicemanagement_services_rollouts_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::servicemanagement::GenerateConfigReportResponse;
use crate::providers::gcp::clients::servicemanagement::ListOperationsResponse;
use crate::providers::gcp::clients::servicemanagement::ListServiceConfigsResponse;
use crate::providers::gcp::clients::servicemanagement::ListServiceRolloutsResponse;
use crate::providers::gcp::clients::servicemanagement::ListServicesResponse;
use crate::providers::gcp::clients::servicemanagement::ManagedService;
use crate::providers::gcp::clients::servicemanagement::Operation;
use crate::providers::gcp::clients::servicemanagement::Policy;
use crate::providers::gcp::clients::servicemanagement::Rollout;
use crate::providers::gcp::clients::servicemanagement::Service;
use crate::providers::gcp::clients::servicemanagement::TestIamPermissionsResponse;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementOperationsGetArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementOperationsListArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesConfigsCreateArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesConfigsGetArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesConfigsListArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesConfigsSubmitArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesConsumersGetIamPolicyArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesConsumersSetIamPolicyArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesConsumersTestIamPermissionsArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesDeleteArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesGetArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesGetConfigArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesGetIamPolicyArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesListArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesRolloutsCreateArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesRolloutsGetArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesRolloutsListArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesSetIamPolicyArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesTestIamPermissionsArgs;
use crate::providers::gcp::clients::servicemanagement::ServicemanagementServicesUndeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ServicemanagementProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
/// * `R` - DNS resolver type for HTTP client
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let http_client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
/// let client = ProviderClient::new("my-project", "dev", state_store, http_client);
/// let provider = ServicemanagementProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ServicemanagementProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ServicemanagementProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ServicemanagementProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ServicemanagementProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Servicemanagement operations get.
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
    pub fn servicemanagement_operations_get(
        &self,
        args: &ServicemanagementOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement operations list.
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
    pub fn servicemanagement_operations_list(
        &self,
        args: &ServicemanagementOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.name,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services create.
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
    pub fn servicemanagement_services_create(
        &self,
        args: &ServicemanagementServicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services delete.
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
    pub fn servicemanagement_services_delete(
        &self,
        args: &ServicemanagementServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_delete_builder(
            &self.http_client,
            &args.serviceName,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services generate config report.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateConfigReportResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicemanagement_services_generate_config_report(
        &self,
        args: &ServicemanagementServicesGenerateConfigReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateConfigReportResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_generate_config_report_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_generate_config_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedService result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicemanagement_services_get(
        &self,
        args: &ServicemanagementServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedService, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_get_builder(
            &self.http_client,
            &args.serviceName,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services get config.
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
    pub fn servicemanagement_services_get_config(
        &self,
        args: &ServicemanagementServicesGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_get_config_builder(
            &self.http_client,
            &args.serviceName,
            &args.configId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services get iam policy.
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
    pub fn servicemanagement_services_get_iam_policy(
        &self,
        args: &ServicemanagementServicesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services list.
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
    pub fn servicemanagement_services_list(
        &self,
        args: &ServicemanagementServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_list_builder(
            &self.http_client,
            &args.consumerId,
            &args.pageSize,
            &args.pageToken,
            &args.producerProjectId,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services set iam policy.
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
    pub fn servicemanagement_services_set_iam_policy(
        &self,
        args: &ServicemanagementServicesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services test iam permissions.
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
    pub fn servicemanagement_services_test_iam_permissions(
        &self,
        args: &ServicemanagementServicesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services undelete.
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
    pub fn servicemanagement_services_undelete(
        &self,
        args: &ServicemanagementServicesUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_undelete_builder(
            &self.http_client,
            &args.serviceName,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services configs create.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn servicemanagement_services_configs_create(
        &self,
        args: &ServicemanagementServicesConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_configs_create_builder(
            &self.http_client,
            &args.serviceName,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services configs get.
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
    pub fn servicemanagement_services_configs_get(
        &self,
        args: &ServicemanagementServicesConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_configs_get_builder(
            &self.http_client,
            &args.serviceName,
            &args.configId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicemanagement_services_configs_list(
        &self,
        args: &ServicemanagementServicesConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_configs_list_builder(
            &self.http_client,
            &args.serviceName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services configs submit.
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
    pub fn servicemanagement_services_configs_submit(
        &self,
        args: &ServicemanagementServicesConfigsSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_configs_submit_builder(
            &self.http_client,
            &args.serviceName,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_configs_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services consumers get iam policy.
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
    pub fn servicemanagement_services_consumers_get_iam_policy(
        &self,
        args: &ServicemanagementServicesConsumersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_consumers_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_consumers_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services consumers set iam policy.
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
    pub fn servicemanagement_services_consumers_set_iam_policy(
        &self,
        args: &ServicemanagementServicesConsumersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_consumers_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_consumers_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services consumers test iam permissions.
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
    pub fn servicemanagement_services_consumers_test_iam_permissions(
        &self,
        args: &ServicemanagementServicesConsumersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_consumers_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_consumers_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services rollouts create.
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
    pub fn servicemanagement_services_rollouts_create(
        &self,
        args: &ServicemanagementServicesRolloutsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_rollouts_create_builder(
            &self.http_client,
            &args.serviceName,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_rollouts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services rollouts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Rollout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicemanagement_services_rollouts_get(
        &self,
        args: &ServicemanagementServicesRolloutsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rollout, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_rollouts_get_builder(
            &self.http_client,
            &args.serviceName,
            &args.rolloutId,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_rollouts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicemanagement services rollouts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceRolloutsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicemanagement_services_rollouts_list(
        &self,
        args: &ServicemanagementServicesRolloutsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceRolloutsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicemanagement_services_rollouts_list_builder(
            &self.http_client,
            &args.serviceName,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = servicemanagement_services_rollouts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
