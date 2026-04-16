//! PeopleProvider - State-aware people API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       people API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::people::{
    people_contact_groups_batch_get_builder, people_contact_groups_batch_get_task,
    people_contact_groups_create_builder, people_contact_groups_create_task,
    people_contact_groups_delete_builder, people_contact_groups_delete_task,
    people_contact_groups_get_builder, people_contact_groups_get_task,
    people_contact_groups_list_builder, people_contact_groups_list_task,
    people_contact_groups_update_builder, people_contact_groups_update_task,
    people_contact_groups_members_modify_builder, people_contact_groups_members_modify_task,
    people_other_contacts_copy_other_contact_to_my_contacts_group_builder, people_other_contacts_copy_other_contact_to_my_contacts_group_task,
    people_other_contacts_list_builder, people_other_contacts_list_task,
    people_other_contacts_search_builder, people_other_contacts_search_task,
    people_people_batch_create_contacts_builder, people_people_batch_create_contacts_task,
    people_people_batch_delete_contacts_builder, people_people_batch_delete_contacts_task,
    people_people_batch_update_contacts_builder, people_people_batch_update_contacts_task,
    people_people_create_contact_builder, people_people_create_contact_task,
    people_people_delete_contact_builder, people_people_delete_contact_task,
    people_people_delete_contact_photo_builder, people_people_delete_contact_photo_task,
    people_people_get_builder, people_people_get_task,
    people_people_get_batch_get_builder, people_people_get_batch_get_task,
    people_people_list_directory_people_builder, people_people_list_directory_people_task,
    people_people_search_contacts_builder, people_people_search_contacts_task,
    people_people_search_directory_people_builder, people_people_search_directory_people_task,
    people_people_update_contact_builder, people_people_update_contact_task,
    people_people_update_contact_photo_builder, people_people_update_contact_photo_task,
    people_people_connections_list_builder, people_people_connections_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::people::BatchCreateContactsResponse;
