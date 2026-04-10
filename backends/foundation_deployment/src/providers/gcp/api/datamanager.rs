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
    datamanager_account_types_accounts_user_list_direct_licenses_create_builder, datamanager_account_types_accounts_user_list_direct_licenses_create_task,
    datamanager_account_types_accounts_user_list_direct_licenses_patch_builder, datamanager_account_types_accounts_user_list_direct_licenses_patch_task,
    datamanager_account_types_accounts_user_list_global_licenses_create_builder, datamanager_account_types_accounts_user_list_global_licenses_create_task,
    datamanager_account_types_accounts_user_list_global_licenses_patch_builder, datamanager_account_types_accounts_user_list_global_licenses_patch_task,
    datamanager_account_types_accounts_user_lists_create_builder, datamanager_account_types_accounts_user_lists_create_task,
    datamanager_account_types_accounts_user_lists_delete_builder, datamanager_account_types_accounts_user_lists_delete_task,
    datamanager_account_types_accounts_user_lists_patch_builder, datamanager_account_types_accounts_user_lists_patch_task,
    datamanager_audience_members_ingest_builder, datamanager_audience_members_ingest_task,
    datamanager_audience_members_remove_builder, datamanager_audience_members_remove_task,
    datamanager_events_ingest_builder, datamanager_events_ingest_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datamanager::Empty;
use crate::providers::gcp::clients::datamanager::IngestAudienceMembersResponse;
use crate::providers::gcp::clients::datamanager::IngestEventsResponse;
use crate::providers::gcp::clients::datamanager::PartnerLink;
use crate::providers::gcp::clients::datamanager::RemoveAudienceMembersResponse;
use crate::providers::gcp::clients::datamanager::RetrieveInsightsResponse;
use crate::providers::gcp::clients::datamanager::UserList;
use crate::providers::gcp::clients::datamanager::UserListDirectLicense;
use crate::providers::gcp::clients::datamanager::UserListGlobalLicense;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsInsightsRetrieveArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsPartnerLinksCreateArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsPartnerLinksDeleteArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListDirectLicensesCreateArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListDirectLicensesPatchArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListGlobalLicensesCreateArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListGlobalLicensesPatchArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListsCreateArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListsDeleteArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAccountTypesAccountsUserListsPatchArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAudienceMembersIngestArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerAudienceMembersRemoveArgs;
use crate::providers::gcp::clients::datamanager::DatamanagerEventsIngestArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatamanagerProvider with automatic state tracking.
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
/// let provider = DatamanagerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DatamanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DatamanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DatamanagerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Datamanager account types accounts insights retrieve.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Datamanager account types accounts user list direct licenses patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Datamanager account types accounts user list global licenses patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datamanager account types accounts user lists patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
