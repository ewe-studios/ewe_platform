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

use crate::providers::prisma_postgres::clients::{
    get_v1_compute_services_builder, get_v1_compute_services_task,
    post_v1_compute_services_builder, post_v1_compute_services_task,
    get_v1_compute_services_versions_by_version_id_builder, get_v1_compute_services_versions_by_version_id_task,
    delete_v1_compute_services_versions_by_version_id_builder, delete_v1_compute_services_versions_by_version_id_task,
    post_v1_compute_services_versions_by_version_id_start_builder, post_v1_compute_services_versions_by_version_id_start_task,
    post_v1_compute_services_versions_by_version_id_stop_builder, post_v1_compute_services_versions_by_version_id_stop_task,
    get_v1_compute_services_by_compute_service_id_builder, get_v1_compute_services_by_compute_service_id_task,
    patch_v1_compute_services_by_compute_service_id_builder, patch_v1_compute_services_by_compute_service_id_task,
    delete_v1_compute_services_by_compute_service_id_builder, delete_v1_compute_services_by_compute_service_id_task,
    post_v1_compute_services_by_compute_service_id_promote_builder, post_v1_compute_services_by_compute_service_id_promote_task,
    get_v1_compute_services_by_compute_service_id_versions_builder, get_v1_compute_services_by_compute_service_id_versions_task,
    post_v1_compute_services_by_compute_service_id_versions_builder, post_v1_compute_services_by_compute_service_id_versions_task,
    get_v1_connections_builder, get_v1_connections_task,
    post_v1_connections_builder, post_v1_connections_task,
    get_v1_connections_by_id_builder, get_v1_connections_by_id_task,
    delete_v1_connections_by_id_builder, delete_v1_connections_by_id_task,
    post_v1_connections_by_id_rotate_builder, post_v1_connections_by_id_rotate_task,
    get_v1_databases_builder, get_v1_databases_task,
    post_v1_databases_builder, post_v1_databases_task,
    get_v1_databases_by_database_id_builder, get_v1_databases_by_database_id_task,
    patch_v1_databases_by_database_id_builder, patch_v1_databases_by_database_id_task,
    delete_v1_databases_by_database_id_builder, delete_v1_databases_by_database_id_task,
    get_v1_databases_by_database_id_backups_builder, get_v1_databases_by_database_id_backups_task,
    get_v1_databases_by_database_id_connections_builder, get_v1_databases_by_database_id_connections_task,
    post_v1_databases_by_database_id_connections_builder, post_v1_databases_by_database_id_connections_task,
    get_v1_databases_by_database_id_usage_builder, get_v1_databases_by_database_id_usage_task,
    post_v1_databases_by_target_database_id_restore_builder, post_v1_databases_by_target_database_id_restore_task,
    get_v1_integrations_builder, get_v1_integrations_task,
    get_v1_integrations_by_id_builder, get_v1_integrations_by_id_task,
    delete_v1_integrations_by_id_builder, delete_v1_integrations_by_id_task,
    get_v1_projects_builder, get_v1_projects_task,
    post_v1_projects_builder, post_v1_projects_task,
    get_v1_projects_by_id_builder, get_v1_projects_by_id_task,
    patch_v1_projects_by_id_builder, patch_v1_projects_by_id_task,
    delete_v1_projects_by_id_builder, delete_v1_projects_by_id_task,
    post_v1_projects_by_id_transfer_builder, post_v1_projects_by_id_transfer_task,
    get_v1_projects_by_project_id_compute_services_builder, get_v1_projects_by_project_id_compute_services_task,
    post_v1_projects_by_project_id_compute_services_builder, post_v1_projects_by_project_id_compute_services_task,
    get_v1_projects_by_project_id_databases_builder, get_v1_projects_by_project_id_databases_task,
    post_v1_projects_by_project_id_databases_builder, post_v1_projects_by_project_id_databases_task,
    get_v1_regions_builder, get_v1_regions_task,
    get_v1_regions_accelerate_builder, get_v1_regions_accelerate_task,
    get_v1_regions_postgres_builder, get_v1_regions_postgres_task,
    get_v1_versions_builder, get_v1_versions_task,
    post_v1_versions_builder, post_v1_versions_task,
    get_v1_versions_by_version_id_builder, get_v1_versions_by_version_id_task,
    delete_v1_versions_by_version_id_builder, delete_v1_versions_by_version_id_task,
    post_v1_versions_by_version_id_start_builder, post_v1_versions_by_version_id_start_task,
    post_v1_versions_by_version_id_stop_builder, post_v1_versions_by_version_id_stop_task,
    get_v1_workspaces_builder, get_v1_workspaces_task,
    get_v1_workspaces_by_id_builder, get_v1_workspaces_by_id_task,
    get_v1_workspaces_by_workspace_id_integrations_builder, get_v1_workspaces_by_workspace_id_integrations_task,
    delete_v1_workspaces_by_workspace_id_integrations_by_client_id_builder, delete_v1_workspaces_by_workspace_id_integrations_by_client_id_task,
};
use crate::providers::prisma_postgres::clients::types::{ApiError, ApiPending};
use crate::providers::prisma_postgres::clients::ComputeservicesGetResponse;
use crate::providers::prisma_postgres::clients::ComputeservicesPatchResponse;
use crate::providers::prisma_postgres::clients::ComputeservicesPostResponse;
use crate::providers::prisma_postgres::clients::ComputeservicesPromotePostResponse;
use crate::providers::prisma_postgres::clients::ComputeservicesVersionsGetResponse;
use crate::providers::prisma_postgres::clients::ComputeservicesVersionsPostResponse;
use crate::providers::prisma_postgres::clients::ComputeservicesVersionsStartPostResponse;
use crate::providers::prisma_postgres::clients::ConnectionsGetResponse;
use crate::providers::prisma_postgres::clients::ConnectionsPostResponse;
use crate::providers::prisma_postgres::clients::ConnectionsRotatePostResponse;
use crate::providers::prisma_postgres::clients::DatabasesBackupsGetResponse;
use crate::providers::prisma_postgres::clients::DatabasesConnectionsGetResponse;
use crate::providers::prisma_postgres::clients::DatabasesConnectionsPostResponse;
use crate::providers::prisma_postgres::clients::DatabasesGetResponse;
use crate::providers::prisma_postgres::clients::DatabasesPatchResponse;
use crate::providers::prisma_postgres::clients::DatabasesPostResponse;
use crate::providers::prisma_postgres::clients::DatabasesRestorePostResponse;
use crate::providers::prisma_postgres::clients::DatabasesUsageGetResponse;
use crate::providers::prisma_postgres::clients::IntegrationsGetResponse;
use crate::providers::prisma_postgres::clients::ProjectsComputeservicesGetResponse;
use crate::providers::prisma_postgres::clients::ProjectsComputeservicesPostResponse;
use crate::providers::prisma_postgres::clients::ProjectsDatabasesGetResponse;
use crate::providers::prisma_postgres::clients::ProjectsDatabasesPostResponse;
use crate::providers::prisma_postgres::clients::ProjectsGetResponse;
use crate::providers::prisma_postgres::clients::ProjectsPatchResponse;
use crate::providers::prisma_postgres::clients::ProjectsPostResponse;
use crate::providers::prisma_postgres::clients::RegionsAccelerateGetResponse;
use crate::providers::prisma_postgres::clients::RegionsGetResponse;
use crate::providers::prisma_postgres::clients::RegionsPostgresGetResponse;
use crate::providers::prisma_postgres::clients::VersionsGetResponse;
use crate::providers::prisma_postgres::clients::VersionsPostResponse;
use crate::providers::prisma_postgres::clients::VersionsStartPostResponse;
use crate::providers::prisma_postgres::clients::WorkspacesGetResponse;
use crate::providers::prisma_postgres::clients::WorkspacesIntegrationsGetResponse;
use crate::providers::prisma_postgres::clients::DeleteV1ComputeServicesByComputeServiceIdArgs;
use crate::providers::prisma_postgres::clients::DeleteV1ComputeServicesVersionsByVersionIdArgs;
use crate::providers::prisma_postgres::clients::DeleteV1ConnectionsByIdArgs;
use crate::providers::prisma_postgres::clients::DeleteV1DatabasesByDatabaseIdArgs;
use crate::providers::prisma_postgres::clients::DeleteV1IntegrationsByIdArgs;
use crate::providers::prisma_postgres::clients::DeleteV1ProjectsByIdArgs;
use crate::providers::prisma_postgres::clients::DeleteV1VersionsByVersionIdArgs;
use crate::providers::prisma_postgres::clients::DeleteV1WorkspacesByWorkspaceIdIntegrationsByClientIdArgs;
use crate::providers::prisma_postgres::clients::GetV1ComputeServicesArgs;
use crate::providers::prisma_postgres::clients::GetV1ComputeServicesByComputeServiceIdArgs;
use crate::providers::prisma_postgres::clients::GetV1ComputeServicesByComputeServiceIdVersionsArgs;
use crate::providers::prisma_postgres::clients::GetV1ComputeServicesVersionsByVersionIdArgs;
use crate::providers::prisma_postgres::clients::GetV1ConnectionsArgs;
use crate::providers::prisma_postgres::clients::GetV1ConnectionsByIdArgs;
use crate::providers::prisma_postgres::clients::GetV1DatabasesArgs;
use crate::providers::prisma_postgres::clients::GetV1DatabasesByDatabaseIdArgs;
use crate::providers::prisma_postgres::clients::GetV1DatabasesByDatabaseIdBackupsArgs;
use crate::providers::prisma_postgres::clients::GetV1DatabasesByDatabaseIdConnectionsArgs;
use crate::providers::prisma_postgres::clients::GetV1DatabasesByDatabaseIdUsageArgs;
use crate::providers::prisma_postgres::clients::GetV1IntegrationsArgs;
use crate::providers::prisma_postgres::clients::GetV1IntegrationsByIdArgs;
use crate::providers::prisma_postgres::clients::GetV1ProjectsArgs;
use crate::providers::prisma_postgres::clients::GetV1ProjectsByIdArgs;
use crate::providers::prisma_postgres::clients::GetV1ProjectsByProjectIdComputeServicesArgs;
use crate::providers::prisma_postgres::clients::GetV1ProjectsByProjectIdDatabasesArgs;
use crate::providers::prisma_postgres::clients::GetV1RegionsArgs;
use crate::providers::prisma_postgres::clients::GetV1VersionsArgs;
use crate::providers::prisma_postgres::clients::GetV1VersionsByVersionIdArgs;
use crate::providers::prisma_postgres::clients::GetV1WorkspacesArgs;
use crate::providers::prisma_postgres::clients::GetV1WorkspacesByIdArgs;
use crate::providers::prisma_postgres::clients::GetV1WorkspacesByWorkspaceIdIntegrationsArgs;
use crate::providers::prisma_postgres::clients::PatchV1ComputeServicesByComputeServiceIdArgs;
use crate::providers::prisma_postgres::clients::PatchV1DatabasesByDatabaseIdArgs;
use crate::providers::prisma_postgres::clients::PatchV1ProjectsByIdArgs;
use crate::providers::prisma_postgres::clients::PostV1ComputeServicesArgs;
use crate::providers::prisma_postgres::clients::PostV1ComputeServicesByComputeServiceIdPromoteArgs;
use crate::providers::prisma_postgres::clients::PostV1ComputeServicesByComputeServiceIdVersionsArgs;
use crate::providers::prisma_postgres::clients::PostV1ComputeServicesVersionsByVersionIdStartArgs;
use crate::providers::prisma_postgres::clients::PostV1ComputeServicesVersionsByVersionIdStopArgs;
use crate::providers::prisma_postgres::clients::PostV1ConnectionsArgs;
use crate::providers::prisma_postgres::clients::PostV1ConnectionsByIdRotateArgs;
use crate::providers::prisma_postgres::clients::PostV1DatabasesArgs;
use crate::providers::prisma_postgres::clients::PostV1DatabasesByDatabaseIdConnectionsArgs;
use crate::providers::prisma_postgres::clients::PostV1DatabasesByTargetDatabaseIdRestoreArgs;
use crate::providers::prisma_postgres::clients::PostV1ProjectsArgs;
use crate::providers::prisma_postgres::clients::PostV1ProjectsByIdTransferArgs;
use crate::providers::prisma_postgres::clients::PostV1ProjectsByProjectIdComputeServicesArgs;
use crate::providers::prisma_postgres::clients::PostV1ProjectsByProjectIdDatabasesArgs;
use crate::providers::prisma_postgres::clients::PostV1VersionsArgs;
use crate::providers::prisma_postgres::clients::PostV1VersionsByVersionIdStartArgs;
use crate::providers::prisma_postgres::clients::PostV1VersionsByVersionIdStopArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PrismaPostgresProvider with automatic state tracking.
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
/// let provider = PrismaPostgresProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PrismaPostgresProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PrismaPostgresProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PrismaPostgresProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PrismaPostgresProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Get v1 compute services.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_compute_services(
        &self,
        args: &GetV1ComputeServicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_compute_services_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_compute_services_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 compute services versions by version id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesVersionsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_compute_services_versions_by_version_id(
        &self,
        args: &GetV1ComputeServicesVersionsByVersionIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesVersionsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_compute_services_versions_by_version_id_builder(
            &self.http_client,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_compute_services_versions_by_version_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 compute services by compute service id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_compute_services_by_compute_service_id(
        &self,
        args: &GetV1ComputeServicesByComputeServiceIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_compute_services_by_compute_service_id_builder(
            &self.http_client,
            &args.computeServiceId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_compute_services_by_compute_service_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 compute services by compute service id versions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeservicesVersionsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_compute_services_by_compute_service_id_versions(
        &self,
        args: &GetV1ComputeServicesByComputeServiceIdVersionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeservicesVersionsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_compute_services_by_compute_service_id_versions_builder(
            &self.http_client,
            &args.computeServiceId,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_compute_services_by_compute_service_id_versions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 connections.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_connections(
        &self,
        args: &GetV1ConnectionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_connections_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.databaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_connections_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 connections by id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_connections_by_id(
        &self,
        args: &GetV1ConnectionsByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_connections_by_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_connections_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 databases.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_databases(
        &self,
        args: &GetV1DatabasesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_databases_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_databases_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 databases by database id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_databases_by_database_id(
        &self,
        args: &GetV1DatabasesByDatabaseIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_databases_by_database_id_builder(
            &self.http_client,
            &args.databaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_databases_by_database_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 databases by database id backups.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesBackupsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_databases_by_database_id_backups(
        &self,
        args: &GetV1DatabasesByDatabaseIdBackupsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesBackupsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_databases_by_database_id_backups_builder(
            &self.http_client,
            &args.databaseId,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_databases_by_database_id_backups_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get v1 databases by database id connections.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesConnectionsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_databases_by_database_id_connections(
        &self,
        args: &GetV1DatabasesByDatabaseIdConnectionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesConnectionsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_databases_by_database_id_connections_builder(
            &self.http_client,
            &args.databaseId,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_databases_by_database_id_connections_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 databases by database id usage.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesUsageGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_databases_by_database_id_usage(
        &self,
        args: &GetV1DatabasesByDatabaseIdUsageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesUsageGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_databases_by_database_id_usage_builder(
            &self.http_client,
            &args.databaseId,
            &args.startDate,
            &args.endDate,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_databases_by_database_id_usage_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Post v1 databases by target database id restore.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get v1 integrations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IntegrationsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_integrations(
        &self,
        args: &GetV1IntegrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IntegrationsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_integrations_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.workspaceId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_integrations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get v1 integrations by id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IntegrationsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_integrations_by_id(
        &self,
        args: &GetV1IntegrationsByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IntegrationsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_integrations_by_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_integrations_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 projects.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_projects(
        &self,
        args: &GetV1ProjectsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_projects_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_projects_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 projects by id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_projects_by_id(
        &self,
        args: &GetV1ProjectsByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_projects_by_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_projects_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 projects by project id compute services.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectsComputeservicesGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_projects_by_project_id_compute_services(
        &self,
        args: &GetV1ProjectsByProjectIdComputeServicesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectsComputeservicesGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_projects_by_project_id_compute_services_builder(
            &self.http_client,
            &args.projectId,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_projects_by_project_id_compute_services_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 projects by project id databases.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectsDatabasesGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_projects_by_project_id_databases(
        &self,
        args: &GetV1ProjectsByProjectIdDatabasesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectsDatabasesGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_projects_by_project_id_databases_builder(
            &self.http_client,
            &args.projectId,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_projects_by_project_id_databases_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 regions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RegionsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_regions(
        &self,
        args: &GetV1RegionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RegionsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_regions_builder(
            &self.http_client,
            &args.product,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_regions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get v1 regions accelerate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RegionsAccelerateGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_regions_accelerate(
        &self,
        args: &GetV1RegionsAccelerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RegionsAccelerateGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_regions_accelerate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_regions_accelerate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get v1 regions postgres.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RegionsPostgresGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_regions_postgres(
        &self,
        args: &GetV1RegionsPostgresArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RegionsPostgresGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_regions_postgres_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_regions_postgres_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get v1 versions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VersionsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_versions(
        &self,
        args: &GetV1VersionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VersionsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_versions_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
            &args.computeServiceId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_versions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 versions by version id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VersionsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_versions_by_version_id(
        &self,
        args: &GetV1VersionsByVersionIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VersionsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_versions_by_version_id_builder(
            &self.http_client,
            &args.versionId,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_versions_by_version_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Get v1 workspaces.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkspacesGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_workspaces(
        &self,
        args: &GetV1WorkspacesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkspacesGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_workspaces_builder(
            &self.http_client,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_workspaces_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get v1 workspaces by id.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkspacesGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_workspaces_by_id(
        &self,
        args: &GetV1WorkspacesByIdArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkspacesGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_workspaces_by_id_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_workspaces_by_id_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get v1 workspaces by workspace id integrations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkspacesIntegrationsGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn get_v1_workspaces_by_workspace_id_integrations(
        &self,
        args: &GetV1WorkspacesByWorkspaceIdIntegrationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkspacesIntegrationsGetResponse, ProviderError<ApiError>>,
            P = crate::providers::prisma_postgres::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = get_v1_workspaces_by_workspace_id_integrations_builder(
            &self.http_client,
            &args.workspaceId,
            &args.cursor,
            &args.limit,
        )
        .map_err(ProviderError::Api)?;

        let task = get_v1_workspaces_by_workspace_id_integrations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
