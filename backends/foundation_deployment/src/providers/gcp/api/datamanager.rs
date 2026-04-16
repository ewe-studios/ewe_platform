//! DatamanagerProvider - State-aware datamanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       datamanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::datamanager::{
    datamanager_account_types_accounts_insights_retrieve_builder, datamanager_account_types_accounts_insights_retrieve_task,
    datamanager_account_types_accounts_partner_links_create_builder, datamanager_account_types_accounts_partner_links_create_task,
    datamanager_account_types_accounts_partner_links_delete_builder, datamanager_account_types_accounts_partner_links_delete_task,
    datamanager_account_types_accounts_partner_links_search_builder, datamanager_account_types_accounts_partner_links_search_task,
    datamanager_account_types_accounts_user_list_direct_licenses_create_builder, datamanager_account_types_accounts_user_list_direct_licenses_create_task,
    datamanager_account_types_accounts_user_list_direct_licenses_get_builder, datamanager_account_types_accounts_user_list_direct_licenses_get_task,
    datamanager_account_types_accounts_user_list_direct_licenses_list_builder, datamanager_account_types_accounts_user_list_direct_licenses_list_task,
    datamanager_account_types_accounts_user_list_direct_licenses_patch_builder, datamanager_account_types_accounts_user_list_direct_licenses_patch_task,
    datamanager_account_types_accounts_user_list_global_licenses_create_builder, datamanager_account_types_accounts_user_list_global_licenses_create_task,
    datamanager_account_types_accounts_user_list_global_licenses_get_builder, datamanager_account_types_accounts_user_list_global_licenses_get_task,
    datamanager_account_types_accounts_user_list_global_licenses_list_builder, datamanager_account_types_accounts_user_list_global_licenses_list_task,
    datamanager_account_types_accounts_user_list_global_licenses_patch_builder, datamanager_account_types_accounts_user_list_global_licenses_patch_task,
    datamanager_account_types_accounts_user_list_global_licenses_user_list_global_license_customer_infos_list_builder, datamanager_account_types_accounts_user_list_global_licenses_user_list_global_license_customer_infos_list_task,
    datamanager_account_types_accounts_user_lists_create_builder, datamanager_account_types_accounts_user_lists_create_task,
    datamanager_account_types_accounts_user_lists_delete_builder, datamanager_account_types_accounts_user_lists_delete_task,
    datamanager_account_types_accounts_user_lists_get_builder, datamanager_account_types_accounts_user_lists_get_task,
    datamanager_account_types_accounts_user_lists_list_builder, datamanager_account_types_accounts_user_lists_list_task,
    datamanager_account_types_accounts_user_lists_patch_builder, datamanager_account_types_accounts_user_lists_patch_task,
    datamanager_audience_members_ingest_builder, datamanager_audience_members_ingest_task,
    datamanager_audience_members_remove_builder, datamanager_audience_members_remove_task,
    datamanager_events_ingest_builder, datamanager_events_ingest_task,
    datamanager_request_status_retrieve_builder, datamanager_request_status_retrieve_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datamanager::Empty;
