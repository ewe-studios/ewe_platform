//! RedisProvider - State-aware redis API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       redis API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::redis::{
    redis_projects_locations_get_builder, redis_projects_locations_get_task,
    redis_projects_locations_get_shared_regional_certificate_authority_builder, redis_projects_locations_get_shared_regional_certificate_authority_task,
    redis_projects_locations_list_builder, redis_projects_locations_list_task,
    redis_projects_locations_acl_policies_create_builder, redis_projects_locations_acl_policies_create_task,
    redis_projects_locations_acl_policies_delete_builder, redis_projects_locations_acl_policies_delete_task,
    redis_projects_locations_acl_policies_get_builder, redis_projects_locations_acl_policies_get_task,
    redis_projects_locations_acl_policies_list_builder, redis_projects_locations_acl_policies_list_task,
    redis_projects_locations_acl_policies_patch_builder, redis_projects_locations_acl_policies_patch_task,
    redis_projects_locations_backup_collections_get_builder, redis_projects_locations_backup_collections_get_task,
    redis_projects_locations_backup_collections_list_builder, redis_projects_locations_backup_collections_list_task,
    redis_projects_locations_backup_collections_backups_delete_builder, redis_projects_locations_backup_collections_backups_delete_task,
    redis_projects_locations_backup_collections_backups_export_builder, redis_projects_locations_backup_collections_backups_export_task,
    redis_projects_locations_backup_collections_backups_get_builder, redis_projects_locations_backup_collections_backups_get_task,
    redis_projects_locations_backup_collections_backups_list_builder, redis_projects_locations_backup_collections_backups_list_task,
    redis_projects_locations_clusters_add_token_auth_user_builder, redis_projects_locations_clusters_add_token_auth_user_task,
    redis_projects_locations_clusters_backup_builder, redis_projects_locations_clusters_backup_task,
    redis_projects_locations_clusters_create_builder, redis_projects_locations_clusters_create_task,
    redis_projects_locations_clusters_delete_builder, redis_projects_locations_clusters_delete_task,
    redis_projects_locations_clusters_get_builder, redis_projects_locations_clusters_get_task,
    redis_projects_locations_clusters_get_certificate_authority_builder, redis_projects_locations_clusters_get_certificate_authority_task,
    redis_projects_locations_clusters_list_builder, redis_projects_locations_clusters_list_task,
    redis_projects_locations_clusters_patch_builder, redis_projects_locations_clusters_patch_task,
    redis_projects_locations_clusters_reschedule_cluster_maintenance_builder, redis_projects_locations_clusters_reschedule_cluster_maintenance_task,
    redis_projects_locations_clusters_token_auth_users_add_auth_token_builder, redis_projects_locations_clusters_token_auth_users_add_auth_token_task,
    redis_projects_locations_clusters_token_auth_users_delete_builder, redis_projects_locations_clusters_token_auth_users_delete_task,
    redis_projects_locations_clusters_token_auth_users_get_builder, redis_projects_locations_clusters_token_auth_users_get_task,
    redis_projects_locations_clusters_token_auth_users_list_builder, redis_projects_locations_clusters_token_auth_users_list_task,
    redis_projects_locations_clusters_token_auth_users_auth_tokens_delete_builder, redis_projects_locations_clusters_token_auth_users_auth_tokens_delete_task,
    redis_projects_locations_clusters_token_auth_users_auth_tokens_get_builder, redis_projects_locations_clusters_token_auth_users_auth_tokens_get_task,
    redis_projects_locations_clusters_token_auth_users_auth_tokens_list_builder, redis_projects_locations_clusters_token_auth_users_auth_tokens_list_task,
    redis_projects_locations_instances_create_builder, redis_projects_locations_instances_create_task,
    redis_projects_locations_instances_delete_builder, redis_projects_locations_instances_delete_task,
    redis_projects_locations_instances_export_builder, redis_projects_locations_instances_export_task,
    redis_projects_locations_instances_failover_builder, redis_projects_locations_instances_failover_task,
    redis_projects_locations_instances_get_builder, redis_projects_locations_instances_get_task,
    redis_projects_locations_instances_get_auth_string_builder, redis_projects_locations_instances_get_auth_string_task,
    redis_projects_locations_instances_import_builder, redis_projects_locations_instances_import_task,
    redis_projects_locations_instances_list_builder, redis_projects_locations_instances_list_task,
    redis_projects_locations_instances_patch_builder, redis_projects_locations_instances_patch_task,
    redis_projects_locations_instances_reschedule_maintenance_builder, redis_projects_locations_instances_reschedule_maintenance_task,
    redis_projects_locations_instances_upgrade_builder, redis_projects_locations_instances_upgrade_task,
    redis_projects_locations_operations_cancel_builder, redis_projects_locations_operations_cancel_task,
    redis_projects_locations_operations_delete_builder, redis_projects_locations_operations_delete_task,
    redis_projects_locations_operations_get_builder, redis_projects_locations_operations_get_task,
    redis_projects_locations_operations_list_builder, redis_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::redis::AclPolicy;