use crate::providers::gcp::clients::people::BatchGetContactGroupsResponse;
use crate::providers::gcp::clients::people::BatchUpdateContactsResponse;
use crate::providers::gcp::clients::people::ContactGroup;
use crate::providers::gcp::clients::people::DeleteContactPhotoResponse;
use crate::providers::gcp::clients::people::Empty;
use crate::providers::gcp::clients::people::GetPeopleResponse;
use crate::providers::gcp::clients::people::ListConnectionsResponse;
use crate::providers::gcp::clients::people::ListContactGroupsResponse;
use crate::providers::gcp::clients::people::ListDirectoryPeopleResponse;
use crate::providers::gcp::clients::people::ListOtherContactsResponse;
use crate::providers::gcp::clients::people::ModifyContactGroupMembersResponse;
use crate::providers::gcp::clients::people::Person;
use crate::providers::gcp::clients::people::SearchDirectoryPeopleResponse;
use crate::providers::gcp::clients::people::SearchResponse;
use crate::providers::gcp::clients::people::UpdateContactPhotoResponse;
use crate::providers::gcp::clients::people::PeopleContactGroupsBatchGetArgs;
use crate::providers::gcp::clients::people::PeopleContactGroupsDeleteArgs;
use crate::providers::gcp::clients::people::PeopleContactGroupsGetArgs;
use crate::providers::gcp::clients::people::PeopleContactGroupsListArgs;
use crate::providers::gcp::clients::people::PeopleContactGroupsMembersModifyArgs;
use crate::providers::gcp::clients::people::PeopleContactGroupsUpdateArgs;
use crate::providers::gcp::clients::people::PeopleOtherContactsCopyOtherContactToMyContactsGroupArgs;
use crate::providers::gcp::clients::people::PeopleOtherContactsListArgs;
use crate::providers::gcp::clients::people::PeopleOtherContactsSearchArgs;
use crate::providers::gcp::clients::people::PeoplePeopleConnectionsListArgs;
use crate::providers::gcp::clients::people::PeoplePeopleCreateContactArgs;
use crate::providers::gcp::clients::people::PeoplePeopleDeleteContactArgs;
use crate::providers::gcp::clients::people::PeoplePeopleDeleteContactPhotoArgs;
use crate::providers::gcp::clients::people::PeoplePeopleGetArgs;
use crate::providers::gcp::clients::people::PeoplePeopleGetBatchGetArgs;
use crate::providers::gcp::clients::people::PeoplePeopleListDirectoryPeopleArgs;
use crate::providers::gcp::clients::people::PeoplePeopleSearchContactsArgs;
use crate::providers::gcp::clients::people::PeoplePeopleSearchDirectoryPeopleArgs;
use crate::providers::gcp::clients::people::PeoplePeopleUpdateContactArgs;
use crate::providers::gcp::clients::people::PeoplePeopleUpdateContactPhotoArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PeopleProvider with automatic state tracking.
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
/// let provider = PeopleProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PeopleProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PeopleProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PeopleProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PeopleProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// People contact groups batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetContactGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_contact_groups_batch_get(
        &self,
        args: &PeopleContactGroupsBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetContactGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_contact_groups_batch_get_builder(
            &self.http_client,
            &args.groupFields,
            &args.maxMembers,
            &args.resourceNames,
        )
        .map_err(ProviderError::Api)?;

        let task = people_contact_groups_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People contact groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContactGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_contact_groups_create(
        &self,
        args: &PeopleContactGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContactGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_contact_groups_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = people_contact_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People contact groups delete.
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
    pub fn people_contact_groups_delete(
        &self,
        args: &PeopleContactGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_contact_groups_delete_builder(
            &self.http_client,
            &args.resourceName,
            &args.deleteContacts,
        )
        .map_err(ProviderError::Api)?;

        let task = people_contact_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People contact groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContactGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_contact_groups_get(
        &self,
        args: &PeopleContactGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContactGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_contact_groups_get_builder(
            &self.http_client,
            &args.resourceName,
            &args.groupFields,
            &args.maxMembers,
        )
        .map_err(ProviderError::Api)?;

        let task = people_contact_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People contact groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListContactGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_contact_groups_list(
        &self,
        args: &PeopleContactGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListContactGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_contact_groups_list_builder(
            &self.http_client,
            &args.groupFields,
            &args.pageSize,
            &args.pageToken,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = people_contact_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People contact groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ContactGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_contact_groups_update(
        &self,
        args: &PeopleContactGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ContactGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_contact_groups_update_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = people_contact_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People contact groups members modify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ModifyContactGroupMembersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_contact_groups_members_modify(
        &self,
        args: &PeopleContactGroupsMembersModifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ModifyContactGroupMembersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_contact_groups_members_modify_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = people_contact_groups_members_modify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People other contacts copy other contact to my contacts group.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Person result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_other_contacts_copy_other_contact_to_my_contacts_group(
        &self,
        args: &PeopleOtherContactsCopyOtherContactToMyContactsGroupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_other_contacts_copy_other_contact_to_my_contacts_group_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = people_other_contacts_copy_other_contact_to_my_contacts_group_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People other contacts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOtherContactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_other_contacts_list(
        &self,
        args: &PeopleOtherContactsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOtherContactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_other_contacts_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.readMask,
            &args.requestSyncToken,
            &args.sources,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = people_other_contacts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People other contacts search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_other_contacts_search(
        &self,
        args: &PeopleOtherContactsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_other_contacts_search_builder(
            &self.http_client,
            &args.pageSize,
            &args.query,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = people_other_contacts_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people batch create contacts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchCreateContactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_people_batch_create_contacts(
        &self,
        args: &PeoplePeopleBatchCreateContactsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchCreateContactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_batch_create_contacts_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_batch_create_contacts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people batch delete contacts.
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
    pub fn people_people_batch_delete_contacts(
        &self,
        args: &PeoplePeopleBatchDeleteContactsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_batch_delete_contacts_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_batch_delete_contacts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people batch update contacts.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateContactsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_people_batch_update_contacts(
        &self,
        args: &PeoplePeopleBatchUpdateContactsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateContactsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_batch_update_contacts_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_batch_update_contacts_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people create contact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Person result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_people_create_contact(
        &self,
        args: &PeoplePeopleCreateContactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_create_contact_builder(
            &self.http_client,
            &args.personFields,
            &args.sources,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_create_contact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people delete contact.
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
    pub fn people_people_delete_contact(
        &self,
        args: &PeoplePeopleDeleteContactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_delete_contact_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_delete_contact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people delete contact photo.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteContactPhotoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_people_delete_contact_photo(
        &self,
        args: &PeoplePeopleDeleteContactPhotoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteContactPhotoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_delete_contact_photo_builder(
            &self.http_client,
            &args.resourceName,
            &args.personFields,
            &args.sources,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_delete_contact_photo_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Person result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_people_get(
        &self,
        args: &PeoplePeopleGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_get_builder(
            &self.http_client,
            &args.resourceName,
            &args.personFields,
            &args.requestMask_includeField,
            &args.sources,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people get batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetPeopleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_people_get_batch_get(
        &self,
        args: &PeoplePeopleGetBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetPeopleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_get_batch_get_builder(
            &self.http_client,
            &args.personFields,
            &args.requestMask_includeField,
            &args.resourceNames,
            &args.sources,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_get_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people list directory people.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDirectoryPeopleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_people_list_directory_people(
        &self,
        args: &PeoplePeopleListDirectoryPeopleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDirectoryPeopleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_list_directory_people_builder(
            &self.http_client,
            &args.mergeSources,
            &args.pageSize,
            &args.pageToken,
            &args.readMask,
            &args.requestSyncToken,
            &args.sources,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_list_directory_people_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people search contacts.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_people_search_contacts(
        &self,
        args: &PeoplePeopleSearchContactsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_search_contacts_builder(
            &self.http_client,
            &args.pageSize,
            &args.query,
            &args.readMask,
            &args.sources,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_search_contacts_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people search directory people.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchDirectoryPeopleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_people_search_directory_people(
        &self,
        args: &PeoplePeopleSearchDirectoryPeopleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchDirectoryPeopleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_search_directory_people_builder(
            &self.http_client,
            &args.mergeSources,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.readMask,
            &args.sources,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_search_directory_people_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people update contact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Person result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_people_update_contact(
        &self,
        args: &PeoplePeopleUpdateContactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Person, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_update_contact_builder(
            &self.http_client,
            &args.resourceName,
            &args.personFields,
            &args.sources,
            &args.updatePersonFields,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_update_contact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people update contact photo.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateContactPhotoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn people_people_update_contact_photo(
        &self,
        args: &PeoplePeopleUpdateContactPhotoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateContactPhotoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_update_contact_photo_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_update_contact_photo_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// People people connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn people_people_connections_list(
        &self,
        args: &PeoplePeopleConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = people_people_connections_list_builder(
            &self.http_client,
            &args.resourceName,
            &args.pageSize,
            &args.pageToken,
            &args.personFields,
            &args.requestMask_includeField,
            &args.requestSyncToken,
            &args.sortOrder,
            &args.sources,
            &args.syncToken,
        )
        .map_err(ProviderError::Api)?;

        let task = people_people_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
