//! SaasservicemgmtProvider - State-aware saasservicemgmt API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       saasservicemgmt API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::saasservicemgmt::{
    saasservicemgmt_projects_locations_releases_create_builder, saasservicemgmt_projects_locations_releases_create_task,
    saasservicemgmt_projects_locations_releases_delete_builder, saasservicemgmt_projects_locations_releases_delete_task,
    saasservicemgmt_projects_locations_releases_patch_builder, saasservicemgmt_projects_locations_releases_patch_task,
    saasservicemgmt_projects_locations_rollout_kinds_create_builder, saasservicemgmt_projects_locations_rollout_kinds_create_task,
    saasservicemgmt_projects_locations_rollout_kinds_delete_builder, saasservicemgmt_projects_locations_rollout_kinds_delete_task,
    saasservicemgmt_projects_locations_rollout_kinds_patch_builder, saasservicemgmt_projects_locations_rollout_kinds_patch_task,
    saasservicemgmt_projects_locations_rollouts_create_builder, saasservicemgmt_projects_locations_rollouts_create_task,
    saasservicemgmt_projects_locations_rollouts_delete_builder, saasservicemgmt_projects_locations_rollouts_delete_task,
    saasservicemgmt_projects_locations_rollouts_patch_builder, saasservicemgmt_projects_locations_rollouts_patch_task,
    saasservicemgmt_projects_locations_saas_create_builder, saasservicemgmt_projects_locations_saas_create_task,
    saasservicemgmt_projects_locations_saas_delete_builder, saasservicemgmt_projects_locations_saas_delete_task,
    saasservicemgmt_projects_locations_saas_patch_builder, saasservicemgmt_projects_locations_saas_patch_task,
    saasservicemgmt_projects_locations_tenants_create_builder, saasservicemgmt_projects_locations_tenants_create_task,
    saasservicemgmt_projects_locations_tenants_delete_builder, saasservicemgmt_projects_locations_tenants_delete_task,
    saasservicemgmt_projects_locations_tenants_patch_builder, saasservicemgmt_projects_locations_tenants_patch_task,
    saasservicemgmt_projects_locations_unit_kinds_create_builder, saasservicemgmt_projects_locations_unit_kinds_create_task,
    saasservicemgmt_projects_locations_unit_kinds_delete_builder, saasservicemgmt_projects_locations_unit_kinds_delete_task,
    saasservicemgmt_projects_locations_unit_kinds_patch_builder, saasservicemgmt_projects_locations_unit_kinds_patch_task,
    saasservicemgmt_projects_locations_unit_operations_create_builder, saasservicemgmt_projects_locations_unit_operations_create_task,
    saasservicemgmt_projects_locations_unit_operations_delete_builder, saasservicemgmt_projects_locations_unit_operations_delete_task,
    saasservicemgmt_projects_locations_unit_operations_patch_builder, saasservicemgmt_projects_locations_unit_operations_patch_task,
    saasservicemgmt_projects_locations_units_create_builder, saasservicemgmt_projects_locations_units_create_task,
    saasservicemgmt_projects_locations_units_delete_builder, saasservicemgmt_projects_locations_units_delete_task,
    saasservicemgmt_projects_locations_units_patch_builder, saasservicemgmt_projects_locations_units_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::saasservicemgmt::Empty;
