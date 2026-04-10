//! DomainsProvider - State-aware domains API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       domains API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::domains::{
    domains_projects_locations_registrations_configure_contact_settings_builder, domains_projects_locations_registrations_configure_contact_settings_task,
    domains_projects_locations_registrations_configure_dns_settings_builder, domains_projects_locations_registrations_configure_dns_settings_task,
    domains_projects_locations_registrations_configure_management_settings_builder, domains_projects_locations_registrations_configure_management_settings_task,
    domains_projects_locations_registrations_delete_builder, domains_projects_locations_registrations_delete_task,
    domains_projects_locations_registrations_export_builder, domains_projects_locations_registrations_export_task,
    domains_projects_locations_registrations_import_builder, domains_projects_locations_registrations_import_task,
    domains_projects_locations_registrations_initiate_push_transfer_builder, domains_projects_locations_registrations_initiate_push_transfer_task,
    domains_projects_locations_registrations_patch_builder, domains_projects_locations_registrations_patch_task,
    domains_projects_locations_registrations_register_builder, domains_projects_locations_registrations_register_task,
    domains_projects_locations_registrations_renew_domain_builder, domains_projects_locations_registrations_renew_domain_task,
    domains_projects_locations_registrations_reset_authorization_code_builder, domains_projects_locations_registrations_reset_authorization_code_task,
    domains_projects_locations_registrations_set_iam_policy_builder, domains_projects_locations_registrations_set_iam_policy_task,
    domains_projects_locations_registrations_test_iam_permissions_builder, domains_projects_locations_registrations_test_iam_permissions_task,
    domains_projects_locations_registrations_transfer_builder, domains_projects_locations_registrations_transfer_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::domains::AuthorizationCode;
use crate::providers::gcp::clients::domains::Operation;
use crate::providers::gcp::clients::domains::Policy;
use crate::providers::gcp::clients::domains::TestIamPermissionsResponse;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsConfigureContactSettingsArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsConfigureDnsSettingsArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsConfigureManagementSettingsArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsDeleteArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsExportArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsImportArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsInitiatePushTransferArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsPatchArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsRegisterArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsRenewDomainArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsResetAuthorizationCodeArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsSetIamPolicyArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::domains::DomainsProjectsLocationsRegistrationsTransferArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DomainsProvider with automatic state tracking.
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
/// let provider = DomainsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DomainsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DomainsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DomainsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Domains projects locations registrations configure contact settings.
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
    pub fn domains_projects_locations_registrations_configure_contact_settings(
        &self,
        args: &DomainsProjectsLocationsRegistrationsConfigureContactSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_configure_contact_settings_builder(
            &self.http_client,
            &args.registration,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_configure_contact_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations configure dns settings.
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
    pub fn domains_projects_locations_registrations_configure_dns_settings(
        &self,
        args: &DomainsProjectsLocationsRegistrationsConfigureDnsSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_configure_dns_settings_builder(
            &self.http_client,
            &args.registration,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_configure_dns_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations configure management settings.
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
    pub fn domains_projects_locations_registrations_configure_management_settings(
        &self,
        args: &DomainsProjectsLocationsRegistrationsConfigureManagementSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_configure_management_settings_builder(
            &self.http_client,
            &args.registration,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_configure_management_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations delete.
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
    pub fn domains_projects_locations_registrations_delete(
        &self,
        args: &DomainsProjectsLocationsRegistrationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations export.
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
    pub fn domains_projects_locations_registrations_export(
        &self,
        args: &DomainsProjectsLocationsRegistrationsExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_export_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations import.
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
    pub fn domains_projects_locations_registrations_import(
        &self,
        args: &DomainsProjectsLocationsRegistrationsImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_import_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations initiate push transfer.
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
    pub fn domains_projects_locations_registrations_initiate_push_transfer(
        &self,
        args: &DomainsProjectsLocationsRegistrationsInitiatePushTransferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_initiate_push_transfer_builder(
            &self.http_client,
            &args.registration,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_initiate_push_transfer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations patch.
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
    pub fn domains_projects_locations_registrations_patch(
        &self,
        args: &DomainsProjectsLocationsRegistrationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations register.
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
    pub fn domains_projects_locations_registrations_register(
        &self,
        args: &DomainsProjectsLocationsRegistrationsRegisterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_register_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_register_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations renew domain.
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
    pub fn domains_projects_locations_registrations_renew_domain(
        &self,
        args: &DomainsProjectsLocationsRegistrationsRenewDomainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_renew_domain_builder(
            &self.http_client,
            &args.registration,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_renew_domain_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations reset authorization code.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthorizationCode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn domains_projects_locations_registrations_reset_authorization_code(
        &self,
        args: &DomainsProjectsLocationsRegistrationsResetAuthorizationCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthorizationCode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_reset_authorization_code_builder(
            &self.http_client,
            &args.registration,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_reset_authorization_code_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations set iam policy.
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
    pub fn domains_projects_locations_registrations_set_iam_policy(
        &self,
        args: &DomainsProjectsLocationsRegistrationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations test iam permissions.
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
    pub fn domains_projects_locations_registrations_test_iam_permissions(
        &self,
        args: &DomainsProjectsLocationsRegistrationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Domains projects locations registrations transfer.
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
    pub fn domains_projects_locations_registrations_transfer(
        &self,
        args: &DomainsProjectsLocationsRegistrationsTransferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = domains_projects_locations_registrations_transfer_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = domains_projects_locations_registrations_transfer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
