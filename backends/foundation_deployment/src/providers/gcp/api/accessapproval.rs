//! AccessapprovalProvider - State-aware accessapproval API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       accessapproval API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::accessapproval::{
    accessapproval_folders_delete_access_approval_settings_builder, accessapproval_folders_delete_access_approval_settings_task,
    accessapproval_folders_get_access_approval_settings_builder, accessapproval_folders_get_access_approval_settings_task,
    accessapproval_folders_get_service_account_builder, accessapproval_folders_get_service_account_task,
    accessapproval_folders_update_access_approval_settings_builder, accessapproval_folders_update_access_approval_settings_task,
    accessapproval_folders_approval_requests_approve_builder, accessapproval_folders_approval_requests_approve_task,
    accessapproval_folders_approval_requests_dismiss_builder, accessapproval_folders_approval_requests_dismiss_task,
    accessapproval_folders_approval_requests_get_builder, accessapproval_folders_approval_requests_get_task,
    accessapproval_folders_approval_requests_invalidate_builder, accessapproval_folders_approval_requests_invalidate_task,
    accessapproval_folders_approval_requests_list_builder, accessapproval_folders_approval_requests_list_task,
    accessapproval_organizations_delete_access_approval_settings_builder, accessapproval_organizations_delete_access_approval_settings_task,
    accessapproval_organizations_get_access_approval_settings_builder, accessapproval_organizations_get_access_approval_settings_task,
    accessapproval_organizations_get_service_account_builder, accessapproval_organizations_get_service_account_task,
    accessapproval_organizations_update_access_approval_settings_builder, accessapproval_organizations_update_access_approval_settings_task,
    accessapproval_organizations_approval_requests_approve_builder, accessapproval_organizations_approval_requests_approve_task,
    accessapproval_organizations_approval_requests_dismiss_builder, accessapproval_organizations_approval_requests_dismiss_task,
    accessapproval_organizations_approval_requests_get_builder, accessapproval_organizations_approval_requests_get_task,
    accessapproval_organizations_approval_requests_invalidate_builder, accessapproval_organizations_approval_requests_invalidate_task,
    accessapproval_organizations_approval_requests_list_builder, accessapproval_organizations_approval_requests_list_task,
    accessapproval_projects_delete_access_approval_settings_builder, accessapproval_projects_delete_access_approval_settings_task,
    accessapproval_projects_get_access_approval_settings_builder, accessapproval_projects_get_access_approval_settings_task,
    accessapproval_projects_get_service_account_builder, accessapproval_projects_get_service_account_task,
    accessapproval_projects_update_access_approval_settings_builder, accessapproval_projects_update_access_approval_settings_task,
    accessapproval_projects_approval_requests_approve_builder, accessapproval_projects_approval_requests_approve_task,
    accessapproval_projects_approval_requests_dismiss_builder, accessapproval_projects_approval_requests_dismiss_task,
    accessapproval_projects_approval_requests_get_builder, accessapproval_projects_approval_requests_get_task,
    accessapproval_projects_approval_requests_invalidate_builder, accessapproval_projects_approval_requests_invalidate_task,
    accessapproval_projects_approval_requests_list_builder, accessapproval_projects_approval_requests_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::accessapproval::AccessApprovalServiceAccount;
