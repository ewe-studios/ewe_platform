//! ApikeysProvider - State-aware apikeys API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       apikeys API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::apikeys::{
    apikeys_keys_lookup_key_builder, apikeys_keys_lookup_key_task,
    apikeys_operations_get_builder, apikeys_operations_get_task,
    apikeys_projects_locations_keys_create_builder, apikeys_projects_locations_keys_create_task,
    apikeys_projects_locations_keys_delete_builder, apikeys_projects_locations_keys_delete_task,
    apikeys_projects_locations_keys_get_builder, apikeys_projects_locations_keys_get_task,
    apikeys_projects_locations_keys_get_key_string_builder, apikeys_projects_locations_keys_get_key_string_task,
    apikeys_projects_locations_keys_list_builder, apikeys_projects_locations_keys_list_task,
    apikeys_projects_locations_keys_patch_builder, apikeys_projects_locations_keys_patch_task,
    apikeys_projects_locations_keys_undelete_builder, apikeys_projects_locations_keys_undelete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::apikeys::Operation;
use crate::providers::gcp::clients::apikeys::V2GetKeyStringResponse;
use crate::providers::gcp::clients::apikeys::V2Key;
use crate::providers::gcp::clients::apikeys::V2ListKeysResponse;
use crate::providers::gcp::clients::apikeys::V2LookupKeyResponse;
use crate::providers::gcp::clients::apikeys::ApikeysKeysLookupKeyArgs;
use crate::providers::gcp::clients::apikeys::ApikeysOperationsGetArgs;
use crate::providers::gcp::clients::apikeys::ApikeysProjectsLocationsKeysCreateArgs;
use crate::providers::gcp::clients::apikeys::ApikeysProjectsLocationsKeysDeleteArgs;
use crate::providers::gcp::clients::apikeys::ApikeysProjectsLocationsKeysGetArgs;
use crate::providers::gcp::clients::apikeys::ApikeysProjectsLocationsKeysGetKeyStringArgs;
use crate::providers::gcp::clients::apikeys::ApikeysProjectsLocationsKeysListArgs;
use crate::providers::gcp::clients::apikeys::ApikeysProjectsLocationsKeysPatchArgs;
use crate::providers::gcp::clients::apikeys::ApikeysProjectsLocationsKeysUndeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ApikeysProvider with automatic state tracking.
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
/// let provider = ApikeysProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ApikeysProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ApikeysProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ApikeysProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Apikeys keys lookup key.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V2LookupKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apikeys_keys_lookup_key(
        &self,
        args: &ApikeysKeysLookupKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V2LookupKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_keys_lookup_key_builder(
            &self.http_client,
            &args.keyString,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_keys_lookup_key_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apikeys operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn apikeys_operations_get(
        &self,
        args: &ApikeysOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apikeys projects locations keys create.
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
    pub fn apikeys_projects_locations_keys_create(
        &self,
        args: &ApikeysProjectsLocationsKeysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_projects_locations_keys_create_builder(
            &self.http_client,
            &args.parent,
            &args.keyId,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_projects_locations_keys_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apikeys projects locations keys delete.
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
    pub fn apikeys_projects_locations_keys_delete(
        &self,
        args: &ApikeysProjectsLocationsKeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_projects_locations_keys_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_projects_locations_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apikeys projects locations keys get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V2Key result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apikeys_projects_locations_keys_get(
        &self,
        args: &ApikeysProjectsLocationsKeysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V2Key, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_projects_locations_keys_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_projects_locations_keys_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apikeys projects locations keys get key string.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V2GetKeyStringResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apikeys_projects_locations_keys_get_key_string(
        &self,
        args: &ApikeysProjectsLocationsKeysGetKeyStringArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V2GetKeyStringResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_projects_locations_keys_get_key_string_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_projects_locations_keys_get_key_string_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apikeys projects locations keys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the V2ListKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apikeys_projects_locations_keys_list(
        &self,
        args: &ApikeysProjectsLocationsKeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<V2ListKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_projects_locations_keys_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_projects_locations_keys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apikeys projects locations keys patch.
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
    pub fn apikeys_projects_locations_keys_patch(
        &self,
        args: &ApikeysProjectsLocationsKeysPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_projects_locations_keys_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_projects_locations_keys_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apikeys projects locations keys undelete.
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
    pub fn apikeys_projects_locations_keys_undelete(
        &self,
        args: &ApikeysProjectsLocationsKeysUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apikeys_projects_locations_keys_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apikeys_projects_locations_keys_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
