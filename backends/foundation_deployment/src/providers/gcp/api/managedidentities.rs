//! ManagedidentitiesProvider - State-aware managedidentities API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       managedidentities API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::managedidentities::{
    managedidentities_projects_locations_get_builder, managedidentities_projects_locations_get_task,
    managedidentities_projects_locations_list_builder, managedidentities_projects_locations_list_task,
    managedidentities_projects_locations_global_domains_attach_trust_builder, managedidentities_projects_locations_global_domains_attach_trust_task,
    managedidentities_projects_locations_global_domains_check_migration_permission_builder, managedidentities_projects_locations_global_domains_check_migration_permission_task,
    managedidentities_projects_locations_global_domains_create_builder, managedidentities_projects_locations_global_domains_create_task,
    managedidentities_projects_locations_global_domains_delete_builder, managedidentities_projects_locations_global_domains_delete_task,
    managedidentities_projects_locations_global_domains_detach_trust_builder, managedidentities_projects_locations_global_domains_detach_trust_task,
    managedidentities_projects_locations_global_domains_disable_migration_builder, managedidentities_projects_locations_global_domains_disable_migration_task,
    managedidentities_projects_locations_global_domains_domain_join_machine_builder, managedidentities_projects_locations_global_domains_domain_join_machine_task,
    managedidentities_projects_locations_global_domains_enable_migration_builder, managedidentities_projects_locations_global_domains_enable_migration_task,
    managedidentities_projects_locations_global_domains_extend_schema_builder, managedidentities_projects_locations_global_domains_extend_schema_task,
    managedidentities_projects_locations_global_domains_get_builder, managedidentities_projects_locations_global_domains_get_task,
    managedidentities_projects_locations_global_domains_get_iam_policy_builder, managedidentities_projects_locations_global_domains_get_iam_policy_task,
    managedidentities_projects_locations_global_domains_get_ldapssettings_builder, managedidentities_projects_locations_global_domains_get_ldapssettings_task,
    managedidentities_projects_locations_global_domains_list_builder, managedidentities_projects_locations_global_domains_list_task,
    managedidentities_projects_locations_global_domains_patch_builder, managedidentities_projects_locations_global_domains_patch_task,
    managedidentities_projects_locations_global_domains_reconfigure_trust_builder, managedidentities_projects_locations_global_domains_reconfigure_trust_task,
    managedidentities_projects_locations_global_domains_reset_admin_password_builder, managedidentities_projects_locations_global_domains_reset_admin_password_task,
    managedidentities_projects_locations_global_domains_restore_builder, managedidentities_projects_locations_global_domains_restore_task,
    managedidentities_projects_locations_global_domains_set_iam_policy_builder, managedidentities_projects_locations_global_domains_set_iam_policy_task,
    managedidentities_projects_locations_global_domains_test_iam_permissions_builder, managedidentities_projects_locations_global_domains_test_iam_permissions_task,
    managedidentities_projects_locations_global_domains_update_ldapssettings_builder, managedidentities_projects_locations_global_domains_update_ldapssettings_task,
    managedidentities_projects_locations_global_domains_validate_trust_builder, managedidentities_projects_locations_global_domains_validate_trust_task,
    managedidentities_projects_locations_global_domains_backups_create_builder, managedidentities_projects_locations_global_domains_backups_create_task,
    managedidentities_projects_locations_global_domains_backups_delete_builder, managedidentities_projects_locations_global_domains_backups_delete_task,
    managedidentities_projects_locations_global_domains_backups_get_builder, managedidentities_projects_locations_global_domains_backups_get_task,
    managedidentities_projects_locations_global_domains_backups_get_iam_policy_builder, managedidentities_projects_locations_global_domains_backups_get_iam_policy_task,
    managedidentities_projects_locations_global_domains_backups_list_builder, managedidentities_projects_locations_global_domains_backups_list_task,
    managedidentities_projects_locations_global_domains_backups_patch_builder, managedidentities_projects_locations_global_domains_backups_patch_task,
    managedidentities_projects_locations_global_domains_backups_set_iam_policy_builder, managedidentities_projects_locations_global_domains_backups_set_iam_policy_task,
    managedidentities_projects_locations_global_domains_backups_test_iam_permissions_builder, managedidentities_projects_locations_global_domains_backups_test_iam_permissions_task,
    managedidentities_projects_locations_global_domains_sql_integrations_get_builder, managedidentities_projects_locations_global_domains_sql_integrations_get_task,
    managedidentities_projects_locations_global_domains_sql_integrations_list_builder, managedidentities_projects_locations_global_domains_sql_integrations_list_task,
    managedidentities_projects_locations_global_operations_cancel_builder, managedidentities_projects_locations_global_operations_cancel_task,
    managedidentities_projects_locations_global_operations_delete_builder, managedidentities_projects_locations_global_operations_delete_task,
    managedidentities_projects_locations_global_operations_get_builder, managedidentities_projects_locations_global_operations_get_task,
    managedidentities_projects_locations_global_operations_list_builder, managedidentities_projects_locations_global_operations_list_task,
    managedidentities_projects_locations_global_peerings_create_builder, managedidentities_projects_locations_global_peerings_create_task,
    managedidentities_projects_locations_global_peerings_delete_builder, managedidentities_projects_locations_global_peerings_delete_task,
    managedidentities_projects_locations_global_peerings_get_builder, managedidentities_projects_locations_global_peerings_get_task,
    managedidentities_projects_locations_global_peerings_get_iam_policy_builder, managedidentities_projects_locations_global_peerings_get_iam_policy_task,
    managedidentities_projects_locations_global_peerings_list_builder, managedidentities_projects_locations_global_peerings_list_task,
    managedidentities_projects_locations_global_peerings_patch_builder, managedidentities_projects_locations_global_peerings_patch_task,
    managedidentities_projects_locations_global_peerings_set_iam_policy_builder, managedidentities_projects_locations_global_peerings_set_iam_policy_task,
    managedidentities_projects_locations_global_peerings_test_iam_permissions_builder, managedidentities_projects_locations_global_peerings_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::managedidentities::Backup;