use crate::providers::gcp::clients::accessapproval::AccessApprovalSettings;
use crate::providers::gcp::clients::accessapproval::ApprovalRequest;
use crate::providers::gcp::clients::accessapproval::Empty;
use crate::providers::gcp::clients::accessapproval::ListApprovalRequestsResponse;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersApprovalRequestsApproveArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersApprovalRequestsDismissArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersApprovalRequestsGetArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersApprovalRequestsInvalidateArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersApprovalRequestsListArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersDeleteAccessApprovalSettingsArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersGetAccessApprovalSettingsArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersGetServiceAccountArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalFoldersUpdateAccessApprovalSettingsArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsApprovalRequestsApproveArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsApprovalRequestsDismissArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsApprovalRequestsGetArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsApprovalRequestsInvalidateArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsApprovalRequestsListArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsDeleteAccessApprovalSettingsArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsGetAccessApprovalSettingsArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsGetServiceAccountArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalOrganizationsUpdateAccessApprovalSettingsArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsApprovalRequestsApproveArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsApprovalRequestsDismissArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsApprovalRequestsGetArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsApprovalRequestsInvalidateArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsApprovalRequestsListArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsDeleteAccessApprovalSettingsArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsGetAccessApprovalSettingsArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsGetServiceAccountArgs;
use crate::providers::gcp::clients::accessapproval::AccessapprovalProjectsUpdateAccessApprovalSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AccessapprovalProvider with automatic state tracking.
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
/// let provider = AccessapprovalProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AccessapprovalProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AccessapprovalProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AccessapprovalProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AccessapprovalProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Accessapproval folders delete access approval settings.
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
    pub fn accessapproval_folders_delete_access_approval_settings(
        &self,
        args: &AccessapprovalFoldersDeleteAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_delete_access_approval_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_delete_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval folders get access approval settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_folders_get_access_approval_settings(
        &self,
        args: &AccessapprovalFoldersGetAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_get_access_approval_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_get_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval folders get service account.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalServiceAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_folders_get_service_account(
        &self,
        args: &AccessapprovalFoldersGetServiceAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalServiceAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_get_service_account_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_get_service_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval folders update access approval settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_folders_update_access_approval_settings(
        &self,
        args: &AccessapprovalFoldersUpdateAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_update_access_approval_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_update_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval folders approval requests approve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_folders_approval_requests_approve(
        &self,
        args: &AccessapprovalFoldersApprovalRequestsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_approval_requests_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_approval_requests_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval folders approval requests dismiss.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_folders_approval_requests_dismiss(
        &self,
        args: &AccessapprovalFoldersApprovalRequestsDismissArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_approval_requests_dismiss_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_approval_requests_dismiss_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval folders approval requests get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_folders_approval_requests_get(
        &self,
        args: &AccessapprovalFoldersApprovalRequestsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_approval_requests_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_approval_requests_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval folders approval requests invalidate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_folders_approval_requests_invalidate(
        &self,
        args: &AccessapprovalFoldersApprovalRequestsInvalidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_approval_requests_invalidate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_approval_requests_invalidate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval folders approval requests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApprovalRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_folders_approval_requests_list(
        &self,
        args: &AccessapprovalFoldersApprovalRequestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApprovalRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_folders_approval_requests_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_folders_approval_requests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations delete access approval settings.
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
    pub fn accessapproval_organizations_delete_access_approval_settings(
        &self,
        args: &AccessapprovalOrganizationsDeleteAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_delete_access_approval_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_delete_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations get access approval settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_organizations_get_access_approval_settings(
        &self,
        args: &AccessapprovalOrganizationsGetAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_get_access_approval_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_get_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations get service account.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalServiceAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_organizations_get_service_account(
        &self,
        args: &AccessapprovalOrganizationsGetServiceAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalServiceAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_get_service_account_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_get_service_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations update access approval settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_organizations_update_access_approval_settings(
        &self,
        args: &AccessapprovalOrganizationsUpdateAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_update_access_approval_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_update_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations approval requests approve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_organizations_approval_requests_approve(
        &self,
        args: &AccessapprovalOrganizationsApprovalRequestsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_approval_requests_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_approval_requests_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations approval requests dismiss.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_organizations_approval_requests_dismiss(
        &self,
        args: &AccessapprovalOrganizationsApprovalRequestsDismissArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_approval_requests_dismiss_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_approval_requests_dismiss_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations approval requests get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_organizations_approval_requests_get(
        &self,
        args: &AccessapprovalOrganizationsApprovalRequestsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_approval_requests_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_approval_requests_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations approval requests invalidate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_organizations_approval_requests_invalidate(
        &self,
        args: &AccessapprovalOrganizationsApprovalRequestsInvalidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_approval_requests_invalidate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_approval_requests_invalidate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval organizations approval requests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApprovalRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_organizations_approval_requests_list(
        &self,
        args: &AccessapprovalOrganizationsApprovalRequestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApprovalRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_organizations_approval_requests_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_organizations_approval_requests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects delete access approval settings.
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
    pub fn accessapproval_projects_delete_access_approval_settings(
        &self,
        args: &AccessapprovalProjectsDeleteAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_delete_access_approval_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_delete_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects get access approval settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_projects_get_access_approval_settings(
        &self,
        args: &AccessapprovalProjectsGetAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_get_access_approval_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_get_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects get service account.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalServiceAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_projects_get_service_account(
        &self,
        args: &AccessapprovalProjectsGetServiceAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalServiceAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_get_service_account_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_get_service_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects update access approval settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AccessApprovalSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_projects_update_access_approval_settings(
        &self,
        args: &AccessapprovalProjectsUpdateAccessApprovalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AccessApprovalSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_update_access_approval_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_update_access_approval_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects approval requests approve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_projects_approval_requests_approve(
        &self,
        args: &AccessapprovalProjectsApprovalRequestsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_approval_requests_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_approval_requests_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects approval requests dismiss.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn accessapproval_projects_approval_requests_dismiss(
        &self,
        args: &AccessapprovalProjectsApprovalRequestsDismissArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_approval_requests_dismiss_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_approval_requests_dismiss_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects approval requests get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_projects_approval_requests_get(
        &self,
        args: &AccessapprovalProjectsApprovalRequestsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_approval_requests_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_approval_requests_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects approval requests invalidate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApprovalRequest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_projects_approval_requests_invalidate(
        &self,
        args: &AccessapprovalProjectsApprovalRequestsInvalidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApprovalRequest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_approval_requests_invalidate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_approval_requests_invalidate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Accessapproval projects approval requests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApprovalRequestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn accessapproval_projects_approval_requests_list(
        &self,
        args: &AccessapprovalProjectsApprovalRequestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApprovalRequestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = accessapproval_projects_approval_requests_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = accessapproval_projects_approval_requests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
