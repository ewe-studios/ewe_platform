//! NotebooksProvider - State-aware notebooks API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       notebooks API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::notebooks::{
    notebooks_projects_locations_get_builder, notebooks_projects_locations_get_task,
    notebooks_projects_locations_list_builder, notebooks_projects_locations_list_task,
    notebooks_projects_locations_instances_check_authorization_builder, notebooks_projects_locations_instances_check_authorization_task,
    notebooks_projects_locations_instances_check_upgradability_builder, notebooks_projects_locations_instances_check_upgradability_task,
    notebooks_projects_locations_instances_create_builder, notebooks_projects_locations_instances_create_task,
    notebooks_projects_locations_instances_delete_builder, notebooks_projects_locations_instances_delete_task,
    notebooks_projects_locations_instances_diagnose_builder, notebooks_projects_locations_instances_diagnose_task,
    notebooks_projects_locations_instances_generate_access_token_builder, notebooks_projects_locations_instances_generate_access_token_task,
    notebooks_projects_locations_instances_get_builder, notebooks_projects_locations_instances_get_task,
    notebooks_projects_locations_instances_get_config_builder, notebooks_projects_locations_instances_get_config_task,
    notebooks_projects_locations_instances_get_iam_policy_builder, notebooks_projects_locations_instances_get_iam_policy_task,
    notebooks_projects_locations_instances_list_builder, notebooks_projects_locations_instances_list_task,
    notebooks_projects_locations_instances_patch_builder, notebooks_projects_locations_instances_patch_task,
    notebooks_projects_locations_instances_report_info_system_builder, notebooks_projects_locations_instances_report_info_system_task,
    notebooks_projects_locations_instances_reset_builder, notebooks_projects_locations_instances_reset_task,
    notebooks_projects_locations_instances_resize_disk_builder, notebooks_projects_locations_instances_resize_disk_task,
    notebooks_projects_locations_instances_restore_builder, notebooks_projects_locations_instances_restore_task,
    notebooks_projects_locations_instances_rollback_builder, notebooks_projects_locations_instances_rollback_task,
    notebooks_projects_locations_instances_set_iam_policy_builder, notebooks_projects_locations_instances_set_iam_policy_task,
    notebooks_projects_locations_instances_start_builder, notebooks_projects_locations_instances_start_task,
    notebooks_projects_locations_instances_stop_builder, notebooks_projects_locations_instances_stop_task,
    notebooks_projects_locations_instances_test_iam_permissions_builder, notebooks_projects_locations_instances_test_iam_permissions_task,
    notebooks_projects_locations_instances_upgrade_builder, notebooks_projects_locations_instances_upgrade_task,
    notebooks_projects_locations_instances_upgrade_system_builder, notebooks_projects_locations_instances_upgrade_system_task,
    notebooks_projects_locations_operations_cancel_builder, notebooks_projects_locations_operations_cancel_task,
    notebooks_projects_locations_operations_delete_builder, notebooks_projects_locations_operations_delete_task,
    notebooks_projects_locations_operations_get_builder, notebooks_projects_locations_operations_get_task,
    notebooks_projects_locations_operations_list_builder, notebooks_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::notebooks::CheckAuthorizationResponse;