use crate::providers::gcp::clients::saasservicemgmt::Release;
use crate::providers::gcp::clients::saasservicemgmt::Rollout;
use crate::providers::gcp::clients::saasservicemgmt::RolloutKind;
use crate::providers::gcp::clients::saasservicemgmt::Saas;
use crate::providers::gcp::clients::saasservicemgmt::Tenant;
use crate::providers::gcp::clients::saasservicemgmt::Unit;
use crate::providers::gcp::clients::saasservicemgmt::UnitKind;
use crate::providers::gcp::clients::saasservicemgmt::UnitOperation;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsReleasesCreateArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsReleasesDeleteArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsReleasesPatchArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsRolloutKindsCreateArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsRolloutKindsDeleteArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsRolloutKindsPatchArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsRolloutsCreateArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsRolloutsDeleteArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsRolloutsPatchArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsSaasCreateArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsSaasDeleteArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsSaasPatchArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsTenantsCreateArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsTenantsDeleteArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsTenantsPatchArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitKindsCreateArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitKindsDeleteArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitKindsPatchArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitOperationsCreateArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitOperationsDeleteArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitOperationsPatchArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitsCreateArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitsDeleteArgs;
use crate::providers::gcp::clients::saasservicemgmt::SaasservicemgmtProjectsLocationsUnitsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SaasservicemgmtProvider with automatic state tracking.
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
/// let provider = SaasservicemgmtProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SaasservicemgmtProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SaasservicemgmtProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SaasservicemgmtProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Saasservicemgmt projects locations releases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Release result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_releases_create(
        &self,
        args: &SaasservicemgmtProjectsLocationsReleasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Release, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_releases_create_builder(
            &self.http_client,
            &args.parent,
            &args.releaseId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_releases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations releases delete.
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
    pub fn saasservicemgmt_projects_locations_releases_delete(
        &self,
        args: &SaasservicemgmtProjectsLocationsReleasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_releases_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_releases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations releases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Release result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_releases_patch(
        &self,
        args: &SaasservicemgmtProjectsLocationsReleasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Release, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_releases_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_releases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations rollout kinds create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RolloutKind result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_rollout_kinds_create(
        &self,
        args: &SaasservicemgmtProjectsLocationsRolloutKindsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RolloutKind, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_rollout_kinds_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.rolloutKindId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_rollout_kinds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations rollout kinds delete.
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
    pub fn saasservicemgmt_projects_locations_rollout_kinds_delete(
        &self,
        args: &SaasservicemgmtProjectsLocationsRolloutKindsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_rollout_kinds_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_rollout_kinds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations rollout kinds patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RolloutKind result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_rollout_kinds_patch(
        &self,
        args: &SaasservicemgmtProjectsLocationsRolloutKindsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RolloutKind, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_rollout_kinds_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_rollout_kinds_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations rollouts create.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_rollouts_create(
        &self,
        args: &SaasservicemgmtProjectsLocationsRolloutsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rollout, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_rollouts_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.rolloutId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_rollouts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations rollouts delete.
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
    pub fn saasservicemgmt_projects_locations_rollouts_delete(
        &self,
        args: &SaasservicemgmtProjectsLocationsRolloutsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_rollouts_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_rollouts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations rollouts patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_rollouts_patch(
        &self,
        args: &SaasservicemgmtProjectsLocationsRolloutsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rollout, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_rollouts_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_rollouts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations saas create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Saas result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_saas_create(
        &self,
        args: &SaasservicemgmtProjectsLocationsSaasCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Saas, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_saas_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.saasId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_saas_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations saas delete.
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
    pub fn saasservicemgmt_projects_locations_saas_delete(
        &self,
        args: &SaasservicemgmtProjectsLocationsSaasDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_saas_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_saas_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations saas patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Saas result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_saas_patch(
        &self,
        args: &SaasservicemgmtProjectsLocationsSaasPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Saas, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_saas_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_saas_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations tenants create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tenant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_tenants_create(
        &self,
        args: &SaasservicemgmtProjectsLocationsTenantsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tenant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_tenants_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.tenantId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_tenants_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations tenants delete.
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
    pub fn saasservicemgmt_projects_locations_tenants_delete(
        &self,
        args: &SaasservicemgmtProjectsLocationsTenantsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_tenants_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_tenants_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations tenants patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tenant result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_tenants_patch(
        &self,
        args: &SaasservicemgmtProjectsLocationsTenantsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tenant, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_tenants_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_tenants_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations unit kinds create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UnitKind result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_unit_kinds_create(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitKindsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UnitKind, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_unit_kinds_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.unitKindId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_unit_kinds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations unit kinds delete.
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
    pub fn saasservicemgmt_projects_locations_unit_kinds_delete(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitKindsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_unit_kinds_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_unit_kinds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations unit kinds patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UnitKind result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_unit_kinds_patch(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitKindsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UnitKind, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_unit_kinds_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_unit_kinds_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations unit operations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UnitOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_unit_operations_create(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitOperationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UnitOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_unit_operations_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.unitOperationId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_unit_operations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations unit operations delete.
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
    pub fn saasservicemgmt_projects_locations_unit_operations_delete(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_unit_operations_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_unit_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations unit operations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UnitOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_unit_operations_patch(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitOperationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UnitOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_unit_operations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_unit_operations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations units create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Unit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_units_create(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Unit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_units_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.unitId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_units_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations units delete.
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
    pub fn saasservicemgmt_projects_locations_units_delete(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_units_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_units_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Saasservicemgmt projects locations units patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Unit result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn saasservicemgmt_projects_locations_units_patch(
        &self,
        args: &SaasservicemgmtProjectsLocationsUnitsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Unit, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = saasservicemgmt_projects_locations_units_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = saasservicemgmt_projects_locations_units_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
