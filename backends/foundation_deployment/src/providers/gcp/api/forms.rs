//! FormsProvider - State-aware forms API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       forms API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::forms::{
    forms_forms_batch_update_builder, forms_forms_batch_update_task,
    forms_forms_create_builder, forms_forms_create_task,
    forms_forms_set_publish_settings_builder, forms_forms_set_publish_settings_task,
    forms_forms_watches_create_builder, forms_forms_watches_create_task,
    forms_forms_watches_delete_builder, forms_forms_watches_delete_task,
    forms_forms_watches_renew_builder, forms_forms_watches_renew_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::forms::BatchUpdateFormResponse;
use crate::providers::gcp::clients::forms::Empty;
use crate::providers::gcp::clients::forms::Form;
use crate::providers::gcp::clients::forms::SetPublishSettingsResponse;
use crate::providers::gcp::clients::forms::Watch;
use crate::providers::gcp::clients::forms::FormsFormsBatchUpdateArgs;
use crate::providers::gcp::clients::forms::FormsFormsCreateArgs;
use crate::providers::gcp::clients::forms::FormsFormsSetPublishSettingsArgs;
use crate::providers::gcp::clients::forms::FormsFormsWatchesCreateArgs;
use crate::providers::gcp::clients::forms::FormsFormsWatchesDeleteArgs;
use crate::providers::gcp::clients::forms::FormsFormsWatchesRenewArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FormsProvider with automatic state tracking.
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
/// let provider = FormsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FormsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FormsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FormsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Forms forms batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateFormResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn forms_forms_batch_update(
        &self,
        args: &FormsFormsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateFormResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = forms_forms_batch_update_builder(
            &self.http_client,
            &args.formId,
        )
        .map_err(ProviderError::Api)?;

        let task = forms_forms_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Forms forms create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Form result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn forms_forms_create(
        &self,
        args: &FormsFormsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Form, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = forms_forms_create_builder(
            &self.http_client,
            &args.unpublished,
        )
        .map_err(ProviderError::Api)?;

        let task = forms_forms_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Forms forms set publish settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetPublishSettingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn forms_forms_set_publish_settings(
        &self,
        args: &FormsFormsSetPublishSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetPublishSettingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = forms_forms_set_publish_settings_builder(
            &self.http_client,
            &args.formId,
        )
        .map_err(ProviderError::Api)?;

        let task = forms_forms_set_publish_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Forms forms watches create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Watch result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn forms_forms_watches_create(
        &self,
        args: &FormsFormsWatchesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Watch, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = forms_forms_watches_create_builder(
            &self.http_client,
            &args.formId,
        )
        .map_err(ProviderError::Api)?;

        let task = forms_forms_watches_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Forms forms watches delete.
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
    pub fn forms_forms_watches_delete(
        &self,
        args: &FormsFormsWatchesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = forms_forms_watches_delete_builder(
            &self.http_client,
            &args.formId,
            &args.watchId,
        )
        .map_err(ProviderError::Api)?;

        let task = forms_forms_watches_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Forms forms watches renew.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Watch result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn forms_forms_watches_renew(
        &self,
        args: &FormsFormsWatchesRenewArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Watch, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = forms_forms_watches_renew_builder(
            &self.http_client,
            &args.formId,
            &args.watchId,
        )
        .map_err(ProviderError::Api)?;

        let task = forms_forms_watches_renew_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
