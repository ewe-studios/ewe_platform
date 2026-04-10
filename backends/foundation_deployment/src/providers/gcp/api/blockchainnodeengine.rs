//! BlockchainnodeengineProvider - State-aware blockchainnodeengine API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       blockchainnodeengine API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::blockchainnodeengine::{
    blockchainnodeengine_projects_locations_get_builder, blockchainnodeengine_projects_locations_get_task,
    blockchainnodeengine_projects_locations_list_builder, blockchainnodeengine_projects_locations_list_task,
    blockchainnodeengine_projects_locations_blockchain_nodes_create_builder, blockchainnodeengine_projects_locations_blockchain_nodes_create_task,
    blockchainnodeengine_projects_locations_blockchain_nodes_delete_builder, blockchainnodeengine_projects_locations_blockchain_nodes_delete_task,
    blockchainnodeengine_projects_locations_blockchain_nodes_get_builder, blockchainnodeengine_projects_locations_blockchain_nodes_get_task,
    blockchainnodeengine_projects_locations_blockchain_nodes_list_builder, blockchainnodeengine_projects_locations_blockchain_nodes_list_task,
    blockchainnodeengine_projects_locations_blockchain_nodes_patch_builder, blockchainnodeengine_projects_locations_blockchain_nodes_patch_task,
    blockchainnodeengine_projects_locations_operations_cancel_builder, blockchainnodeengine_projects_locations_operations_cancel_task,
    blockchainnodeengine_projects_locations_operations_delete_builder, blockchainnodeengine_projects_locations_operations_delete_task,
    blockchainnodeengine_projects_locations_operations_get_builder, blockchainnodeengine_projects_locations_operations_get_task,
    blockchainnodeengine_projects_locations_operations_list_builder, blockchainnodeengine_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainNode;
use crate::providers::gcp::clients::blockchainnodeengine::GoogleProtobufEmpty;
use crate::providers::gcp::clients::blockchainnodeengine::ListBlockchainNodesResponse;
use crate::providers::gcp::clients::blockchainnodeengine::ListLocationsResponse;
use crate::providers::gcp::clients::blockchainnodeengine::ListOperationsResponse;
use crate::providers::gcp::clients::blockchainnodeengine::Location;
use crate::providers::gcp::clients::blockchainnodeengine::Operation;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsBlockchainNodesCreateArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsBlockchainNodesDeleteArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsBlockchainNodesGetArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsBlockchainNodesListArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsBlockchainNodesPatchArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsGetArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsListArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::blockchainnodeengine::BlockchainnodeengineProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BlockchainnodeengineProvider with automatic state tracking.
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
/// let provider = BlockchainnodeengineProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BlockchainnodeengineProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BlockchainnodeengineProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BlockchainnodeengineProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Blockchainnodeengine projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blockchainnodeengine_projects_locations_get(
        &self,
        args: &BlockchainnodeengineProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blockchainnodeengine_projects_locations_list(
        &self,
        args: &BlockchainnodeengineProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations blockchain nodes create.
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
    pub fn blockchainnodeengine_projects_locations_blockchain_nodes_create(
        &self,
        args: &BlockchainnodeengineProjectsLocationsBlockchainNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_blockchain_nodes_create_builder(
            &self.http_client,
            &args.parent,
            &args.blockchainNodeId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_blockchain_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations blockchain nodes delete.
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
    pub fn blockchainnodeengine_projects_locations_blockchain_nodes_delete(
        &self,
        args: &BlockchainnodeengineProjectsLocationsBlockchainNodesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_blockchain_nodes_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_blockchain_nodes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations blockchain nodes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BlockchainNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blockchainnodeengine_projects_locations_blockchain_nodes_get(
        &self,
        args: &BlockchainnodeengineProjectsLocationsBlockchainNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BlockchainNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_blockchain_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_blockchain_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations blockchain nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBlockchainNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blockchainnodeengine_projects_locations_blockchain_nodes_list(
        &self,
        args: &BlockchainnodeengineProjectsLocationsBlockchainNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBlockchainNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_blockchain_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_blockchain_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations blockchain nodes patch.
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
    pub fn blockchainnodeengine_projects_locations_blockchain_nodes_patch(
        &self,
        args: &BlockchainnodeengineProjectsLocationsBlockchainNodesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_blockchain_nodes_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_blockchain_nodes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations operations cancel.
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
    pub fn blockchainnodeengine_projects_locations_operations_cancel(
        &self,
        args: &BlockchainnodeengineProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations operations delete.
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
    pub fn blockchainnodeengine_projects_locations_operations_delete(
        &self,
        args: &BlockchainnodeengineProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations operations get.
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
    pub fn blockchainnodeengine_projects_locations_operations_get(
        &self,
        args: &BlockchainnodeengineProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Blockchainnodeengine projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn blockchainnodeengine_projects_locations_operations_list(
        &self,
        args: &BlockchainnodeengineProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = blockchainnodeengine_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = blockchainnodeengine_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
