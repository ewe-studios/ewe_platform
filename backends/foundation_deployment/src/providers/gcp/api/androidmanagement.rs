//! AndroidmanagementProvider - State-aware androidmanagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       androidmanagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::androidmanagement::{
    androidmanagement_enterprises_create_builder, androidmanagement_enterprises_create_task,
    androidmanagement_enterprises_delete_builder, androidmanagement_enterprises_delete_task,
    androidmanagement_enterprises_generate_enterprise_upgrade_url_builder, androidmanagement_enterprises_generate_enterprise_upgrade_url_task,
    androidmanagement_enterprises_patch_builder, androidmanagement_enterprises_patch_task,
    androidmanagement_enterprises_devices_delete_builder, androidmanagement_enterprises_devices_delete_task,
    androidmanagement_enterprises_devices_issue_command_builder, androidmanagement_enterprises_devices_issue_command_task,
    androidmanagement_enterprises_devices_patch_builder, androidmanagement_enterprises_devices_patch_task,
    androidmanagement_enterprises_devices_operations_cancel_builder, androidmanagement_enterprises_devices_operations_cancel_task,
    androidmanagement_enterprises_enrollment_tokens_create_builder, androidmanagement_enterprises_enrollment_tokens_create_task,
    androidmanagement_enterprises_enrollment_tokens_delete_builder, androidmanagement_enterprises_enrollment_tokens_delete_task,
    androidmanagement_enterprises_migration_tokens_create_builder, androidmanagement_enterprises_migration_tokens_create_task,
    androidmanagement_enterprises_policies_delete_builder, androidmanagement_enterprises_policies_delete_task,
    androidmanagement_enterprises_policies_modify_policy_applications_builder, androidmanagement_enterprises_policies_modify_policy_applications_task,
    androidmanagement_enterprises_policies_patch_builder, androidmanagement_enterprises_policies_patch_task,
    androidmanagement_enterprises_policies_remove_policy_applications_builder, androidmanagement_enterprises_policies_remove_policy_applications_task,
    androidmanagement_enterprises_web_apps_create_builder, androidmanagement_enterprises_web_apps_create_task,
    androidmanagement_enterprises_web_apps_delete_builder, androidmanagement_enterprises_web_apps_delete_task,
    androidmanagement_enterprises_web_apps_patch_builder, androidmanagement_enterprises_web_apps_patch_task,
    androidmanagement_enterprises_web_tokens_create_builder, androidmanagement_enterprises_web_tokens_create_task,
    androidmanagement_signup_urls_create_builder, androidmanagement_signup_urls_create_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::androidmanagement::Device;
