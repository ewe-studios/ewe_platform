//! FirebaseapphostingProvider - State-aware firebaseapphosting API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebaseapphosting API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebaseapphosting::{
    firebaseapphosting_projects_locations_get_builder, firebaseapphosting_projects_locations_get_task,
    firebaseapphosting_projects_locations_list_builder, firebaseapphosting_projects_locations_list_task,
    firebaseapphosting_projects_locations_backends_create_builder, firebaseapphosting_projects_locations_backends_create_task,
    firebaseapphosting_projects_locations_backends_delete_builder, firebaseapphosting_projects_locations_backends_delete_task,
    firebaseapphosting_projects_locations_backends_get_builder, firebaseapphosting_projects_locations_backends_get_task,
    firebaseapphosting_projects_locations_backends_list_builder, firebaseapphosting_projects_locations_backends_list_task,
    firebaseapphosting_projects_locations_backends_patch_builder, firebaseapphosting_projects_locations_backends_patch_task,
    firebaseapphosting_projects_locations_backends_builds_create_builder, firebaseapphosting_projects_locations_backends_builds_create_task,
    firebaseapphosting_projects_locations_backends_builds_delete_builder, firebaseapphosting_projects_locations_backends_builds_delete_task,
    firebaseapphosting_projects_locations_backends_builds_get_builder, firebaseapphosting_projects_locations_backends_builds_get_task,
    firebaseapphosting_projects_locations_backends_builds_list_builder, firebaseapphosting_projects_locations_backends_builds_list_task,
    firebaseapphosting_projects_locations_backends_domains_create_builder, firebaseapphosting_projects_locations_backends_domains_create_task,
    firebaseapphosting_projects_locations_backends_domains_delete_builder, firebaseapphosting_projects_locations_backends_domains_delete_task,
    firebaseapphosting_projects_locations_backends_domains_get_builder, firebaseapphosting_projects_locations_backends_domains_get_task,
    firebaseapphosting_projects_locations_backends_domains_list_builder, firebaseapphosting_projects_locations_backends_domains_list_task,
    firebaseapphosting_projects_locations_backends_domains_patch_builder, firebaseapphosting_projects_locations_backends_domains_patch_task,
    firebaseapphosting_projects_locations_backends_rollouts_create_builder, firebaseapphosting_projects_locations_backends_rollouts_create_task,
    firebaseapphosting_projects_locations_backends_rollouts_get_builder, firebaseapphosting_projects_locations_backends_rollouts_get_task,
    firebaseapphosting_projects_locations_backends_rollouts_list_builder, firebaseapphosting_projects_locations_backends_rollouts_list_task,
    firebaseapphosting_projects_locations_backends_traffic_get_builder, firebaseapphosting_projects_locations_backends_traffic_get_task,
    firebaseapphosting_projects_locations_backends_traffic_patch_builder, firebaseapphosting_projects_locations_backends_traffic_patch_task,
    firebaseapphosting_projects_locations_operations_cancel_builder, firebaseapphosting_projects_locations_operations_cancel_task,
    firebaseapphosting_projects_locations_operations_delete_builder, firebaseapphosting_projects_locations_operations_delete_task,
    firebaseapphosting_projects_locations_operations_get_builder, firebaseapphosting_projects_locations_operations_get_task,
    firebaseapphosting_projects_locations_operations_list_builder, firebaseapphosting_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebaseapphosting::Backend;