use crate::providers::gcp::clients::notebooks::CheckInstanceUpgradabilityResponse;
use crate::providers::gcp::clients::notebooks::Config;
use crate::providers::gcp::clients::notebooks::Empty;
use crate::providers::gcp::clients::notebooks::GenerateAccessTokenResponse;
use crate::providers::gcp::clients::notebooks::Instance;
use crate::providers::gcp::clients::notebooks::ListInstancesResponse;
use crate::providers::gcp::clients::notebooks::ListLocationsResponse;
use crate::providers::gcp::clients::notebooks::ListOperationsResponse;
use crate::providers::gcp::clients::notebooks::Location;
use crate::providers::gcp::clients::notebooks::Operation;
use crate::providers::gcp::clients::notebooks::Policy;
use crate::providers::gcp::clients::notebooks::TestIamPermissionsResponse;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsGetArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesCheckAuthorizationArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesCheckUpgradabilityArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesCreateArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesDeleteArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesDiagnoseArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesGenerateAccessTokenArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesGetArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesGetConfigArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesGetIamPolicyArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesListArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesPatchArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesReportInfoSystemArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesResetArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesResizeDiskArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesRestoreArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesRollbackArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesSetIamPolicyArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesStartArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesStopArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesTestIamPermissionsArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesUpgradeArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsInstancesUpgradeSystemArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsListArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::notebooks::NotebooksProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NotebooksProvider with automatic state tracking.
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
/// let provider = NotebooksProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct NotebooksProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> NotebooksProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new NotebooksProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Notebooks projects locations get.
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
    pub fn notebooks_projects_locations_get(
        &self,
        args: &NotebooksProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations list.
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
    pub fn notebooks_projects_locations_list(
        &self,
        args: &NotebooksProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances check authorization.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckAuthorizationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn notebooks_projects_locations_instances_check_authorization(
        &self,
        args: &NotebooksProjectsLocationsInstancesCheckAuthorizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckAuthorizationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_check_authorization_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_check_authorization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances check upgradability.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckInstanceUpgradabilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn notebooks_projects_locations_instances_check_upgradability(
        &self,
        args: &NotebooksProjectsLocationsInstancesCheckUpgradabilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckInstanceUpgradabilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_check_upgradability_builder(
            &self.http_client,
            &args.notebookInstance,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_check_upgradability_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances create.
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
    pub fn notebooks_projects_locations_instances_create(
        &self,
        args: &NotebooksProjectsLocationsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.instanceId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances delete.
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
    pub fn notebooks_projects_locations_instances_delete(
        &self,
        args: &NotebooksProjectsLocationsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances diagnose.
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
    pub fn notebooks_projects_locations_instances_diagnose(
        &self,
        args: &NotebooksProjectsLocationsInstancesDiagnoseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_diagnose_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_diagnose_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances generate access token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateAccessTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn notebooks_projects_locations_instances_generate_access_token(
        &self,
        args: &NotebooksProjectsLocationsInstancesGenerateAccessTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateAccessTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_generate_access_token_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_generate_access_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Instance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn notebooks_projects_locations_instances_get(
        &self,
        args: &NotebooksProjectsLocationsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances get config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Config result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn notebooks_projects_locations_instances_get_config(
        &self,
        args: &NotebooksProjectsLocationsInstancesGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Config, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances get iam policy.
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
    pub fn notebooks_projects_locations_instances_get_iam_policy(
        &self,
        args: &NotebooksProjectsLocationsInstancesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInstancesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn notebooks_projects_locations_instances_list(
        &self,
        args: &NotebooksProjectsLocationsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances patch.
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
    pub fn notebooks_projects_locations_instances_patch(
        &self,
        args: &NotebooksProjectsLocationsInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances report info system.
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
    pub fn notebooks_projects_locations_instances_report_info_system(
        &self,
        args: &NotebooksProjectsLocationsInstancesReportInfoSystemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_report_info_system_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_report_info_system_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances reset.
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
    pub fn notebooks_projects_locations_instances_reset(
        &self,
        args: &NotebooksProjectsLocationsInstancesResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_reset_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances resize disk.
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
    pub fn notebooks_projects_locations_instances_resize_disk(
        &self,
        args: &NotebooksProjectsLocationsInstancesResizeDiskArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_resize_disk_builder(
            &self.http_client,
            &args.notebookInstance,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_resize_disk_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances restore.
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
    pub fn notebooks_projects_locations_instances_restore(
        &self,
        args: &NotebooksProjectsLocationsInstancesRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances rollback.
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
    pub fn notebooks_projects_locations_instances_rollback(
        &self,
        args: &NotebooksProjectsLocationsInstancesRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances set iam policy.
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
    pub fn notebooks_projects_locations_instances_set_iam_policy(
        &self,
        args: &NotebooksProjectsLocationsInstancesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances start.
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
    pub fn notebooks_projects_locations_instances_start(
        &self,
        args: &NotebooksProjectsLocationsInstancesStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances stop.
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
    pub fn notebooks_projects_locations_instances_stop(
        &self,
        args: &NotebooksProjectsLocationsInstancesStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances test iam permissions.
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
    pub fn notebooks_projects_locations_instances_test_iam_permissions(
        &self,
        args: &NotebooksProjectsLocationsInstancesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances upgrade.
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
    pub fn notebooks_projects_locations_instances_upgrade(
        &self,
        args: &NotebooksProjectsLocationsInstancesUpgradeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_upgrade_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_upgrade_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations instances upgrade system.
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
    pub fn notebooks_projects_locations_instances_upgrade_system(
        &self,
        args: &NotebooksProjectsLocationsInstancesUpgradeSystemArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_instances_upgrade_system_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_instances_upgrade_system_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations operations cancel.
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
    pub fn notebooks_projects_locations_operations_cancel(
        &self,
        args: &NotebooksProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations operations delete.
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
    pub fn notebooks_projects_locations_operations_delete(
        &self,
        args: &NotebooksProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations operations get.
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
    pub fn notebooks_projects_locations_operations_get(
        &self,
        args: &NotebooksProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Notebooks projects locations operations list.
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
    pub fn notebooks_projects_locations_operations_list(
        &self,
        args: &NotebooksProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = notebooks_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = notebooks_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
