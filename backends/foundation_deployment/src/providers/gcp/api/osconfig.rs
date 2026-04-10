//! OsconfigProvider - State-aware osconfig API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       osconfig API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::osconfig::{
    osconfig_folders_locations_global_policy_orchestrators_create_builder, osconfig_folders_locations_global_policy_orchestrators_create_task,
    osconfig_folders_locations_global_policy_orchestrators_delete_builder, osconfig_folders_locations_global_policy_orchestrators_delete_task,
    osconfig_folders_locations_global_policy_orchestrators_patch_builder, osconfig_folders_locations_global_policy_orchestrators_patch_task,
    osconfig_folders_locations_operations_cancel_builder, osconfig_folders_locations_operations_cancel_task,
    osconfig_folders_locations_operations_delete_builder, osconfig_folders_locations_operations_delete_task,
    osconfig_organizations_locations_global_policy_orchestrators_create_builder, osconfig_organizations_locations_global_policy_orchestrators_create_task,
    osconfig_organizations_locations_global_policy_orchestrators_delete_builder, osconfig_organizations_locations_global_policy_orchestrators_delete_task,
    osconfig_organizations_locations_global_policy_orchestrators_patch_builder, osconfig_organizations_locations_global_policy_orchestrators_patch_task,
    osconfig_organizations_locations_operations_cancel_builder, osconfig_organizations_locations_operations_cancel_task,
    osconfig_organizations_locations_operations_delete_builder, osconfig_organizations_locations_operations_delete_task,
    osconfig_projects_locations_global_policy_orchestrators_create_builder, osconfig_projects_locations_global_policy_orchestrators_create_task,
    osconfig_projects_locations_global_policy_orchestrators_delete_builder, osconfig_projects_locations_global_policy_orchestrators_delete_task,
    osconfig_projects_locations_global_policy_orchestrators_patch_builder, osconfig_projects_locations_global_policy_orchestrators_patch_task,
    osconfig_projects_locations_operations_cancel_builder, osconfig_projects_locations_operations_cancel_task,
    osconfig_projects_locations_operations_delete_builder, osconfig_projects_locations_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::osconfig::Empty;