use crate::providers::gcp::clients::redis::AuthToken;
use crate::providers::gcp::clients::redis::Backup;
use crate::providers::gcp::clients::redis::BackupCollection;
use crate::providers::gcp::clients::redis::CertificateAuthority;
use crate::providers::gcp::clients::redis::Cluster;
use crate::providers::gcp::clients::redis::Empty;
use crate::providers::gcp::clients::redis::Instance;
use crate::providers::gcp::clients::redis::InstanceAuthString;
use crate::providers::gcp::clients::redis::ListAclPoliciesResponse;
use crate::providers::gcp::clients::redis::ListAuthTokensResponse;
use crate::providers::gcp::clients::redis::ListBackupCollectionsResponse;
use crate::providers::gcp::clients::redis::ListBackupsResponse;
use crate::providers::gcp::clients::redis::ListClustersResponse;
use crate::providers::gcp::clients::redis::ListInstancesResponse;
use crate::providers::gcp::clients::redis::ListLocationsResponse;
use crate::providers::gcp::clients::redis::ListOperationsResponse;
use crate::providers::gcp::clients::redis::ListTokenAuthUsersResponse;
use crate::providers::gcp::clients::redis::Location;
use crate::providers::gcp::clients::redis::Operation;
use crate::providers::gcp::clients::redis::SharedRegionalCertificateAuthority;
use crate::providers::gcp::clients::redis::TokenAuthUser;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsAclPoliciesCreateArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsAclPoliciesDeleteArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsAclPoliciesGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsAclPoliciesListArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsAclPoliciesPatchArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsBackupCollectionsBackupsDeleteArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsBackupCollectionsBackupsExportArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsBackupCollectionsBackupsGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsBackupCollectionsBackupsListArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsBackupCollectionsGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsBackupCollectionsListArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersAddTokenAuthUserArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersBackupArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersCreateArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersDeleteArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersGetCertificateAuthorityArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersListArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersPatchArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersRescheduleClusterMaintenanceArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersTokenAuthUsersAddAuthTokenArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersTokenAuthUsersAuthTokensDeleteArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersTokenAuthUsersAuthTokensGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersTokenAuthUsersAuthTokensListArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersTokenAuthUsersDeleteArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersTokenAuthUsersGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsClustersTokenAuthUsersListArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsGetSharedRegionalCertificateAuthorityArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesCreateArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesDeleteArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesExportArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesFailoverArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesGetAuthStringArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesImportArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesListArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesPatchArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesRescheduleMaintenanceArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsInstancesUpgradeArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsListArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::redis::RedisProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RedisProvider with automatic state tracking.
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
/// let provider = RedisProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct RedisProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> RedisProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new RedisProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new RedisProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Redis projects locations get.
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
    pub fn redis_projects_locations_get(
        &self,
        args: &RedisProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations get shared regional certificate authority.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SharedRegionalCertificateAuthority result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_get_shared_regional_certificate_authority(
        &self,
        args: &RedisProjectsLocationsGetSharedRegionalCertificateAuthorityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SharedRegionalCertificateAuthority, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_get_shared_regional_certificate_authority_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_get_shared_regional_certificate_authority_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations list.
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
    pub fn redis_projects_locations_list(
        &self,
        args: &RedisProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations acl policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AclPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn redis_projects_locations_acl_policies_create(
        &self,
        args: &RedisProjectsLocationsAclPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AclPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_acl_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.aclPolicyId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_acl_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations acl policies delete.
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
    pub fn redis_projects_locations_acl_policies_delete(
        &self,
        args: &RedisProjectsLocationsAclPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_acl_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_acl_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations acl policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AclPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_acl_policies_get(
        &self,
        args: &RedisProjectsLocationsAclPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AclPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_acl_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_acl_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations acl policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAclPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_acl_policies_list(
        &self,
        args: &RedisProjectsLocationsAclPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAclPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_acl_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_acl_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations acl policies patch.
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
    pub fn redis_projects_locations_acl_policies_patch(
        &self,
        args: &RedisProjectsLocationsAclPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_acl_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_acl_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations backup collections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupCollection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_backup_collections_get(
        &self,
        args: &RedisProjectsLocationsBackupCollectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupCollection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_backup_collections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_backup_collections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations backup collections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupCollectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_backup_collections_list(
        &self,
        args: &RedisProjectsLocationsBackupCollectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupCollectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_backup_collections_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_backup_collections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations backup collections backups delete.
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
    pub fn redis_projects_locations_backup_collections_backups_delete(
        &self,
        args: &RedisProjectsLocationsBackupCollectionsBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_backup_collections_backups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_backup_collections_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations backup collections backups export.
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
    pub fn redis_projects_locations_backup_collections_backups_export(
        &self,
        args: &RedisProjectsLocationsBackupCollectionsBackupsExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_backup_collections_backups_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_backup_collections_backups_export_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations backup collections backups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Backup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_backup_collections_backups_get(
        &self,
        args: &RedisProjectsLocationsBackupCollectionsBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_backup_collections_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_backup_collections_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations backup collections backups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_backup_collections_backups_list(
        &self,
        args: &RedisProjectsLocationsBackupCollectionsBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_backup_collections_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_backup_collections_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters add token auth user.
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
    pub fn redis_projects_locations_clusters_add_token_auth_user(
        &self,
        args: &RedisProjectsLocationsClustersAddTokenAuthUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_add_token_auth_user_builder(
            &self.http_client,
            &args.cluster,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_add_token_auth_user_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters backup.
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
    pub fn redis_projects_locations_clusters_backup(
        &self,
        args: &RedisProjectsLocationsClustersBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_backup_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters create.
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
    pub fn redis_projects_locations_clusters_create(
        &self,
        args: &RedisProjectsLocationsClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.clusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters delete.
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
    pub fn redis_projects_locations_clusters_delete(
        &self,
        args: &RedisProjectsLocationsClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Cluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_clusters_get(
        &self,
        args: &RedisProjectsLocationsClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters get certificate authority.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CertificateAuthority result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_clusters_get_certificate_authority(
        &self,
        args: &RedisProjectsLocationsClustersGetCertificateAuthorityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CertificateAuthority, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_get_certificate_authority_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_get_certificate_authority_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_clusters_list(
        &self,
        args: &RedisProjectsLocationsClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters patch.
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
    pub fn redis_projects_locations_clusters_patch(
        &self,
        args: &RedisProjectsLocationsClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters reschedule cluster maintenance.
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
    pub fn redis_projects_locations_clusters_reschedule_cluster_maintenance(
        &self,
        args: &RedisProjectsLocationsClustersRescheduleClusterMaintenanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_reschedule_cluster_maintenance_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_reschedule_cluster_maintenance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters token auth users add auth token.
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
    pub fn redis_projects_locations_clusters_token_auth_users_add_auth_token(
        &self,
        args: &RedisProjectsLocationsClustersTokenAuthUsersAddAuthTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_token_auth_users_add_auth_token_builder(
            &self.http_client,
            &args.tokenAuthUser,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_token_auth_users_add_auth_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters token auth users delete.
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
    pub fn redis_projects_locations_clusters_token_auth_users_delete(
        &self,
        args: &RedisProjectsLocationsClustersTokenAuthUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_token_auth_users_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_token_auth_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters token auth users get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TokenAuthUser result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_clusters_token_auth_users_get(
        &self,
        args: &RedisProjectsLocationsClustersTokenAuthUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TokenAuthUser, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_token_auth_users_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_token_auth_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters token auth users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTokenAuthUsersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_clusters_token_auth_users_list(
        &self,
        args: &RedisProjectsLocationsClustersTokenAuthUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTokenAuthUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_token_auth_users_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_token_auth_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters token auth users auth tokens delete.
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
    pub fn redis_projects_locations_clusters_token_auth_users_auth_tokens_delete(
        &self,
        args: &RedisProjectsLocationsClustersTokenAuthUsersAuthTokensDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_token_auth_users_auth_tokens_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_token_auth_users_auth_tokens_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters token auth users auth tokens get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_clusters_token_auth_users_auth_tokens_get(
        &self,
        args: &RedisProjectsLocationsClustersTokenAuthUsersAuthTokensGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_token_auth_users_auth_tokens_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_token_auth_users_auth_tokens_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations clusters token auth users auth tokens list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAuthTokensResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_clusters_token_auth_users_auth_tokens_list(
        &self,
        args: &RedisProjectsLocationsClustersTokenAuthUsersAuthTokensListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAuthTokensResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_clusters_token_auth_users_auth_tokens_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_clusters_token_auth_users_auth_tokens_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances create.
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
    pub fn redis_projects_locations_instances_create(
        &self,
        args: &RedisProjectsLocationsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.instanceId,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances delete.
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
    pub fn redis_projects_locations_instances_delete(
        &self,
        args: &RedisProjectsLocationsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances export.
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
    pub fn redis_projects_locations_instances_export(
        &self,
        args: &RedisProjectsLocationsInstancesExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_export_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances failover.
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
    pub fn redis_projects_locations_instances_failover(
        &self,
        args: &RedisProjectsLocationsInstancesFailoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_failover_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_failover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Instance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_instances_get(
        &self,
        args: &RedisProjectsLocationsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances get auth string.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstanceAuthString result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_instances_get_auth_string(
        &self,
        args: &RedisProjectsLocationsInstancesGetAuthStringArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstanceAuthString, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_get_auth_string_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_get_auth_string_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances import.
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
    pub fn redis_projects_locations_instances_import(
        &self,
        args: &RedisProjectsLocationsInstancesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_import_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInstancesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn redis_projects_locations_instances_list(
        &self,
        args: &RedisProjectsLocationsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances patch.
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
    pub fn redis_projects_locations_instances_patch(
        &self,
        args: &RedisProjectsLocationsInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances reschedule maintenance.
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
    pub fn redis_projects_locations_instances_reschedule_maintenance(
        &self,
        args: &RedisProjectsLocationsInstancesRescheduleMaintenanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_reschedule_maintenance_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_reschedule_maintenance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations instances upgrade.
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
    pub fn redis_projects_locations_instances_upgrade(
        &self,
        args: &RedisProjectsLocationsInstancesUpgradeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_instances_upgrade_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_instances_upgrade_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations operations cancel.
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
    pub fn redis_projects_locations_operations_cancel(
        &self,
        args: &RedisProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations operations delete.
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
    pub fn redis_projects_locations_operations_delete(
        &self,
        args: &RedisProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations operations get.
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
    pub fn redis_projects_locations_operations_get(
        &self,
        args: &RedisProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Redis projects locations operations list.
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
    pub fn redis_projects_locations_operations_list(
        &self,
        args: &RedisProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = redis_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = redis_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
