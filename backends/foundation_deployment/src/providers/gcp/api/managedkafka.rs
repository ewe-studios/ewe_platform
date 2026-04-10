//! ManagedkafkaProvider - State-aware managedkafka API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       managedkafka API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::managedkafka::{
    managedkafka_projects_locations_clusters_create_builder, managedkafka_projects_locations_clusters_create_task,
    managedkafka_projects_locations_clusters_delete_builder, managedkafka_projects_locations_clusters_delete_task,
    managedkafka_projects_locations_clusters_patch_builder, managedkafka_projects_locations_clusters_patch_task,
    managedkafka_projects_locations_clusters_acls_add_acl_entry_builder, managedkafka_projects_locations_clusters_acls_add_acl_entry_task,
    managedkafka_projects_locations_clusters_acls_create_builder, managedkafka_projects_locations_clusters_acls_create_task,
    managedkafka_projects_locations_clusters_acls_delete_builder, managedkafka_projects_locations_clusters_acls_delete_task,
    managedkafka_projects_locations_clusters_acls_patch_builder, managedkafka_projects_locations_clusters_acls_patch_task,
    managedkafka_projects_locations_clusters_acls_remove_acl_entry_builder, managedkafka_projects_locations_clusters_acls_remove_acl_entry_task,
    managedkafka_projects_locations_clusters_consumer_groups_delete_builder, managedkafka_projects_locations_clusters_consumer_groups_delete_task,
    managedkafka_projects_locations_clusters_consumer_groups_patch_builder, managedkafka_projects_locations_clusters_consumer_groups_patch_task,
    managedkafka_projects_locations_clusters_topics_create_builder, managedkafka_projects_locations_clusters_topics_create_task,
    managedkafka_projects_locations_clusters_topics_delete_builder, managedkafka_projects_locations_clusters_topics_delete_task,
    managedkafka_projects_locations_clusters_topics_patch_builder, managedkafka_projects_locations_clusters_topics_patch_task,
    managedkafka_projects_locations_connect_clusters_create_builder, managedkafka_projects_locations_connect_clusters_create_task,
    managedkafka_projects_locations_connect_clusters_delete_builder, managedkafka_projects_locations_connect_clusters_delete_task,
    managedkafka_projects_locations_connect_clusters_patch_builder, managedkafka_projects_locations_connect_clusters_patch_task,
    managedkafka_projects_locations_connect_clusters_connectors_create_builder, managedkafka_projects_locations_connect_clusters_connectors_create_task,
    managedkafka_projects_locations_connect_clusters_connectors_delete_builder, managedkafka_projects_locations_connect_clusters_connectors_delete_task,
    managedkafka_projects_locations_connect_clusters_connectors_patch_builder, managedkafka_projects_locations_connect_clusters_connectors_patch_task,
    managedkafka_projects_locations_connect_clusters_connectors_pause_builder, managedkafka_projects_locations_connect_clusters_connectors_pause_task,
    managedkafka_projects_locations_connect_clusters_connectors_restart_builder, managedkafka_projects_locations_connect_clusters_connectors_restart_task,
    managedkafka_projects_locations_connect_clusters_connectors_resume_builder, managedkafka_projects_locations_connect_clusters_connectors_resume_task,
    managedkafka_projects_locations_connect_clusters_connectors_stop_builder, managedkafka_projects_locations_connect_clusters_connectors_stop_task,
    managedkafka_projects_locations_operations_cancel_builder, managedkafka_projects_locations_operations_cancel_task,
    managedkafka_projects_locations_operations_delete_builder, managedkafka_projects_locations_operations_delete_task,
    managedkafka_projects_locations_schema_registries_create_builder, managedkafka_projects_locations_schema_registries_create_task,
    managedkafka_projects_locations_schema_registries_delete_builder, managedkafka_projects_locations_schema_registries_delete_task,
    managedkafka_projects_locations_schema_registries_compatibility_check_compatibility_builder, managedkafka_projects_locations_schema_registries_compatibility_check_compatibility_task,
    managedkafka_projects_locations_schema_registries_config_delete_builder, managedkafka_projects_locations_schema_registries_config_delete_task,
    managedkafka_projects_locations_schema_registries_config_update_builder, managedkafka_projects_locations_schema_registries_config_update_task,
    managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility_builder, managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility_task,
    managedkafka_projects_locations_schema_registries_contexts_config_delete_builder, managedkafka_projects_locations_schema_registries_contexts_config_delete_task,
    managedkafka_projects_locations_schema_registries_contexts_config_update_builder, managedkafka_projects_locations_schema_registries_contexts_config_update_task,
    managedkafka_projects_locations_schema_registries_contexts_mode_delete_builder, managedkafka_projects_locations_schema_registries_contexts_mode_delete_task,
    managedkafka_projects_locations_schema_registries_contexts_mode_update_builder, managedkafka_projects_locations_schema_registries_contexts_mode_update_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_delete_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_delete_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create_task,
    managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete_builder, managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete_task,
    managedkafka_projects_locations_schema_registries_mode_delete_builder, managedkafka_projects_locations_schema_registries_mode_delete_task,
    managedkafka_projects_locations_schema_registries_mode_update_builder, managedkafka_projects_locations_schema_registries_mode_update_task,
    managedkafka_projects_locations_schema_registries_subjects_delete_builder, managedkafka_projects_locations_schema_registries_subjects_delete_task,
    managedkafka_projects_locations_schema_registries_subjects_lookup_version_builder, managedkafka_projects_locations_schema_registries_subjects_lookup_version_task,
    managedkafka_projects_locations_schema_registries_subjects_versions_create_builder, managedkafka_projects_locations_schema_registries_subjects_versions_create_task,
    managedkafka_projects_locations_schema_registries_subjects_versions_delete_builder, managedkafka_projects_locations_schema_registries_subjects_versions_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::managedkafka::Acl;
