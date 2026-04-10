//! EventarcProvider - State-aware eventarc API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       eventarc API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::eventarc::{
    eventarc_projects_locations_update_google_channel_config_builder, eventarc_projects_locations_update_google_channel_config_task,
    eventarc_projects_locations_channel_connections_create_builder, eventarc_projects_locations_channel_connections_create_task,
    eventarc_projects_locations_channel_connections_delete_builder, eventarc_projects_locations_channel_connections_delete_task,
    eventarc_projects_locations_channel_connections_set_iam_policy_builder, eventarc_projects_locations_channel_connections_set_iam_policy_task,
    eventarc_projects_locations_channel_connections_test_iam_permissions_builder, eventarc_projects_locations_channel_connections_test_iam_permissions_task,
    eventarc_projects_locations_channels_create_builder, eventarc_projects_locations_channels_create_task,
    eventarc_projects_locations_channels_delete_builder, eventarc_projects_locations_channels_delete_task,
    eventarc_projects_locations_channels_patch_builder, eventarc_projects_locations_channels_patch_task,
    eventarc_projects_locations_channels_set_iam_policy_builder, eventarc_projects_locations_channels_set_iam_policy_task,
    eventarc_projects_locations_channels_test_iam_permissions_builder, eventarc_projects_locations_channels_test_iam_permissions_task,
    eventarc_projects_locations_enrollments_create_builder, eventarc_projects_locations_enrollments_create_task,
    eventarc_projects_locations_enrollments_delete_builder, eventarc_projects_locations_enrollments_delete_task,
    eventarc_projects_locations_enrollments_patch_builder, eventarc_projects_locations_enrollments_patch_task,
    eventarc_projects_locations_enrollments_set_iam_policy_builder, eventarc_projects_locations_enrollments_set_iam_policy_task,
    eventarc_projects_locations_enrollments_test_iam_permissions_builder, eventarc_projects_locations_enrollments_test_iam_permissions_task,
    eventarc_projects_locations_google_api_sources_create_builder, eventarc_projects_locations_google_api_sources_create_task,
    eventarc_projects_locations_google_api_sources_delete_builder, eventarc_projects_locations_google_api_sources_delete_task,
    eventarc_projects_locations_google_api_sources_patch_builder, eventarc_projects_locations_google_api_sources_patch_task,
    eventarc_projects_locations_google_api_sources_set_iam_policy_builder, eventarc_projects_locations_google_api_sources_set_iam_policy_task,
    eventarc_projects_locations_google_api_sources_test_iam_permissions_builder, eventarc_projects_locations_google_api_sources_test_iam_permissions_task,
    eventarc_projects_locations_message_buses_create_builder, eventarc_projects_locations_message_buses_create_task,
    eventarc_projects_locations_message_buses_delete_builder, eventarc_projects_locations_message_buses_delete_task,
    eventarc_projects_locations_message_buses_patch_builder, eventarc_projects_locations_message_buses_patch_task,
    eventarc_projects_locations_message_buses_set_iam_policy_builder, eventarc_projects_locations_message_buses_set_iam_policy_task,
    eventarc_projects_locations_message_buses_test_iam_permissions_builder, eventarc_projects_locations_message_buses_test_iam_permissions_task,
    eventarc_projects_locations_operations_cancel_builder, eventarc_projects_locations_operations_cancel_task,
    eventarc_projects_locations_operations_delete_builder, eventarc_projects_locations_operations_delete_task,
    eventarc_projects_locations_pipelines_create_builder, eventarc_projects_locations_pipelines_create_task,
    eventarc_projects_locations_pipelines_delete_builder, eventarc_projects_locations_pipelines_delete_task,
    eventarc_projects_locations_pipelines_patch_builder, eventarc_projects_locations_pipelines_patch_task,
    eventarc_projects_locations_pipelines_set_iam_policy_builder, eventarc_projects_locations_pipelines_set_iam_policy_task,
    eventarc_projects_locations_pipelines_test_iam_permissions_builder, eventarc_projects_locations_pipelines_test_iam_permissions_task,
    eventarc_projects_locations_triggers_create_builder, eventarc_projects_locations_triggers_create_task,
    eventarc_projects_locations_triggers_delete_builder, eventarc_projects_locations_triggers_delete_task,
    eventarc_projects_locations_triggers_patch_builder, eventarc_projects_locations_triggers_patch_task,
    eventarc_projects_locations_triggers_set_iam_policy_builder, eventarc_projects_locations_triggers_set_iam_policy_task,
    eventarc_projects_locations_triggers_test_iam_permissions_builder, eventarc_projects_locations_triggers_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::eventarc::Empty;
