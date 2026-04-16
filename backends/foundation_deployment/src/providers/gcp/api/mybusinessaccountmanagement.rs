//! MybusinessaccountmanagementProvider - State-aware mybusinessaccountmanagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       mybusinessaccountmanagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::mybusinessaccountmanagement::{
    mybusinessaccountmanagement_accounts_create_builder, mybusinessaccountmanagement_accounts_create_task,
    mybusinessaccountmanagement_accounts_get_builder, mybusinessaccountmanagement_accounts_get_task,
    mybusinessaccountmanagement_accounts_list_builder, mybusinessaccountmanagement_accounts_list_task,
    mybusinessaccountmanagement_accounts_patch_builder, mybusinessaccountmanagement_accounts_patch_task,
    mybusinessaccountmanagement_accounts_admins_create_builder, mybusinessaccountmanagement_accounts_admins_create_task,
    mybusinessaccountmanagement_accounts_admins_delete_builder, mybusinessaccountmanagement_accounts_admins_delete_task,
    mybusinessaccountmanagement_accounts_admins_list_builder, mybusinessaccountmanagement_accounts_admins_list_task,
    mybusinessaccountmanagement_accounts_admins_patch_builder, mybusinessaccountmanagement_accounts_admins_patch_task,
    mybusinessaccountmanagement_accounts_invitations_accept_builder, mybusinessaccountmanagement_accounts_invitations_accept_task,
    mybusinessaccountmanagement_accounts_invitations_decline_builder, mybusinessaccountmanagement_accounts_invitations_decline_task,
    mybusinessaccountmanagement_accounts_invitations_list_builder, mybusinessaccountmanagement_accounts_invitations_list_task,
    mybusinessaccountmanagement_locations_transfer_builder, mybusinessaccountmanagement_locations_transfer_task,
    mybusinessaccountmanagement_locations_admins_create_builder, mybusinessaccountmanagement_locations_admins_create_task,
    mybusinessaccountmanagement_locations_admins_delete_builder, mybusinessaccountmanagement_locations_admins_delete_task,
    mybusinessaccountmanagement_locations_admins_list_builder, mybusinessaccountmanagement_locations_admins_list_task,
    mybusinessaccountmanagement_locations_admins_patch_builder, mybusinessaccountmanagement_locations_admins_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::mybusinessaccountmanagement::Account;
use crate::providers::gcp::clients::mybusinessaccountmanagement::Admin;
use crate::providers::gcp::clients::mybusinessaccountmanagement::Empty;
use crate::providers::gcp::clients::mybusinessaccountmanagement::ListAccountAdminsResponse;
use crate::providers::gcp::clients::mybusinessaccountmanagement::ListAccountsResponse;
use crate::providers::gcp::clients::mybusinessaccountmanagement::ListInvitationsResponse;
use crate::providers::gcp::clients::mybusinessaccountmanagement::ListLocationAdminsResponse;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsAdminsCreateArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsAdminsDeleteArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsAdminsListArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsAdminsPatchArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsGetArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsInvitationsAcceptArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsInvitationsDeclineArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsInvitationsListArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsListArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementAccountsPatchArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementLocationsAdminsCreateArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementLocationsAdminsDeleteArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementLocationsAdminsListArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementLocationsAdminsPatchArgs;
use crate::providers::gcp::clients::mybusinessaccountmanagement::MybusinessaccountmanagementLocationsTransferArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MybusinessaccountmanagementProvider with automatic state tracking.
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
/// let provider = MybusinessaccountmanagementProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct MybusinessaccountmanagementProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> MybusinessaccountmanagementProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new MybusinessaccountmanagementProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new MybusinessaccountmanagementProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Mybusinessaccountmanagement accounts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessaccountmanagement_accounts_create(
        &self,
        args: &MybusinessaccountmanagementAccountsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessaccountmanagement_accounts_get(
        &self,
        args: &MybusinessaccountmanagementAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessaccountmanagement_accounts_list(
        &self,
        args: &MybusinessaccountmanagementAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.parentAccount,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessaccountmanagement_accounts_patch(
        &self,
        args: &MybusinessaccountmanagementAccountsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts admins create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Admin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessaccountmanagement_accounts_admins_create(
        &self,
        args: &MybusinessaccountmanagementAccountsAdminsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Admin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_admins_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_admins_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts admins delete.
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
    pub fn mybusinessaccountmanagement_accounts_admins_delete(
        &self,
        args: &MybusinessaccountmanagementAccountsAdminsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_admins_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_admins_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts admins list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountAdminsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessaccountmanagement_accounts_admins_list(
        &self,
        args: &MybusinessaccountmanagementAccountsAdminsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountAdminsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_admins_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_admins_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts admins patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Admin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessaccountmanagement_accounts_admins_patch(
        &self,
        args: &MybusinessaccountmanagementAccountsAdminsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Admin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_admins_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_admins_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts invitations accept.
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
    pub fn mybusinessaccountmanagement_accounts_invitations_accept(
        &self,
        args: &MybusinessaccountmanagementAccountsInvitationsAcceptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_invitations_accept_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_invitations_accept_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts invitations decline.
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
    pub fn mybusinessaccountmanagement_accounts_invitations_decline(
        &self,
        args: &MybusinessaccountmanagementAccountsInvitationsDeclineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_invitations_decline_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_invitations_decline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement accounts invitations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInvitationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessaccountmanagement_accounts_invitations_list(
        &self,
        args: &MybusinessaccountmanagementAccountsInvitationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInvitationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_accounts_invitations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_accounts_invitations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement locations transfer.
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
    pub fn mybusinessaccountmanagement_locations_transfer(
        &self,
        args: &MybusinessaccountmanagementLocationsTransferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_locations_transfer_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_locations_transfer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement locations admins create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Admin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessaccountmanagement_locations_admins_create(
        &self,
        args: &MybusinessaccountmanagementLocationsAdminsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Admin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_locations_admins_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_locations_admins_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement locations admins delete.
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
    pub fn mybusinessaccountmanagement_locations_admins_delete(
        &self,
        args: &MybusinessaccountmanagementLocationsAdminsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_locations_admins_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_locations_admins_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement locations admins list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationAdminsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessaccountmanagement_locations_admins_list(
        &self,
        args: &MybusinessaccountmanagementLocationsAdminsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationAdminsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_locations_admins_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_locations_admins_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessaccountmanagement locations admins patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Admin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessaccountmanagement_locations_admins_patch(
        &self,
        args: &MybusinessaccountmanagementLocationsAdminsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Admin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessaccountmanagement_locations_admins_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessaccountmanagement_locations_admins_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