use crate::providers::gcp::clients::managedkafka::AddAclEntryResponse;
use crate::providers::gcp::clients::managedkafka::CheckCompatibilityResponse;
use crate::providers::gcp::clients::managedkafka::Connector;
use crate::providers::gcp::clients::managedkafka::ConsumerGroup;
use crate::providers::gcp::clients::managedkafka::CreateVersionResponse;
use crate::providers::gcp::clients::managedkafka::Empty;
use crate::providers::gcp::clients::managedkafka::HttpBody;
use crate::providers::gcp::clients::managedkafka::Operation;
use crate::providers::gcp::clients::managedkafka::PauseConnectorResponse;
use crate::providers::gcp::clients::managedkafka::RemoveAclEntryResponse;
use crate::providers::gcp::clients::managedkafka::RestartConnectorResponse;
use crate::providers::gcp::clients::managedkafka::ResumeConnectorResponse;
use crate::providers::gcp::clients::managedkafka::SchemaConfig;
use crate::providers::gcp::clients::managedkafka::SchemaMode;
use crate::providers::gcp::clients::managedkafka::SchemaRegistry;
use crate::providers::gcp::clients::managedkafka::SchemaVersion;
use crate::providers::gcp::clients::managedkafka::StopConnectorResponse;
use crate::providers::gcp::clients::managedkafka::Topic;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsAddAclEntryArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersAclsRemoveAclEntryArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersConsumerGroupsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersConsumerGroupsPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersTopicsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersTopicsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsClustersTopicsPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsPauseArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsRestartArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsResumeArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersConnectorsStopArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsConnectClustersPatchArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesCompatibilityCheckCompatibilityArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesConfigDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesConfigUpdateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsCompatibilityCheckCompatibilityArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigUpdateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeUpdateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsLookupVersionArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesModeDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesModeUpdateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsDeleteArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsLookupVersionArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsCreateArgs;
use crate::providers::gcp::clients::managedkafka::ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ManagedkafkaProvider with automatic state tracking.
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
/// let provider = ManagedkafkaProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ManagedkafkaProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ManagedkafkaProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ManagedkafkaProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Managedkafka projects locations clusters create.
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
    pub fn managedkafka_projects_locations_clusters_create(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.clusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters delete.
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
    pub fn managedkafka_projects_locations_clusters_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters patch.
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
    pub fn managedkafka_projects_locations_clusters_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls add acl entry.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddAclEntryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_acls_add_acl_entry(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsAddAclEntryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddAclEntryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_add_acl_entry_builder(
            &self.http_client,
            &args.acl,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_add_acl_entry_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Acl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_acls_create(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Acl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_create_builder(
            &self.http_client,
            &args.parent,
            &args.aclId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls delete.
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
    pub fn managedkafka_projects_locations_clusters_acls_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Acl result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_acls_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Acl, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters acls remove acl entry.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemoveAclEntryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_acls_remove_acl_entry(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersAclsRemoveAclEntryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemoveAclEntryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_acls_remove_acl_entry_builder(
            &self.http_client,
            &args.acl,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_acls_remove_acl_entry_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters consumer groups delete.
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
    pub fn managedkafka_projects_locations_clusters_consumer_groups_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersConsumerGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_consumer_groups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_consumer_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters consumer groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConsumerGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_consumer_groups_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersConsumerGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConsumerGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_consumer_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_consumer_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters topics create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_topics_create(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersTopicsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_topics_create_builder(
            &self.http_client,
            &args.parent,
            &args.topicId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_topics_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters topics delete.
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
    pub fn managedkafka_projects_locations_clusters_topics_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersTopicsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_topics_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_topics_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations clusters topics patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_clusters_topics_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsClustersTopicsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_clusters_topics_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_clusters_topics_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters create.
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
    pub fn managedkafka_projects_locations_connect_clusters_create(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectClusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters delete.
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
    pub fn managedkafka_projects_locations_connect_clusters_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters patch.
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
    pub fn managedkafka_projects_locations_connect_clusters_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Connector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_create(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectorId,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors delete.
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
    pub fn managedkafka_projects_locations_connect_clusters_connectors_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Connector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_patch(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors pause.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PauseConnectorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_pause(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PauseConnectorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors restart.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RestartConnectorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_restart(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsRestartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RestartConnectorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_restart_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_restart_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResumeConnectorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_resume(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResumeConnectorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations connect clusters connectors stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StopConnectorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_connect_clusters_connectors_stop(
        &self,
        args: &ManagedkafkaProjectsLocationsConnectClustersConnectorsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StopConnectorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_connect_clusters_connectors_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_connect_clusters_connectors_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations operations cancel.
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
    pub fn managedkafka_projects_locations_operations_cancel(
        &self,
        args: &ManagedkafkaProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations operations delete.
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
    pub fn managedkafka_projects_locations_operations_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaRegistry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_create(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaRegistry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries delete.
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
    pub fn managedkafka_projects_locations_schema_registries_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries compatibility check compatibility.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckCompatibilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_compatibility_check_compatibility(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesCompatibilityCheckCompatibilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckCompatibilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_compatibility_check_compatibility_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_compatibility_check_compatibility_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries config delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_config_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesConfigDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_config_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_config_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries config update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_config_update(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesConfigUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_config_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_config_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts compatibility check compatibility.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckCompatibilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsCompatibilityCheckCompatibilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckCompatibilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_compatibility_check_compatibility_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts config delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_config_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_config_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_config_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts config update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_config_update(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsConfigUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_config_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_config_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts mode delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_mode_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_mode_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_mode_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts mode update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_mode_update(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsModeUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_mode_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_mode_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_delete_builder(
            &self.http_client,
            &args.name,
            &args.permanent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects lookup version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsLookupVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_lookup_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries contexts subjects versions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesContextsSubjectsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.permanent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_contexts_subjects_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries mode delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_mode_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesModeDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_mode_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_mode_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries mode update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaMode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_mode_update(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesModeUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaMode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_mode_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_mode_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_delete_builder(
            &self.http_client,
            &args.name,
            &args.permanent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects lookup version.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_lookup_version(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsLookupVersionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_lookup_version_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_lookup_version_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CreateVersionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_versions_create(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CreateVersionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Managedkafka projects locations schema registries subjects versions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn managedkafka_projects_locations_schema_registries_subjects_versions_delete(
        &self,
        args: &ManagedkafkaProjectsLocationsSchemaRegistriesSubjectsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = managedkafka_projects_locations_schema_registries_subjects_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.permanent,
        )
        .map_err(ProviderError::Api)?;

        let task = managedkafka_projects_locations_schema_registries_subjects_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