use crate::providers::gcp::clients::eventarc::GoogleChannelConfig;
use crate::providers::gcp::clients::eventarc::GoogleLongrunningOperation;
use crate::providers::gcp::clients::eventarc::Policy;
use crate::providers::gcp::clients::eventarc::TestIamPermissionsResponse;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsUpdateGoogleChannelConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// EventarcProvider with automatic state tracking.
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
/// let provider = EventarcProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct EventarcProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> EventarcProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new EventarcProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Eventarc projects locations update google channel config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChannelConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_update_google_channel_config(
        &self,
        args: &EventarcProjectsLocationsUpdateGoogleChannelConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChannelConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_update_google_channel_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_update_google_channel_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channel connections create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_channel_connections_create(
        &self,
        args: &EventarcProjectsLocationsChannelConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channel_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.channelConnectionId,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channel_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channel connections delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_channel_connections_delete(
        &self,
        args: &EventarcProjectsLocationsChannelConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channel_connections_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channel_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channel connections set iam policy.
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
    pub fn eventarc_projects_locations_channel_connections_set_iam_policy(
        &self,
        args: &EventarcProjectsLocationsChannelConnectionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channel_connections_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channel_connections_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channel connections test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_channel_connections_test_iam_permissions(
        &self,
        args: &EventarcProjectsLocationsChannelConnectionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channel_connections_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channel_connections_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channels create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_channels_create(
        &self,
        args: &EventarcProjectsLocationsChannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channels_create_builder(
            &self.http_client,
            &args.parent,
            &args.channelId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channels delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_channels_delete(
        &self,
        args: &EventarcProjectsLocationsChannelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channels_delete_builder(
            &self.http_client,
            &args.name,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channels patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_channels_patch(
        &self,
        args: &EventarcProjectsLocationsChannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channels set iam policy.
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
    pub fn eventarc_projects_locations_channels_set_iam_policy(
        &self,
        args: &EventarcProjectsLocationsChannelsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channels_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channels_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channels test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_channels_test_iam_permissions(
        &self,
        args: &EventarcProjectsLocationsChannelsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channels_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channels_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations enrollments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_enrollments_create(
        &self,
        args: &EventarcProjectsLocationsEnrollmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_enrollments_create_builder(
            &self.http_client,
            &args.parent,
            &args.enrollmentId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_enrollments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations enrollments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_enrollments_delete(
        &self,
        args: &EventarcProjectsLocationsEnrollmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_enrollments_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_enrollments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations enrollments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_enrollments_patch(
        &self,
        args: &EventarcProjectsLocationsEnrollmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_enrollments_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_enrollments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations enrollments set iam policy.
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
    pub fn eventarc_projects_locations_enrollments_set_iam_policy(
        &self,
        args: &EventarcProjectsLocationsEnrollmentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_enrollments_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_enrollments_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations enrollments test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_enrollments_test_iam_permissions(
        &self,
        args: &EventarcProjectsLocationsEnrollmentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_enrollments_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_enrollments_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations google api sources create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_google_api_sources_create(
        &self,
        args: &EventarcProjectsLocationsGoogleApiSourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_google_api_sources_create_builder(
            &self.http_client,
            &args.parent,
            &args.googleApiSourceId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_google_api_sources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations google api sources delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_google_api_sources_delete(
        &self,
        args: &EventarcProjectsLocationsGoogleApiSourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_google_api_sources_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_google_api_sources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations google api sources patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_google_api_sources_patch(
        &self,
        args: &EventarcProjectsLocationsGoogleApiSourcesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_google_api_sources_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_google_api_sources_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations google api sources set iam policy.
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
    pub fn eventarc_projects_locations_google_api_sources_set_iam_policy(
        &self,
        args: &EventarcProjectsLocationsGoogleApiSourcesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_google_api_sources_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_google_api_sources_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations google api sources test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_google_api_sources_test_iam_permissions(
        &self,
        args: &EventarcProjectsLocationsGoogleApiSourcesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_google_api_sources_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_google_api_sources_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations message buses create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_message_buses_create(
        &self,
        args: &EventarcProjectsLocationsMessageBusesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_create_builder(
            &self.http_client,
            &args.parent,
            &args.messageBusId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations message buses delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_message_buses_delete(
        &self,
        args: &EventarcProjectsLocationsMessageBusesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations message buses patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_message_buses_patch(
        &self,
        args: &EventarcProjectsLocationsMessageBusesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations message buses set iam policy.
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
    pub fn eventarc_projects_locations_message_buses_set_iam_policy(
        &self,
        args: &EventarcProjectsLocationsMessageBusesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations message buses test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_message_buses_test_iam_permissions(
        &self,
        args: &EventarcProjectsLocationsMessageBusesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations operations cancel.
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
    pub fn eventarc_projects_locations_operations_cancel(
        &self,
        args: &EventarcProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations operations delete.
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
    pub fn eventarc_projects_locations_operations_delete(
        &self,
        args: &EventarcProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations pipelines create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_pipelines_create(
        &self,
        args: &EventarcProjectsLocationsPipelinesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_pipelines_create_builder(
            &self.http_client,
            &args.parent,
            &args.pipelineId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_pipelines_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations pipelines delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_pipelines_delete(
        &self,
        args: &EventarcProjectsLocationsPipelinesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_pipelines_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_pipelines_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations pipelines patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_pipelines_patch(
        &self,
        args: &EventarcProjectsLocationsPipelinesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_pipelines_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_pipelines_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations pipelines set iam policy.
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
    pub fn eventarc_projects_locations_pipelines_set_iam_policy(
        &self,
        args: &EventarcProjectsLocationsPipelinesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_pipelines_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_pipelines_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations pipelines test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_pipelines_test_iam_permissions(
        &self,
        args: &EventarcProjectsLocationsPipelinesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_pipelines_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_pipelines_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations triggers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_triggers_create(
        &self,
        args: &EventarcProjectsLocationsTriggersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_triggers_create_builder(
            &self.http_client,
            &args.parent,
            &args.triggerId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_triggers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations triggers delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_triggers_delete(
        &self,
        args: &EventarcProjectsLocationsTriggersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_triggers_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_triggers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations triggers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_triggers_patch(
        &self,
        args: &EventarcProjectsLocationsTriggersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_triggers_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_triggers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations triggers set iam policy.
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
    pub fn eventarc_projects_locations_triggers_set_iam_policy(
        &self,
        args: &EventarcProjectsLocationsTriggersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_triggers_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_triggers_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations triggers test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn eventarc_projects_locations_triggers_test_iam_permissions(
        &self,
        args: &EventarcProjectsLocationsTriggersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_triggers_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_triggers_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