use crate::providers::gcp::clients::firebaseapphosting::Build;
use crate::providers::gcp::clients::firebaseapphosting::Domain;
use crate::providers::gcp::clients::firebaseapphosting::Empty;
use crate::providers::gcp::clients::firebaseapphosting::ListBackendsResponse;
use crate::providers::gcp::clients::firebaseapphosting::ListBuildsResponse;
use crate::providers::gcp::clients::firebaseapphosting::ListDomainsResponse;
use crate::providers::gcp::clients::firebaseapphosting::ListLocationsResponse;
use crate::providers::gcp::clients::firebaseapphosting::ListOperationsResponse;
use crate::providers::gcp::clients::firebaseapphosting::ListRolloutsResponse;
use crate::providers::gcp::clients::firebaseapphosting::Location;
use crate::providers::gcp::clients::firebaseapphosting::Operation;
use crate::providers::gcp::clients::firebaseapphosting::Rollout;
use crate::providers::gcp::clients::firebaseapphosting::Traffic;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsBuildsCreateArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsBuildsDeleteArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsBuildsGetArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsBuildsListArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsCreateArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsDeleteArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsDomainsCreateArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsDomainsDeleteArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsDomainsGetArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsDomainsListArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsDomainsPatchArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsGetArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsListArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsPatchArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsRolloutsCreateArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsRolloutsGetArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsRolloutsListArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsTrafficGetArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsBackendsTrafficPatchArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsGetArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsListArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::firebaseapphosting::FirebaseapphostingProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebaseapphostingProvider with automatic state tracking.
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
/// let provider = FirebaseapphostingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebaseapphostingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebaseapphostingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebaseapphostingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebaseapphosting projects locations get.
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
    pub fn firebaseapphosting_projects_locations_get(
        &self,
        args: &FirebaseapphostingProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations list.
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
    pub fn firebaseapphosting_projects_locations_list(
        &self,
        args: &FirebaseapphostingProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends create.
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
    pub fn firebaseapphosting_projects_locations_backends_create(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_create_builder(
            &self.http_client,
            &args.parent,
            &args.backendId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends delete.
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
    pub fn firebaseapphosting_projects_locations_backends_delete(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Backend result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_get(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backend, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackendsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_list(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackendsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends patch.
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
    pub fn firebaseapphosting_projects_locations_backends_patch(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends builds create.
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
    pub fn firebaseapphosting_projects_locations_backends_builds_create(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsBuildsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_builds_create_builder(
            &self.http_client,
            &args.parent,
            &args.buildId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_builds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends builds delete.
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
    pub fn firebaseapphosting_projects_locations_backends_builds_delete(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsBuildsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_builds_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_builds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends builds get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Build result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_builds_get(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsBuildsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Build, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_builds_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_builds_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends builds list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBuildsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_builds_list(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsBuildsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBuildsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_builds_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_builds_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends domains create.
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
    pub fn firebaseapphosting_projects_locations_backends_domains_create(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsDomainsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_domains_create_builder(
            &self.http_client,
            &args.parent,
            &args.domainId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_domains_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends domains delete.
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
    pub fn firebaseapphosting_projects_locations_backends_domains_delete(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsDomainsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_domains_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_domains_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends domains get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Domain result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_domains_get(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsDomainsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Domain, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_domains_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_domains_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends domains list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDomainsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_domains_list(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsDomainsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDomainsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_domains_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_domains_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends domains patch.
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
    pub fn firebaseapphosting_projects_locations_backends_domains_patch(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsDomainsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_domains_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_domains_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends rollouts create.
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
    pub fn firebaseapphosting_projects_locations_backends_rollouts_create(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsRolloutsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_rollouts_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.rolloutId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_rollouts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends rollouts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Rollout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_rollouts_get(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsRolloutsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rollout, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_rollouts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_rollouts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends rollouts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRolloutsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_rollouts_list(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsRolloutsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRolloutsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_rollouts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_rollouts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends traffic get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Traffic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaseapphosting_projects_locations_backends_traffic_get(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsTrafficGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Traffic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_traffic_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_traffic_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations backends traffic patch.
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
    pub fn firebaseapphosting_projects_locations_backends_traffic_patch(
        &self,
        args: &FirebaseapphostingProjectsLocationsBackendsTrafficPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_backends_traffic_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_backends_traffic_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations operations cancel.
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
    pub fn firebaseapphosting_projects_locations_operations_cancel(
        &self,
        args: &FirebaseapphostingProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations operations delete.
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
    pub fn firebaseapphosting_projects_locations_operations_delete(
        &self,
        args: &FirebaseapphostingProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations operations get.
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
    pub fn firebaseapphosting_projects_locations_operations_get(
        &self,
        args: &FirebaseapphostingProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaseapphosting projects locations operations list.
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
    pub fn firebaseapphosting_projects_locations_operations_list(
        &self,
        args: &FirebaseapphostingProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaseapphosting_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaseapphosting_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
