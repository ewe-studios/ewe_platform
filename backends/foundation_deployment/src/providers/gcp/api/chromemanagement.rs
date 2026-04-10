//! ChromemanagementProvider - State-aware chromemanagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       chromemanagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::chromemanagement::{
    chromemanagement_customers_certificate_provisioning_processes_claim_builder, chromemanagement_customers_certificate_provisioning_processes_claim_task,
    chromemanagement_customers_certificate_provisioning_processes_set_failure_builder, chromemanagement_customers_certificate_provisioning_processes_set_failure_task,
    chromemanagement_customers_certificate_provisioning_processes_sign_data_builder, chromemanagement_customers_certificate_provisioning_processes_sign_data_task,
    chromemanagement_customers_certificate_provisioning_processes_upload_certificate_builder, chromemanagement_customers_certificate_provisioning_processes_upload_certificate_task,
    chromemanagement_customers_profiles_delete_builder, chromemanagement_customers_profiles_delete_task,
    chromemanagement_customers_profiles_commands_create_builder, chromemanagement_customers_profiles_commands_create_task,
    chromemanagement_customers_telemetry_notification_configs_create_builder, chromemanagement_customers_telemetry_notification_configs_create_task,
    chromemanagement_customers_telemetry_notification_configs_delete_builder, chromemanagement_customers_telemetry_notification_configs_delete_task,
    chromemanagement_customers_third_party_profile_users_move_builder, chromemanagement_customers_third_party_profile_users_move_task,
    chromemanagement_operations_cancel_builder, chromemanagement_operations_cancel_task,
    chromemanagement_operations_delete_builder, chromemanagement_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementV1TelemetryNotificationConfig;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1ChromeBrowserProfileCommand;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1ClaimCertificateProvisioningProcessResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1MoveThirdPartyProfileUserResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1SetFailureResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleChromeManagementVersionsV1UploadCertificateResponse;
use crate::providers::gcp::clients::chromemanagement::GoogleLongrunningOperation;
use crate::providers::gcp::clients::chromemanagement::GoogleProtobufEmpty;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesClaimArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesSetFailureArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesSignDataArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersCertificateProvisioningProcessesUploadCertificateArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersProfilesCommandsCreateArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersProfilesDeleteArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryNotificationConfigsCreateArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersTelemetryNotificationConfigsDeleteArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementCustomersThirdPartyProfileUsersMoveArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementOperationsCancelArgs;
use crate::providers::gcp::clients::chromemanagement::ChromemanagementOperationsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ChromemanagementProvider with automatic state tracking.
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
/// let provider = ChromemanagementProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ChromemanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ChromemanagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ChromemanagementProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Chromemanagement customers certificate provisioning processes claim.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1ClaimCertificateProvisioningProcessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_claim(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesClaimArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1ClaimCertificateProvisioningProcessResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_claim_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_claim_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes set failure.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1SetFailureResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_set_failure(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesSetFailureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1SetFailureResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_set_failure_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_set_failure_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes sign data.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_sign_data(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesSignDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_sign_data_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_sign_data_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers certificate provisioning processes upload certificate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1UploadCertificateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_certificate_provisioning_processes_upload_certificate(
        &self,
        args: &ChromemanagementCustomersCertificateProvisioningProcessesUploadCertificateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1UploadCertificateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_certificate_provisioning_processes_upload_certificate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_certificate_provisioning_processes_upload_certificate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers profiles delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_profiles_delete(
        &self,
        args: &ChromemanagementCustomersProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_profiles_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers profiles commands create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1ChromeBrowserProfileCommand result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_profiles_commands_create(
        &self,
        args: &ChromemanagementCustomersProfilesCommandsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1ChromeBrowserProfileCommand, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_profiles_commands_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_profiles_commands_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry notification configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementV1TelemetryNotificationConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_telemetry_notification_configs_create(
        &self,
        args: &ChromemanagementCustomersTelemetryNotificationConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementV1TelemetryNotificationConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_notification_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_notification_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers telemetry notification configs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_telemetry_notification_configs_delete(
        &self,
        args: &ChromemanagementCustomersTelemetryNotificationConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_telemetry_notification_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_telemetry_notification_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement customers third party profile users move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromeManagementVersionsV1MoveThirdPartyProfileUserResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_customers_third_party_profile_users_move(
        &self,
        args: &ChromemanagementCustomersThirdPartyProfileUsersMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromeManagementVersionsV1MoveThirdPartyProfileUserResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_customers_third_party_profile_users_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_customers_third_party_profile_users_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_operations_cancel(
        &self,
        args: &ChromemanagementOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromemanagement operations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromemanagement_operations_delete(
        &self,
        args: &ChromemanagementOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromemanagement_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = chromemanagement_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