use crate::providers::gcp::clients::androidmanagement::Empty;
use crate::providers::gcp::clients::androidmanagement::EnrollmentToken;
use crate::providers::gcp::clients::androidmanagement::Enterprise;
use crate::providers::gcp::clients::androidmanagement::GenerateEnterpriseUpgradeUrlResponse;
use crate::providers::gcp::clients::androidmanagement::MigrationToken;
use crate::providers::gcp::clients::androidmanagement::ModifyPolicyApplicationsResponse;
use crate::providers::gcp::clients::androidmanagement::Operation;
use crate::providers::gcp::clients::androidmanagement::Policy;
use crate::providers::gcp::clients::androidmanagement::RemovePolicyApplicationsResponse;
use crate::providers::gcp::clients::androidmanagement::SignupUrl;
use crate::providers::gcp::clients::androidmanagement::WebApp;
use crate::providers::gcp::clients::androidmanagement::WebToken;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesCreateArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesDeleteArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesDevicesDeleteArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesDevicesIssueCommandArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesDevicesOperationsCancelArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesDevicesPatchArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesEnrollmentTokensCreateArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesEnrollmentTokensDeleteArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesGenerateEnterpriseUpgradeUrlArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesMigrationTokensCreateArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesPatchArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesPoliciesDeleteArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesPoliciesModifyPolicyApplicationsArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesPoliciesPatchArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesPoliciesRemovePolicyApplicationsArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesWebAppsCreateArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesWebAppsDeleteArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesWebAppsPatchArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementEnterprisesWebTokensCreateArgs;
use crate::providers::gcp::clients::androidmanagement::AndroidmanagementSignupUrlsCreateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AndroidmanagementProvider with automatic state tracking.
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
/// let provider = AndroidmanagementProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AndroidmanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AndroidmanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AndroidmanagementProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Androidmanagement enterprises create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Enterprise result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_create(
        &self,
        args: &AndroidmanagementEnterprisesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Enterprise, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_create_builder(
            &self.http_client,
            &args.agreementAccepted,
            &args.enterpriseToken,
            &args.projectId,
            &args.signupUrlName,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises delete.
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
    pub fn androidmanagement_enterprises_delete(
        &self,
        args: &AndroidmanagementEnterprisesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises generate enterprise upgrade url.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateEnterpriseUpgradeUrlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_generate_enterprise_upgrade_url(
        &self,
        args: &AndroidmanagementEnterprisesGenerateEnterpriseUpgradeUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateEnterpriseUpgradeUrlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_generate_enterprise_upgrade_url_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_generate_enterprise_upgrade_url_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Enterprise result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_patch(
        &self,
        args: &AndroidmanagementEnterprisesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Enterprise, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises devices delete.
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
    pub fn androidmanagement_enterprises_devices_delete(
        &self,
        args: &AndroidmanagementEnterprisesDevicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_devices_delete_builder(
            &self.http_client,
            &args.name,
            &args.wipeDataFlags,
            &args.wipeReasonMessage,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_devices_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises devices issue command.
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
    pub fn androidmanagement_enterprises_devices_issue_command(
        &self,
        args: &AndroidmanagementEnterprisesDevicesIssueCommandArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_devices_issue_command_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_devices_issue_command_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises devices patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Device result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_devices_patch(
        &self,
        args: &AndroidmanagementEnterprisesDevicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Device, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_devices_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_devices_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises devices operations cancel.
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
    pub fn androidmanagement_enterprises_devices_operations_cancel(
        &self,
        args: &AndroidmanagementEnterprisesDevicesOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_devices_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_devices_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises enrollment tokens create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EnrollmentToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_enrollment_tokens_create(
        &self,
        args: &AndroidmanagementEnterprisesEnrollmentTokensCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EnrollmentToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_enrollment_tokens_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_enrollment_tokens_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises enrollment tokens delete.
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
    pub fn androidmanagement_enterprises_enrollment_tokens_delete(
        &self,
        args: &AndroidmanagementEnterprisesEnrollmentTokensDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_enrollment_tokens_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_enrollment_tokens_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises migration tokens create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MigrationToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_migration_tokens_create(
        &self,
        args: &AndroidmanagementEnterprisesMigrationTokensCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MigrationToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_migration_tokens_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_migration_tokens_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises policies delete.
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
    pub fn androidmanagement_enterprises_policies_delete(
        &self,
        args: &AndroidmanagementEnterprisesPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises policies modify policy applications.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ModifyPolicyApplicationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_policies_modify_policy_applications(
        &self,
        args: &AndroidmanagementEnterprisesPoliciesModifyPolicyApplicationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ModifyPolicyApplicationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_policies_modify_policy_applications_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_policies_modify_policy_applications_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises policies patch.
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
    pub fn androidmanagement_enterprises_policies_patch(
        &self,
        args: &AndroidmanagementEnterprisesPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises policies remove policy applications.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemovePolicyApplicationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_policies_remove_policy_applications(
        &self,
        args: &AndroidmanagementEnterprisesPoliciesRemovePolicyApplicationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemovePolicyApplicationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_policies_remove_policy_applications_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_policies_remove_policy_applications_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises web apps create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_web_apps_create(
        &self,
        args: &AndroidmanagementEnterprisesWebAppsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_web_apps_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_web_apps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises web apps delete.
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
    pub fn androidmanagement_enterprises_web_apps_delete(
        &self,
        args: &AndroidmanagementEnterprisesWebAppsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_web_apps_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_web_apps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises web apps patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_web_apps_patch(
        &self,
        args: &AndroidmanagementEnterprisesWebAppsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_web_apps_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_web_apps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement enterprises web tokens create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_enterprises_web_tokens_create(
        &self,
        args: &AndroidmanagementEnterprisesWebTokensCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_enterprises_web_tokens_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_enterprises_web_tokens_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androidmanagement signup urls create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SignupUrl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androidmanagement_signup_urls_create(
        &self,
        args: &AndroidmanagementSignupUrlsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SignupUrl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androidmanagement_signup_urls_create_builder(
            &self.http_client,
            &args.adminEmail,
            &args.allowedDomains,
            &args.callbackUrl,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = androidmanagement_signup_urls_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
