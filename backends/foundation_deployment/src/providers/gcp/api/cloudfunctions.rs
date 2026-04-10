//! CloudfunctionsProvider - State-aware cloudfunctions API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudfunctions API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudfunctions::{
    cloudfunctions_projects_locations_functions_abort_function_upgrade_builder, cloudfunctions_projects_locations_functions_abort_function_upgrade_task,
    cloudfunctions_projects_locations_functions_commit_function_upgrade_builder, cloudfunctions_projects_locations_functions_commit_function_upgrade_task,
    cloudfunctions_projects_locations_functions_commit_function_upgrade_as_gen2_builder, cloudfunctions_projects_locations_functions_commit_function_upgrade_as_gen2_task,
    cloudfunctions_projects_locations_functions_create_builder, cloudfunctions_projects_locations_functions_create_task,
    cloudfunctions_projects_locations_functions_delete_builder, cloudfunctions_projects_locations_functions_delete_task,
    cloudfunctions_projects_locations_functions_detach_function_builder, cloudfunctions_projects_locations_functions_detach_function_task,
    cloudfunctions_projects_locations_functions_generate_download_url_builder, cloudfunctions_projects_locations_functions_generate_download_url_task,
    cloudfunctions_projects_locations_functions_generate_upload_url_builder, cloudfunctions_projects_locations_functions_generate_upload_url_task,
    cloudfunctions_projects_locations_functions_patch_builder, cloudfunctions_projects_locations_functions_patch_task,
    cloudfunctions_projects_locations_functions_redirect_function_upgrade_traffic_builder, cloudfunctions_projects_locations_functions_redirect_function_upgrade_traffic_task,
    cloudfunctions_projects_locations_functions_rollback_function_upgrade_traffic_builder, cloudfunctions_projects_locations_functions_rollback_function_upgrade_traffic_task,
    cloudfunctions_projects_locations_functions_set_iam_policy_builder, cloudfunctions_projects_locations_functions_set_iam_policy_task,
    cloudfunctions_projects_locations_functions_setup_function_upgrade_config_builder, cloudfunctions_projects_locations_functions_setup_function_upgrade_config_task,
    cloudfunctions_projects_locations_functions_test_iam_permissions_builder, cloudfunctions_projects_locations_functions_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudfunctions::GenerateDownloadUrlResponse;
use crate::providers::gcp::clients::cloudfunctions::GenerateUploadUrlResponse;
use crate::providers::gcp::clients::cloudfunctions::Operation;
use crate::providers::gcp::clients::cloudfunctions::Policy;
use crate::providers::gcp::clients::cloudfunctions::TestIamPermissionsResponse;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsAbortFunctionUpgradeArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsCommitFunctionUpgradeArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsCommitFunctionUpgradeAsGen2Args;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsCreateArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsDeleteArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsDetachFunctionArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsGenerateDownloadUrlArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsGenerateUploadUrlArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsPatchArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsRedirectFunctionUpgradeTrafficArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsRollbackFunctionUpgradeTrafficArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsSetupFunctionUpgradeConfigArgs;
use crate::providers::gcp::clients::cloudfunctions::CloudfunctionsProjectsLocationsFunctionsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudfunctionsProvider with automatic state tracking.
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
/// let provider = CloudfunctionsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CloudfunctionsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudfunctionsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudfunctionsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Cloudfunctions projects locations functions abort function upgrade.
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
    pub fn cloudfunctions_projects_locations_functions_abort_function_upgrade(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsAbortFunctionUpgradeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_abort_function_upgrade_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_abort_function_upgrade_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions commit function upgrade.
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
    pub fn cloudfunctions_projects_locations_functions_commit_function_upgrade(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsCommitFunctionUpgradeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_commit_function_upgrade_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_commit_function_upgrade_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions commit function upgrade as gen2.
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
    pub fn cloudfunctions_projects_locations_functions_commit_function_upgrade_as_gen2(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsCommitFunctionUpgradeAsGen2Args,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_commit_function_upgrade_as_gen2_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_commit_function_upgrade_as_gen2_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions create.
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
    pub fn cloudfunctions_projects_locations_functions_create(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_create_builder(
            &self.http_client,
            &args.parent,
            &args.functionId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions delete.
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
    pub fn cloudfunctions_projects_locations_functions_delete(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions detach function.
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
    pub fn cloudfunctions_projects_locations_functions_detach_function(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsDetachFunctionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_detach_function_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_detach_function_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions generate download url.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateDownloadUrlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudfunctions_projects_locations_functions_generate_download_url(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsGenerateDownloadUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateDownloadUrlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_generate_download_url_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_generate_download_url_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions generate upload url.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateUploadUrlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudfunctions_projects_locations_functions_generate_upload_url(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsGenerateUploadUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateUploadUrlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_generate_upload_url_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_generate_upload_url_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions patch.
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
    pub fn cloudfunctions_projects_locations_functions_patch(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions redirect function upgrade traffic.
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
    pub fn cloudfunctions_projects_locations_functions_redirect_function_upgrade_traffic(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsRedirectFunctionUpgradeTrafficArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_redirect_function_upgrade_traffic_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_redirect_function_upgrade_traffic_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions rollback function upgrade traffic.
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
    pub fn cloudfunctions_projects_locations_functions_rollback_function_upgrade_traffic(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsRollbackFunctionUpgradeTrafficArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_rollback_function_upgrade_traffic_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_rollback_function_upgrade_traffic_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions set iam policy.
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
    pub fn cloudfunctions_projects_locations_functions_set_iam_policy(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions setup function upgrade config.
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
    pub fn cloudfunctions_projects_locations_functions_setup_function_upgrade_config(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsSetupFunctionUpgradeConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_setup_function_upgrade_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_setup_function_upgrade_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudfunctions projects locations functions test iam permissions.
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
    pub fn cloudfunctions_projects_locations_functions_test_iam_permissions(
        &self,
        args: &CloudfunctionsProjectsLocationsFunctionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudfunctions_projects_locations_functions_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudfunctions_projects_locations_functions_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