use crate::providers::gcp::clients::osconfig::Operation;
use crate::providers::gcp::clients::osconfig::OsconfigFoldersLocationsGlobalPolicyOrchestratorsCreateArgs;
use crate::providers::gcp::clients::osconfig::OsconfigFoldersLocationsGlobalPolicyOrchestratorsDeleteArgs;
use crate::providers::gcp::clients::osconfig::OsconfigFoldersLocationsGlobalPolicyOrchestratorsPatchArgs;
use crate::providers::gcp::clients::osconfig::OsconfigFoldersLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::osconfig::OsconfigFoldersLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::osconfig::OsconfigOrganizationsLocationsGlobalPolicyOrchestratorsCreateArgs;
use crate::providers::gcp::clients::osconfig::OsconfigOrganizationsLocationsGlobalPolicyOrchestratorsDeleteArgs;
use crate::providers::gcp::clients::osconfig::OsconfigOrganizationsLocationsGlobalPolicyOrchestratorsPatchArgs;
use crate::providers::gcp::clients::osconfig::OsconfigOrganizationsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::osconfig::OsconfigOrganizationsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::osconfig::OsconfigProjectsLocationsGlobalPolicyOrchestratorsCreateArgs;
use crate::providers::gcp::clients::osconfig::OsconfigProjectsLocationsGlobalPolicyOrchestratorsDeleteArgs;
use crate::providers::gcp::clients::osconfig::OsconfigProjectsLocationsGlobalPolicyOrchestratorsPatchArgs;
use crate::providers::gcp::clients::osconfig::OsconfigProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::osconfig::OsconfigProjectsLocationsOperationsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// OsconfigProvider with automatic state tracking.
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
/// let provider = OsconfigProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct OsconfigProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> OsconfigProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new OsconfigProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Osconfig folders locations global policy orchestrators create.
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
    pub fn osconfig_folders_locations_global_policy_orchestrators_create(
        &self,
        args: &OsconfigFoldersLocationsGlobalPolicyOrchestratorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_folders_locations_global_policy_orchestrators_create_builder(
            &self.http_client,
            &args.parent,
            &args.policyOrchestratorId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_folders_locations_global_policy_orchestrators_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig folders locations global policy orchestrators delete.
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
    pub fn osconfig_folders_locations_global_policy_orchestrators_delete(
        &self,
        args: &OsconfigFoldersLocationsGlobalPolicyOrchestratorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_folders_locations_global_policy_orchestrators_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_folders_locations_global_policy_orchestrators_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig folders locations global policy orchestrators patch.
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
    pub fn osconfig_folders_locations_global_policy_orchestrators_patch(
        &self,
        args: &OsconfigFoldersLocationsGlobalPolicyOrchestratorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_folders_locations_global_policy_orchestrators_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_folders_locations_global_policy_orchestrators_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig folders locations operations cancel.
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
    pub fn osconfig_folders_locations_operations_cancel(
        &self,
        args: &OsconfigFoldersLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_folders_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_folders_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig folders locations operations delete.
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
    pub fn osconfig_folders_locations_operations_delete(
        &self,
        args: &OsconfigFoldersLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_folders_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_folders_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig organizations locations global policy orchestrators create.
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
    pub fn osconfig_organizations_locations_global_policy_orchestrators_create(
        &self,
        args: &OsconfigOrganizationsLocationsGlobalPolicyOrchestratorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_organizations_locations_global_policy_orchestrators_create_builder(
            &self.http_client,
            &args.parent,
            &args.policyOrchestratorId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_organizations_locations_global_policy_orchestrators_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig organizations locations global policy orchestrators delete.
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
    pub fn osconfig_organizations_locations_global_policy_orchestrators_delete(
        &self,
        args: &OsconfigOrganizationsLocationsGlobalPolicyOrchestratorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_organizations_locations_global_policy_orchestrators_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_organizations_locations_global_policy_orchestrators_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig organizations locations global policy orchestrators patch.
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
    pub fn osconfig_organizations_locations_global_policy_orchestrators_patch(
        &self,
        args: &OsconfigOrganizationsLocationsGlobalPolicyOrchestratorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_organizations_locations_global_policy_orchestrators_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_organizations_locations_global_policy_orchestrators_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig organizations locations operations cancel.
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
    pub fn osconfig_organizations_locations_operations_cancel(
        &self,
        args: &OsconfigOrganizationsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_organizations_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_organizations_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig organizations locations operations delete.
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
    pub fn osconfig_organizations_locations_operations_delete(
        &self,
        args: &OsconfigOrganizationsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_organizations_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_organizations_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig projects locations global policy orchestrators create.
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
    pub fn osconfig_projects_locations_global_policy_orchestrators_create(
        &self,
        args: &OsconfigProjectsLocationsGlobalPolicyOrchestratorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_projects_locations_global_policy_orchestrators_create_builder(
            &self.http_client,
            &args.parent,
            &args.policyOrchestratorId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_projects_locations_global_policy_orchestrators_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig projects locations global policy orchestrators delete.
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
    pub fn osconfig_projects_locations_global_policy_orchestrators_delete(
        &self,
        args: &OsconfigProjectsLocationsGlobalPolicyOrchestratorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_projects_locations_global_policy_orchestrators_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_projects_locations_global_policy_orchestrators_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig projects locations global policy orchestrators patch.
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
    pub fn osconfig_projects_locations_global_policy_orchestrators_patch(
        &self,
        args: &OsconfigProjectsLocationsGlobalPolicyOrchestratorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_projects_locations_global_policy_orchestrators_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_projects_locations_global_policy_orchestrators_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig projects locations operations cancel.
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
    pub fn osconfig_projects_locations_operations_cancel(
        &self,
        args: &OsconfigProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Osconfig projects locations operations delete.
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
    pub fn osconfig_projects_locations_operations_delete(
        &self,
        args: &OsconfigProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = osconfig_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = osconfig_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
