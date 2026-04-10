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
    people_contact_groups_create_builder, people_contact_groups_create_task,
    people_contact_groups_delete_builder, people_contact_groups_delete_task,
    people_contact_groups_update_builder, people_contact_groups_update_task,
    people_contact_groups_members_modify_builder, people_contact_groups_members_modify_task,
    people_other_contacts_copy_other_contact_to_my_contacts_group_builder, people_other_contacts_copy_other_contact_to_my_contacts_group_task,
    people_people_batch_create_contacts_builder, people_people_batch_create_contacts_task,
    people_people_batch_delete_contacts_builder, people_people_batch_delete_contacts_task,
    people_people_batch_update_contacts_builder, people_people_batch_update_contacts_task,
    people_people_create_contact_builder, people_people_create_contact_task,
    people_people_delete_contact_builder, people_people_delete_contact_task,
    people_people_delete_contact_photo_builder, people_people_delete_contact_photo_task,
    people_people_update_contact_builder, people_people_update_contact_task,
    people_people_update_contact_photo_builder, people_people_update_contact_photo_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::people::BatchCreateContactsResponse;
use crate::providers::gcp::clients::people::BatchUpdateContactsResponse;
use crate::providers::gcp::clients::people::ContactGroup;
use crate::providers::gcp::clients::people::DeleteContactPhotoResponse;
use crate::providers::gcp::clients::people::Empty;
use crate::providers::gcp::clients::people::ModifyContactGroupMembersResponse;
use crate::providers::gcp::clients::people::Person;
use crate::providers::gcp::clients::people::UpdateContactPhotoResponse;
use crate::providers::gcp::clients::people::PeopleContactGroupsCreateArgs;
use crate::providers::gcp::clients::people::PeopleContactGroupsDeleteArgs;
use crate::providers::gcp::clients::people::PeopleContactGroupsMembersModifyArgs;
use crate::providers::gcp::clients::people::PeopleContactGroupsUpdateArgs;
use crate::providers::gcp::clients::people::PeopleOtherContactsCopyOtherContactToMyContactsGroupArgs;
use crate::providers::gcp::clients::people::PeoplePeopleBatchCreateContactsArgs;
use crate::providers::gcp::clients::people::PeoplePeopleBatchDeleteContactsArgs;
use crate::providers::gcp::clients::people::PeoplePeopleBatchUpdateContactsArgs;
use crate::providers::gcp::clients::people::PeoplePeopleCreateContactArgs;
use crate::providers::gcp::clients::people::PeoplePeopleDeleteContactArgs;
use crate::providers::gcp::clients::people::PeoplePeopleDeleteContactPhotoArgs;
use crate::providers::gcp::clients::people::PeoplePeopleUpdateContactArgs;
use crate::providers::gcp::clients::people::PeoplePeopleUpdateContactPhotoArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PeopleProvider with automatic state tracking.
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
/// let provider = PeopleProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PeopleProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PeopleProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PeopleProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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

}