use crate::providers::gcp::clients::managedidentities::CheckMigrationPermissionResponse;
use crate::providers::gcp::clients::managedidentities::Domain;
use crate::providers::gcp::clients::managedidentities::DomainJoinMachineResponse;
use crate::providers::gcp::clients::managedidentities::Empty;
use crate::providers::gcp::clients::managedidentities::LDAPSSettings;
use crate::providers::gcp::clients::managedidentities::ListBackupsResponse;
use crate::providers::gcp::clients::managedidentities::ListDomainsResponse;
use crate::providers::gcp::clients::managedidentities::ListLocationsResponse;
use crate::providers::gcp::clients::managedidentities::ListOperationsResponse;
use crate::providers::gcp::clients::managedidentities::ListPeeringsResponse;
use crate::providers::gcp::clients::managedidentities::ListSqlIntegrationsResponse;
use crate::providers::gcp::clients::managedidentities::Location;
use crate::providers::gcp::clients::managedidentities::Operation;
use crate::providers::gcp::clients::managedidentities::Peering;
use crate::providers::gcp::clients::managedidentities::Policy;
use crate::providers::gcp::clients::managedidentities::ResetAdminPasswordResponse;
use crate::providers::gcp::clients::managedidentities::SqlIntegration;
use crate::providers::gcp::clients::managedidentities::TestIamPermissionsResponse;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGetArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsAttachTrustArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsBackupsCreateArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsBackupsDeleteArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsBackupsGetArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsBackupsGetIamPolicyArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsBackupsListArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsBackupsPatchArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsBackupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsCheckMigrationPermissionArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsCreateArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsDeleteArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsDetachTrustArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsDisableMigrationArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsDomainJoinMachineArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsEnableMigrationArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsExtendSchemaArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsGetArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsGetIamPolicyArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsGetLdapssettingsArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsListArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsPatchArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsReconfigureTrustArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsResetAdminPasswordArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsRestoreArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsSetIamPolicyArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsSqlIntegrationsGetArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsSqlIntegrationsListArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsTestIamPermissionsArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsUpdateLdapssettingsArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalDomainsValidateTrustArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalOperationsCancelArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalOperationsDeleteArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalOperationsGetArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalOperationsListArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalPeeringsCreateArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalPeeringsDeleteArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalPeeringsGetArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalPeeringsGetIamPolicyArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalPeeringsListArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalPeeringsPatchArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalPeeringsSetIamPolicyArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsGlobalPeeringsTestIamPermissionsArgs;
use crate::providers::gcp::clients::managedidentities::ManagedidentitiesProjectsLocationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ManagedidentitiesProvider with automatic state tracking.
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
/// let provider = ManagedidentitiesProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ManagedidentitiesProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ManagedidentitiesProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ManagedidentitiesProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ManagedidentitiesProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Managedidentities projects locations get.
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
    pub fn managedidentities_projects_locations_get(
        &self,
        args: &ManagedidentitiesProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations list.
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
    pub fn managedidentities_projects_locations_list(
        &self,
        args: &ManagedidentitiesProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains attach trust.
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
    pub fn managedidentities_projects_locations_global_domains_attach_trust(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsAttachTrustArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_attach_trust_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_attach_trust_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains check migration permission.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckMigrationPermissionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedidentities_projects_locations_global_domains_check_migration_permission(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsCheckMigrationPermissionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckMigrationPermissionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_check_migration_permission_builder(
            &self.http_client,
            &args.domain,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_check_migration_permission_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains create.
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
    pub fn managedidentities_projects_locations_global_domains_create(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_create_builder(
            &self.http_client,
            &args.parent,
            &args.domainName,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains delete.
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
    pub fn managedidentities_projects_locations_global_domains_delete(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains detach trust.
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
    pub fn managedidentities_projects_locations_global_domains_detach_trust(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsDetachTrustArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_detach_trust_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_detach_trust_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains disable migration.
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
    pub fn managedidentities_projects_locations_global_domains_disable_migration(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsDisableMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_disable_migration_builder(
            &self.http_client,
            &args.domain,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_disable_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains domain join machine.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DomainJoinMachineResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedidentities_projects_locations_global_domains_domain_join_machine(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsDomainJoinMachineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DomainJoinMachineResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_domain_join_machine_builder(
            &self.http_client,
            &args.domain,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_domain_join_machine_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains enable migration.
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
    pub fn managedidentities_projects_locations_global_domains_enable_migration(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsEnableMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_enable_migration_builder(
            &self.http_client,
            &args.domain,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_enable_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains extend schema.
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
    pub fn managedidentities_projects_locations_global_domains_extend_schema(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsExtendSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_extend_schema_builder(
            &self.http_client,
            &args.domain,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_extend_schema_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains get.
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
    pub fn managedidentities_projects_locations_global_domains_get(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Domain, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_domains_get_iam_policy(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains get ldapssettings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LDAPSSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_domains_get_ldapssettings(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsGetLdapssettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LDAPSSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_get_ldapssettings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_get_ldapssettings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains list.
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
    pub fn managedidentities_projects_locations_global_domains_list(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDomainsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains patch.
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
    pub fn managedidentities_projects_locations_global_domains_patch(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains reconfigure trust.
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
    pub fn managedidentities_projects_locations_global_domains_reconfigure_trust(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsReconfigureTrustArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_reconfigure_trust_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_reconfigure_trust_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains reset admin password.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResetAdminPasswordResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedidentities_projects_locations_global_domains_reset_admin_password(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsResetAdminPasswordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResetAdminPasswordResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_reset_admin_password_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_reset_admin_password_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains restore.
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
    pub fn managedidentities_projects_locations_global_domains_restore(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedidentities_projects_locations_global_domains_set_iam_policy(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_domains_test_iam_permissions(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains update ldapssettings.
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
    pub fn managedidentities_projects_locations_global_domains_update_ldapssettings(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsUpdateLdapssettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_update_ldapssettings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_update_ldapssettings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains validate trust.
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
    pub fn managedidentities_projects_locations_global_domains_validate_trust(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsValidateTrustArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_validate_trust_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_validate_trust_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains backups create.
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
    pub fn managedidentities_projects_locations_global_domains_backups_create(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsBackupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_backups_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_backups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains backups delete.
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
    pub fn managedidentities_projects_locations_global_domains_backups_delete(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_backups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains backups get.
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
    pub fn managedidentities_projects_locations_global_domains_backups_get(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains backups get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_domains_backups_get_iam_policy(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsBackupsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_backups_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_backups_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains backups list.
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
    pub fn managedidentities_projects_locations_global_domains_backups_list(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains backups patch.
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
    pub fn managedidentities_projects_locations_global_domains_backups_patch(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsBackupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_backups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_backups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains backups set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedidentities_projects_locations_global_domains_backups_set_iam_policy(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsBackupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_backups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_backups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains backups test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_domains_backups_test_iam_permissions(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsBackupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_backups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_backups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains sql integrations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SqlIntegration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_domains_sql_integrations_get(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsSqlIntegrationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SqlIntegration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_sql_integrations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_sql_integrations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global domains sql integrations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSqlIntegrationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_domains_sql_integrations_list(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalDomainsSqlIntegrationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSqlIntegrationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_domains_sql_integrations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_domains_sql_integrations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global operations cancel.
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
    pub fn managedidentities_projects_locations_global_operations_cancel(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global operations delete.
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
    pub fn managedidentities_projects_locations_global_operations_delete(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global operations get.
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
    pub fn managedidentities_projects_locations_global_operations_get(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global operations list.
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
    pub fn managedidentities_projects_locations_global_operations_list(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global peerings create.
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
    pub fn managedidentities_projects_locations_global_peerings_create(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalPeeringsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_peerings_create_builder(
            &self.http_client,
            &args.parent,
            &args.peeringId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_peerings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global peerings delete.
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
    pub fn managedidentities_projects_locations_global_peerings_delete(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalPeeringsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_peerings_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_peerings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global peerings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Peering result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_peerings_get(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalPeeringsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Peering, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_peerings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_peerings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global peerings get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_peerings_get_iam_policy(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalPeeringsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_peerings_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_peerings_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global peerings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPeeringsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_peerings_list(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalPeeringsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPeeringsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_peerings_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_peerings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global peerings patch.
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
    pub fn managedidentities_projects_locations_global_peerings_patch(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalPeeringsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_peerings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_peerings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global peerings set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedidentities_projects_locations_global_peerings_set_iam_policy(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalPeeringsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_peerings_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_peerings_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedidentities projects locations global peerings test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn managedidentities_projects_locations_global_peerings_test_iam_permissions(
        &self,
        args: &ManagedidentitiesProjectsLocationsGlobalPeeringsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedidentities_projects_locations_global_peerings_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = managedidentities_projects_locations_global_peerings_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