use crate::providers::gcp::clients::datamanager::IngestAudienceMembersResponse;
use crate::providers::gcp::clients::datamanager::IngestEventsResponse;
use crate::providers::gcp::clients::datamanager::ListUserListDirectLicensesResponse;
use crate::providers::gcp::clients::datamanager::ListUserListGlobalLicenseCustomerInfosResponse;
use crate::providers::gcp::clients::datamanager::ListUserListGlobalLicensesResponse;
use crate::providers::gcp::clients::datamanager::ListUserListsResponse;
use crate::providers::gcp::clients::datamanager::PartnerLink;
use crate::providers::gcp::clients::datamanager::RemoveAudienceMembersResponse;
use crate::providers::gcp::clients::datamanager::RetrieveInsightsResponse;
use crate::providers::gcp::clients::datamanager::RetrieveRequestStatusResponse;
use crate::providers::gcp::clients::datamanager::SearchPartnerLinksResponse;
use crate::providers::gcp::clients::datamanager::UserList;
use crate::providers::gcp::clients::datamanager::UserListDirectLicense;
use crate::providers::gcp::clients::datamanager::UserListGlobalLicense;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsInsightsRetrieveArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsPartnerLinksCreateArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsPartnerLinksDeleteArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsPartnerLinksSearchArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListDirectLicensesCreateArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListDirectLicensesGetArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListDirectLicensesListArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListDirectLicensesPatchArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListGlobalLicensesCreateArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListGlobalLicensesGetArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListGlobalLicensesListArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListGlobalLicensesPatchArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListGlobalLicensesUserListGlobalLicenseCustomerInfosListArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListsCreateArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListsDeleteArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListsGetArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListsListArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListsPatchArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerRequestStatusRetrieveArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatamanagerProvider with automatic state tracking.
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
/// let provider = DatamanagerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DatamanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DatamanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DatamanagerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DatamanagerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Datamanager account types accounts insights retrieve.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetrieveInsightsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_insights_retrieve(
        &self,
        args: &DatamanagerAccountTypesAccountsInsightsRetrieveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetrieveInsightsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_insights_retrieve_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_insights_retrieve_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts partner links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PartnerLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamanager_account_types_accounts_partner_links_create(
        &self,
        args: &DatamanagerAccountTypesAccountsPartnerLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PartnerLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_partner_links_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_partner_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts partner links delete.
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
    pub fn datamanager_account_types_accounts_partner_links_delete(
        &self,
        args: &DatamanagerAccountTypesAccountsPartnerLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_partner_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_partner_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts partner links search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchPartnerLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_partner_links_search(
        &self,
        args: &DatamanagerAccountTypesAccountsPartnerLinksSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchPartnerLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_partner_links_search_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_partner_links_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list direct licenses create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserListDirectLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamanager_account_types_accounts_user_list_direct_licenses_create(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListDirectLicensesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserListDirectLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_direct_licenses_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_direct_licenses_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list direct licenses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserListDirectLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_list_direct_licenses_get(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListDirectLicensesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserListDirectLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_direct_licenses_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_direct_licenses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list direct licenses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUserListDirectLicensesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_list_direct_licenses_list(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListDirectLicensesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUserListDirectLicensesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_direct_licenses_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_direct_licenses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list direct licenses patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserListDirectLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_list_direct_licenses_patch(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListDirectLicensesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserListDirectLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_direct_licenses_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_direct_licenses_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list global licenses create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserListGlobalLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamanager_account_types_accounts_user_list_global_licenses_create(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListGlobalLicensesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserListGlobalLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_global_licenses_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_global_licenses_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list global licenses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserListGlobalLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_list_global_licenses_get(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListGlobalLicensesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserListGlobalLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_global_licenses_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_global_licenses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list global licenses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUserListGlobalLicensesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_list_global_licenses_list(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListGlobalLicensesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUserListGlobalLicensesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_global_licenses_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_global_licenses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list global licenses patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserListGlobalLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_list_global_licenses_patch(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListGlobalLicensesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserListGlobalLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_global_licenses_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_global_licenses_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user list global licenses user list global license customer infos list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUserListGlobalLicenseCustomerInfosResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_list_global_licenses_user_list_global_license_customer_infos_list(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListGlobalLicensesUserListGlobalLicenseCustomerInfosListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUserListGlobalLicenseCustomerInfosResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_list_global_licenses_user_list_global_license_customer_infos_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_list_global_licenses_user_list_global_license_customer_infos_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user lists create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamanager_account_types_accounts_user_lists_create(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_lists_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_lists_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user lists delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_lists_delete(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_lists_delete_builder(
            &self.http_client,
            &args.name,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_lists_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user lists get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_lists_get(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_lists_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_lists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user lists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUserListsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_lists_list(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUserListsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_lists_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_lists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user lists patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_account_types_accounts_user_lists_patch(
        &self,
        args: &DatamanagerAccountTypesAccountsUserListsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_account_types_accounts_user_lists_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_account_types_accounts_user_lists_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager audience members ingest.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IngestAudienceMembersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamanager_audience_members_ingest(
        &self,
        args: &DatamanagerAudienceMembersIngestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IngestAudienceMembersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_audience_members_ingest_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_audience_members_ingest_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager audience members remove.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemoveAudienceMembersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamanager_audience_members_remove(
        &self,
        args: &DatamanagerAudienceMembersRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemoveAudienceMembersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_audience_members_remove_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_audience_members_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager events ingest.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IngestEventsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datamanager_events_ingest(
        &self,
        args: &DatamanagerEventsIngestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IngestEventsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_events_ingest_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_events_ingest_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager request status retrieve.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetrieveRequestStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datamanager_request_status_retrieve(
        &self,
        args: &DatamanagerRequestStatusRetrieveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetrieveRequestStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datamanager_request_status_retrieve_builder(
            &self.http_client,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datamanager_request_status_retrieve_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
