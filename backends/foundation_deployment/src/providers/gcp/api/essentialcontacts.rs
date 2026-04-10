//! EssentialcontactsProvider - State-aware essentialcontacts API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       essentialcontacts API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::essentialcontacts::{
    essentialcontacts_folders_contacts_create_builder, essentialcontacts_folders_contacts_create_task,
    essentialcontacts_folders_contacts_delete_builder, essentialcontacts_folders_contacts_delete_task,
    essentialcontacts_folders_contacts_patch_builder, essentialcontacts_folders_contacts_patch_task,
    essentialcontacts_folders_contacts_send_test_message_builder, essentialcontacts_folders_contacts_send_test_message_task,
    essentialcontacts_organizations_contacts_create_builder, essentialcontacts_organizations_contacts_create_task,
    essentialcontacts_organizations_contacts_delete_builder, essentialcontacts_organizations_contacts_delete_task,
    essentialcontacts_organizations_contacts_patch_builder, essentialcontacts_organizations_contacts_patch_task,
    essentialcontacts_organizations_contacts_send_test_message_builder, essentialcontacts_organizations_contacts_send_test_message_task,
    essentialcontacts_projects_contacts_create_builder, essentialcontacts_projects_contacts_create_task,
    essentialcontacts_projects_contacts_delete_builder, essentialcontacts_projects_contacts_delete_task,
    essentialcontacts_projects_contacts_patch_builder, essentialcontacts_projects_contacts_patch_task,
    essentialcontacts_projects_contacts_send_test_message_builder, essentialcontacts_projects_contacts_send_test_message_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::essentialcontacts::GoogleCloudEssentialcontactsV1Contact;
use crate::providers::gcp::clients::essentialcontacts::GoogleProtobufEmpty;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsFoldersContactsCreateArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsFoldersContactsDeleteArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsFoldersContactsPatchArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsFoldersContactsSendTestMessageArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsOrganizationsContactsCreateArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsOrganizationsContactsDeleteArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsOrganizationsContactsPatchArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsOrganizationsContactsSendTestMessageArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsProjectsContactsCreateArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsProjectsContactsDeleteArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsProjectsContactsPatchArgs;
use crate::providers::gcp::clients::essentialcontacts::EssentialcontactsProjectsContactsSendTestMessageArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// EssentialcontactsProvider with automatic state tracking.
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
/// let provider = EssentialcontactsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct EssentialcontactsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> EssentialcontactsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new EssentialcontactsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Essentialcontacts folders contacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudEssentialcontactsV1Contact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn essentialcontacts_folders_contacts_create(
        &self,
        args: &EssentialcontactsFoldersContactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudEssentialcontactsV1Contact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_folders_contacts_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_folders_contacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts folders contacts delete.
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
    pub fn essentialcontacts_folders_contacts_delete(
        &self,
        args: &EssentialcontactsFoldersContactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_folders_contacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_folders_contacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts folders contacts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudEssentialcontactsV1Contact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn essentialcontacts_folders_contacts_patch(
        &self,
        args: &EssentialcontactsFoldersContactsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudEssentialcontactsV1Contact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_folders_contacts_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_folders_contacts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts folders contacts send test message.
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
    pub fn essentialcontacts_folders_contacts_send_test_message(
        &self,
        args: &EssentialcontactsFoldersContactsSendTestMessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_folders_contacts_send_test_message_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_folders_contacts_send_test_message_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts organizations contacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudEssentialcontactsV1Contact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn essentialcontacts_organizations_contacts_create(
        &self,
        args: &EssentialcontactsOrganizationsContactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudEssentialcontactsV1Contact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_organizations_contacts_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_organizations_contacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts organizations contacts delete.
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
    pub fn essentialcontacts_organizations_contacts_delete(
        &self,
        args: &EssentialcontactsOrganizationsContactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_organizations_contacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_organizations_contacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts organizations contacts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudEssentialcontactsV1Contact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn essentialcontacts_organizations_contacts_patch(
        &self,
        args: &EssentialcontactsOrganizationsContactsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudEssentialcontactsV1Contact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_organizations_contacts_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_organizations_contacts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts organizations contacts send test message.
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
    pub fn essentialcontacts_organizations_contacts_send_test_message(
        &self,
        args: &EssentialcontactsOrganizationsContactsSendTestMessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_organizations_contacts_send_test_message_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_organizations_contacts_send_test_message_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts projects contacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudEssentialcontactsV1Contact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn essentialcontacts_projects_contacts_create(
        &self,
        args: &EssentialcontactsProjectsContactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudEssentialcontactsV1Contact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_projects_contacts_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_projects_contacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts projects contacts delete.
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
    pub fn essentialcontacts_projects_contacts_delete(
        &self,
        args: &EssentialcontactsProjectsContactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_projects_contacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_projects_contacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts projects contacts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudEssentialcontactsV1Contact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn essentialcontacts_projects_contacts_patch(
        &self,
        args: &EssentialcontactsProjectsContactsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudEssentialcontactsV1Contact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_projects_contacts_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_projects_contacts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Essentialcontacts projects contacts send test message.
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
    pub fn essentialcontacts_projects_contacts_send_test_message(
        &self,
        args: &EssentialcontactsProjectsContactsSendTestMessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = essentialcontacts_projects_contacts_send_test_message_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = essentialcontacts_projects_contacts_send_test_message_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
