//! PrismaPostgresProvider - State-aware prisma_postgres API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       prisma_postgres API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "prisma_postgres")]

use crate::providers::prisma_postgres::clients::prisma_postgres::{
    post_v1_compute_services_builder, post_v1_compute_services_task,
    delete_v1_compute_services_versions_by_version_id_builder, delete_v1_compute_services_versions_by_version_id_task,
    post_v1_compute_services_versions_by_version_id_start_builder, post_v1_compute_services_versions_by_version_id_start_task,
    post_v1_compute_services_versions_by_version_id_stop_builder, post_v1_compute_services_versions_by_version_id_stop_task,
    patch_v1_compute_services_by_compute_service_id_builder, patch_v1_compute_services_by_compute_service_id_task,
    delete_v1_compute_services_by_compute_service_id_builder, delete_v1_compute_services_by_compute_service_id_task,
    post_v1_compute_services_by_compute_service_id_promote_builder, post_v1_compute_services_by_compute_service_id_promote_task,
    post_v1_compute_services_by_compute_service_id_versions_builder, post_v1_compute_services_by_compute_service_id_versions_task,
    post_v1_connections_builder, post_v1_connections_task,
    delete_v1_connections_by_id_builder, delete_v1_connections_by_id_task,
    post_v1_connections_by_id_rotate_builder, post_v1_connections_by_id_rotate_task,
    post_v1_databases_builder, post_v1_databases_task,
    patch_v1_databases_by_database_id_builder, patch_v1_databases_by_database_id_task,
    delete_v1_databases_by_database_id_builder, delete_v1_databases_by_database_id_task,
    post_v1_databases_by_database_id_connections_builder, post_v1_databases_by_database_id_connections_task,
    post_v1_databases_by_target_database_id_restore_builder, post_v1_databases_by_target_database_id_restore_task,
    delete_v1_integrations_by_id_builder, delete_v1_integrations_by_id_task,
    post_v1_projects_builder, post_v1_projects_task,
    patch_v1_projects_by_id_builder, patch_v1_projects_by_id_task,
    delete_v1_projects_by_id_builder, delete_v1_projects_by_id_task,
    post_v1_projects_by_id_transfer_builder, post_v1_projects_by_id_transfer_task,
    post_v1_projects_by_project_id_compute_services_builder, post_v1_projects_by_project_id_compute_services_task,
    post_v1_projects_by_project_id_databases_builder, post_v1_projects_by_project_id_databases_task,
    post_v1_versions_builder, post_v1_versions_task,
    delete_v1_versions_by_version_id_builder, delete_v1_versions_by_version_id_task,
    post_v1_versions_by_version_id_start_builder, post_v1_versions_by_version_id_start_task,
    post_v1_versions_by_version_id_stop_builder, post_v1_versions_by_version_id_stop_task,
    delete_v1_workspaces_by_workspace_id_integrations_by_client_id_builder, delete_v1_workspaces_by_workspace_id_integrations_by_client_id_task,
};
use crate::providers::prisma_postgres::clients::types::{ApiError, ApiPending};
use crate::providers::prisma_postgres::clients::prisma_postgres::ComputeservicesPatchResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ComputeservicesPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ComputeservicesPromotePostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ComputeservicesVersionsPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ComputeservicesVersionsStartPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ConnectionsPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ConnectionsRotatePostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::DatabasesConnectionsPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::DatabasesPatchResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::DatabasesPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::DatabasesRestorePostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ProjectsComputeservicesPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ProjectsDatabasesPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ProjectsPatchResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::ProjectsPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::VersionsPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::VersionsStartPostResponse;
use crate::providers::prisma_postgres::clients::prisma_postgres::DeleteV1ComputeServicesByComputeServiceIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::DeleteV1ComputeServicesVersionsByVersionIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::DeleteV1ConnectionsByIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::DeleteV1DatabasesByDatabaseIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::DeleteV1IntegrationsByIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::DeleteV1ProjectsByIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::DeleteV1VersionsByVersionIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::DeleteV1WorkspacesByWorkspaceIdIntegrationsByClientIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PatchV1ComputeServicesByComputeServiceIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PatchV1DatabasesByDatabaseIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PatchV1ProjectsByIdArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ComputeServicesArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ComputeServicesByComputeServiceIdPromoteArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ComputeServicesByComputeServiceIdVersionsArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ComputeServicesVersionsByVersionIdStartArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ComputeServicesVersionsByVersionIdStopArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ConnectionsArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ConnectionsByIdRotateArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1DatabasesArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1DatabasesByDatabaseIdConnectionsArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1DatabasesByTargetDatabaseIdRestoreArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ProjectsArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ProjectsByIdTransferArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ProjectsByProjectIdComputeServicesArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1ProjectsByProjectIdDatabasesArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1VersionsArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1VersionsByVersionIdStartArgs;
use crate::providers::prisma_postgres::clients::prisma_postgres::PostV1VersionsByVersionIdStopArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PrismaPostgresProvider with automatic state tracking.
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
/// let provider = PrismaPostgresProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PrismaPostgresProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PrismaPostgresProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PrismaPostgresProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Post v1 compute services.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_compute_services(
        &self,
        args: &PostV1ComputeServicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_compute_services_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_compute_services_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete v1 compute services versions by version id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_v1_compute_services_versions_by_version_id(
        &self,
        args: &DeleteV1ComputeServicesVersionsByVersionIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_v1_compute_services_versions_by_version_id_builder(
            &self.http_client,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_v1_compute_services_versions_by_version_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 compute services versions by version id start.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesVersionsStartPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_compute_services_versions_by_version_id_start(
        &self,
        args: &PostV1ComputeServicesVersionsByVersionIdStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesVersionsStartPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_compute_services_versions_by_version_id_start_builder(
            &self.http_client,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_compute_services_versions_by_version_id_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 compute services versions by version id stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_compute_services_versions_by_version_id_stop(
        &self,
        args: &PostV1ComputeServicesVersionsByVersionIdStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_compute_services_versions_by_version_id_stop_builder(
            &self.http_client,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_compute_services_versions_by_version_id_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Patch v1 compute services by compute service id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesPatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn patch_v1_compute_services_by_compute_service_id(
        &self,
        args: &PatchV1ComputeServicesByComputeServiceIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesPatchResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = patch_v1_compute_services_by_compute_service_id_builder(
            &self.http_client,
            &args.computeServiceId,
        )
        .map_err(ProviderError::Api)?;

        let task = patch_v1_compute_services_by_compute_service_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete v1 compute services by compute service id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_v1_compute_services_by_compute_service_id(
        &self,
        args: &DeleteV1ComputeServicesByComputeServiceIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_v1_compute_services_by_compute_service_id_builder(
            &self.http_client,
            &args.computeServiceId,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_v1_compute_services_by_compute_service_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 compute services by compute service id promote.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesPromotePostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_compute_services_by_compute_service_id_promote(
        &self,
        args: &PostV1ComputeServicesByComputeServiceIdPromoteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesPromotePostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_compute_services_by_compute_service_id_promote_builder(
            &self.http_client,
            &args.computeServiceId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_compute_services_by_compute_service_id_promote_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 compute services by compute service id versions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesVersionsPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_compute_services_by_compute_service_id_versions(
        &self,
        args: &PostV1ComputeServicesByComputeServiceIdVersionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesVersionsPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_compute_services_by_compute_service_id_versions_builder(
            &self.http_client,
            &args.computeServiceId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_compute_services_by_compute_service_id_versions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 connections.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionsPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_connections(
        &self,
        args: &PostV1ConnectionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionsPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_connections_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_connections_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete v1 connections by id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_v1_connections_by_id(
        &self,
        args: &DeleteV1ConnectionsByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_v1_connections_by_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_v1_connections_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 connections by id rotate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionsRotatePostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_connections_by_id_rotate(
        &self,
        args: &PostV1ConnectionsByIdRotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionsRotatePostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_connections_by_id_rotate_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_connections_by_id_rotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 databases.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_databases(
        &self,
        args: &PostV1DatabasesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_databases_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_databases_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Patch v1 databases by database id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesPatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn patch_v1_databases_by_database_id(
        &self,
        args: &PatchV1DatabasesByDatabaseIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesPatchResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = patch_v1_databases_by_database_id_builder(
            &self.http_client,
            &args.databaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = patch_v1_databases_by_database_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete v1 databases by database id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_v1_databases_by_database_id(
        &self,
        args: &DeleteV1DatabasesByDatabaseIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_v1_databases_by_database_id_builder(
            &self.http_client,
            &args.databaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_v1_databases_by_database_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 databases by database id connections.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesConnectionsPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_databases_by_database_id_connections(
        &self,
        args: &PostV1DatabasesByDatabaseIdConnectionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesConnectionsPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_databases_by_database_id_connections_builder(
            &self.http_client,
            &args.databaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_databases_by_database_id_connections_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 databases by target database id restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesRestorePostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_databases_by_target_database_id_restore(
        &self,
        args: &PostV1DatabasesByTargetDatabaseIdRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesRestorePostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_databases_by_target_database_id_restore_builder(
            &self.http_client,
            &args.targetDatabaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_databases_by_target_database_id_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete v1 integrations by id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_v1_integrations_by_id(
        &self,
        args: &DeleteV1IntegrationsByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_v1_integrations_by_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_v1_integrations_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 projects.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectsPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_projects(
        &self,
        args: &PostV1ProjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectsPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_projects_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_projects_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Patch v1 projects by id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectsPatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn patch_v1_projects_by_id(
        &self,
        args: &PatchV1ProjectsByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectsPatchResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = patch_v1_projects_by_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = patch_v1_projects_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete v1 projects by id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_v1_projects_by_id(
        &self,
        args: &DeleteV1ProjectsByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_v1_projects_by_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_v1_projects_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 projects by id transfer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_projects_by_id_transfer(
        &self,
        args: &PostV1ProjectsByIdTransferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_projects_by_id_transfer_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_projects_by_id_transfer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 projects by project id compute services.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectsComputeservicesPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_projects_by_project_id_compute_services(
        &self,
        args: &PostV1ProjectsByProjectIdComputeServicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectsComputeservicesPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_projects_by_project_id_compute_services_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_projects_by_project_id_compute_services_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 projects by project id databases.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectsDatabasesPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_projects_by_project_id_databases(
        &self,
        args: &PostV1ProjectsByProjectIdDatabasesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectsDatabasesPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_projects_by_project_id_databases_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_projects_by_project_id_databases_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 versions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VersionsPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_versions(
        &self,
        args: &PostV1VersionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VersionsPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_versions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_versions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete v1 versions by version id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_v1_versions_by_version_id(
        &self,
        args: &DeleteV1VersionsByVersionIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_v1_versions_by_version_id_builder(
            &self.http_client,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_v1_versions_by_version_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 versions by version id start.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VersionsStartPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_versions_by_version_id_start(
        &self,
        args: &PostV1VersionsByVersionIdStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VersionsStartPostResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_versions_by_version_id_start_builder(
            &self.http_client,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_versions_by_version_id_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 versions by version id stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn post_v1_versions_by_version_id_stop(
        &self,
        args: &PostV1VersionsByVersionIdStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = post_v1_versions_by_version_id_stop_builder(
            &self.http_client,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = post_v1_versions_by_version_id_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Delete v1 workspaces by workspace id integrations by client id.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the () result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn delete_v1_workspaces_by_workspace_id_integrations_by_client_id(
        &self,
        args: &DeleteV1WorkspacesByWorkspaceIdIntegrationsByClientIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<(), ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = delete_v1_workspaces_by_workspace_id_integrations_by_client_id_builder(
            &self.http_client,
            &args.clientId,
            &args.workspaceId,
        )
        .map_err(ProviderError::Api)?;

        let task = delete_v1_workspaces_by_workspace_id_integrations_by_client_id_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
