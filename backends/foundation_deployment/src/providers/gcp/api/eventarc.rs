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
    eventarc_projects_locations_get_builder, eventarc_projects_locations_get_task,
    eventarc_projects_locations_get_google_channel_config_builder, eventarc_projects_locations_get_google_channel_config_task,
    eventarc_projects_locations_list_builder, eventarc_projects_locations_list_task,
    eventarc_projects_locations_update_google_channel_config_builder, eventarc_projects_locations_update_google_channel_config_task,
    eventarc_projects_locations_channel_connections_create_builder, eventarc_projects_locations_channel_connections_create_task,
    eventarc_projects_locations_channel_connections_delete_builder, eventarc_projects_locations_channel_connections_delete_task,
    eventarc_projects_locations_channel_connections_get_builder, eventarc_projects_locations_channel_connections_get_task,
    eventarc_projects_locations_channel_connections_get_iam_policy_builder, eventarc_projects_locations_channel_connections_get_iam_policy_task,
    eventarc_projects_locations_channel_connections_list_builder, eventarc_projects_locations_channel_connections_list_task,
    eventarc_projects_locations_channel_connections_set_iam_policy_builder, eventarc_projects_locations_channel_connections_set_iam_policy_task,
    eventarc_projects_locations_channel_connections_test_iam_permissions_builder, eventarc_projects_locations_channel_connections_test_iam_permissions_task,
    eventarc_projects_locations_channels_create_builder, eventarc_projects_locations_channels_create_task,
    eventarc_projects_locations_channels_delete_builder, eventarc_projects_locations_channels_delete_task,
    eventarc_projects_locations_channels_get_builder, eventarc_projects_locations_channels_get_task,
    eventarc_projects_locations_channels_get_iam_policy_builder, eventarc_projects_locations_channels_get_iam_policy_task,
    eventarc_projects_locations_channels_list_builder, eventarc_projects_locations_channels_list_task,
    eventarc_projects_locations_channels_patch_builder, eventarc_projects_locations_channels_patch_task,
    eventarc_projects_locations_channels_set_iam_policy_builder, eventarc_projects_locations_channels_set_iam_policy_task,
    eventarc_projects_locations_channels_test_iam_permissions_builder, eventarc_projects_locations_channels_test_iam_permissions_task,
    eventarc_projects_locations_enrollments_create_builder, eventarc_projects_locations_enrollments_create_task,
    eventarc_projects_locations_enrollments_delete_builder, eventarc_projects_locations_enrollments_delete_task,
    eventarc_projects_locations_enrollments_get_builder, eventarc_projects_locations_enrollments_get_task,
    eventarc_projects_locations_enrollments_get_iam_policy_builder, eventarc_projects_locations_enrollments_get_iam_policy_task,
    eventarc_projects_locations_enrollments_list_builder, eventarc_projects_locations_enrollments_list_task,
    eventarc_projects_locations_enrollments_patch_builder, eventarc_projects_locations_enrollments_patch_task,
    eventarc_projects_locations_enrollments_set_iam_policy_builder, eventarc_projects_locations_enrollments_set_iam_policy_task,
    eventarc_projects_locations_enrollments_test_iam_permissions_builder, eventarc_projects_locations_enrollments_test_iam_permissions_task,
    eventarc_projects_locations_google_api_sources_create_builder, eventarc_projects_locations_google_api_sources_create_task,
    eventarc_projects_locations_google_api_sources_delete_builder, eventarc_projects_locations_google_api_sources_delete_task,
    eventarc_projects_locations_google_api_sources_get_builder, eventarc_projects_locations_google_api_sources_get_task,
    eventarc_projects_locations_google_api_sources_get_iam_policy_builder, eventarc_projects_locations_google_api_sources_get_iam_policy_task,
    eventarc_projects_locations_google_api_sources_list_builder, eventarc_projects_locations_google_api_sources_list_task,
    eventarc_projects_locations_google_api_sources_patch_builder, eventarc_projects_locations_google_api_sources_patch_task,
    eventarc_projects_locations_google_api_sources_set_iam_policy_builder, eventarc_projects_locations_google_api_sources_set_iam_policy_task,
    eventarc_projects_locations_google_api_sources_test_iam_permissions_builder, eventarc_projects_locations_google_api_sources_test_iam_permissions_task,
    eventarc_projects_locations_message_buses_create_builder, eventarc_projects_locations_message_buses_create_task,
    eventarc_projects_locations_message_buses_delete_builder, eventarc_projects_locations_message_buses_delete_task,
    eventarc_projects_locations_message_buses_get_builder, eventarc_projects_locations_message_buses_get_task,
    eventarc_projects_locations_message_buses_get_iam_policy_builder, eventarc_projects_locations_message_buses_get_iam_policy_task,
    eventarc_projects_locations_message_buses_list_builder, eventarc_projects_locations_message_buses_list_task,
    eventarc_projects_locations_message_buses_list_enrollments_builder, eventarc_projects_locations_message_buses_list_enrollments_task,
    eventarc_projects_locations_message_buses_patch_builder, eventarc_projects_locations_message_buses_patch_task,
    eventarc_projects_locations_message_buses_set_iam_policy_builder, eventarc_projects_locations_message_buses_set_iam_policy_task,
    eventarc_projects_locations_message_buses_test_iam_permissions_builder, eventarc_projects_locations_message_buses_test_iam_permissions_task,
    eventarc_projects_locations_operations_cancel_builder, eventarc_projects_locations_operations_cancel_task,
    eventarc_projects_locations_operations_delete_builder, eventarc_projects_locations_operations_delete_task,
    eventarc_projects_locations_operations_get_builder, eventarc_projects_locations_operations_get_task,
    eventarc_projects_locations_operations_list_builder, eventarc_projects_locations_operations_list_task,
    eventarc_projects_locations_pipelines_create_builder, eventarc_projects_locations_pipelines_create_task,
    eventarc_projects_locations_pipelines_delete_builder, eventarc_projects_locations_pipelines_delete_task,
    eventarc_projects_locations_pipelines_get_builder, eventarc_projects_locations_pipelines_get_task,
    eventarc_projects_locations_pipelines_get_iam_policy_builder, eventarc_projects_locations_pipelines_get_iam_policy_task,
    eventarc_projects_locations_pipelines_list_builder, eventarc_projects_locations_pipelines_list_task,
    eventarc_projects_locations_pipelines_patch_builder, eventarc_projects_locations_pipelines_patch_task,
    eventarc_projects_locations_pipelines_set_iam_policy_builder, eventarc_projects_locations_pipelines_set_iam_policy_task,
    eventarc_projects_locations_pipelines_test_iam_permissions_builder, eventarc_projects_locations_pipelines_test_iam_permissions_task,
    eventarc_projects_locations_providers_get_builder, eventarc_projects_locations_providers_get_task,
    eventarc_projects_locations_providers_list_builder, eventarc_projects_locations_providers_list_task,
    eventarc_projects_locations_triggers_create_builder, eventarc_projects_locations_triggers_create_task,
    eventarc_projects_locations_triggers_delete_builder, eventarc_projects_locations_triggers_delete_task,
    eventarc_projects_locations_triggers_get_builder, eventarc_projects_locations_triggers_get_task,
    eventarc_projects_locations_triggers_get_iam_policy_builder, eventarc_projects_locations_triggers_get_iam_policy_task,
    eventarc_projects_locations_triggers_list_builder, eventarc_projects_locations_triggers_list_task,
    eventarc_projects_locations_triggers_patch_builder, eventarc_projects_locations_triggers_patch_task,
    eventarc_projects_locations_triggers_set_iam_policy_builder, eventarc_projects_locations_triggers_set_iam_policy_task,
    eventarc_projects_locations_triggers_test_iam_permissions_builder, eventarc_projects_locations_triggers_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::eventarc::Channel;
use crate::providers::gcp::clients::eventarc::ChannelConnection;
use crate::providers::gcp::clients::eventarc::Empty;
use crate::providers::gcp::clients::eventarc::Enrollment;
use crate::providers::gcp::clients::eventarc::GoogleApiSource;
use crate::providers::gcp::clients::eventarc::GoogleChannelConfig;
use crate::providers::gcp::clients::eventarc::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::eventarc::GoogleLongrunningOperation;
use crate::providers::gcp::clients::eventarc::ListChannelConnectionsResponse;
use crate::providers::gcp::clients::eventarc::ListChannelsResponse;
use crate::providers::gcp::clients::eventarc::ListEnrollmentsResponse;
use crate::providers::gcp::clients::eventarc::ListGoogleApiSourcesResponse;
use crate::providers::gcp::clients::eventarc::ListLocationsResponse;
use crate::providers::gcp::clients::eventarc::ListMessageBusEnrollmentsResponse;
use crate::providers::gcp::clients::eventarc::ListMessageBusesResponse;
use crate::providers::gcp::clients::eventarc::ListPipelinesResponse;
use crate::providers::gcp::clients::eventarc::ListProvidersResponse;
use crate::providers::gcp::clients::eventarc::ListTriggersResponse;
use crate::providers::gcp::clients::eventarc::Location;
use crate::providers::gcp::clients::eventarc::MessageBus;
use crate::providers::gcp::clients::eventarc::Pipeline;
use crate::providers::gcp::clients::eventarc::Policy;
use crate::providers::gcp::clients::eventarc::Provider;
use crate::providers::gcp::clients::eventarc::TestIamPermissionsResponse;
use crate::providers::gcp::clients::eventarc::Trigger;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsGetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelConnectionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsGetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsChannelsTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsGetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsEnrollmentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGetGoogleChannelConfigArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesGetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsGoogleApiSourcesTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesGetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesListEnrollmentsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsMessageBusesTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesGetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsPipelinesTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsProvidersGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsProvidersListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersCreateArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersDeleteArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersGetArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersGetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersListArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersPatchArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersSetIamPolicyArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsTriggersTestIamPermissionsArgs;
use crate::providers::gcp::clients::eventarc::EventarcProjectsLocationsUpdateGoogleChannelConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// EventarcProvider with automatic state tracking.
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
/// let provider = EventarcProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct EventarcProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> EventarcProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new EventarcProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new EventarcProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Eventarc projects locations get.
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
    pub fn eventarc_projects_locations_get(
        &self,
        args: &EventarcProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations get google channel config.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_get_google_channel_config(
        &self,
        args: &EventarcProjectsLocationsGetGoogleChannelConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChannelConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_get_google_channel_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_get_google_channel_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations list.
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
    pub fn eventarc_projects_locations_list(
        &self,
        args: &EventarcProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Eventarc projects locations channel connections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ChannelConnection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_channel_connections_get(
        &self,
        args: &EventarcProjectsLocationsChannelConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ChannelConnection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channel_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channel_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channel connections get iam policy.
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
    pub fn eventarc_projects_locations_channel_connections_get_iam_policy(
        &self,
        args: &EventarcProjectsLocationsChannelConnectionsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channel_connections_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channel_connections_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channel connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListChannelConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_channel_connections_list(
        &self,
        args: &EventarcProjectsLocationsChannelConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListChannelConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channel_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channel_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Eventarc projects locations channels get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Channel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_channels_get(
        &self,
        args: &EventarcProjectsLocationsChannelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Channel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channels_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channels get iam policy.
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
    pub fn eventarc_projects_locations_channels_get_iam_policy(
        &self,
        args: &EventarcProjectsLocationsChannelsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channels_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channels_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations channels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_channels_list(
        &self,
        args: &EventarcProjectsLocationsChannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_channels_list_builder(
            &self.http_client,
            &args.parent,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_channels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Eventarc projects locations enrollments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Enrollment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_enrollments_get(
        &self,
        args: &EventarcProjectsLocationsEnrollmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Enrollment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_enrollments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_enrollments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations enrollments get iam policy.
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
    pub fn eventarc_projects_locations_enrollments_get_iam_policy(
        &self,
        args: &EventarcProjectsLocationsEnrollmentsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_enrollments_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_enrollments_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations enrollments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEnrollmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_enrollments_list(
        &self,
        args: &EventarcProjectsLocationsEnrollmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEnrollmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_enrollments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_enrollments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Eventarc projects locations google api sources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleApiSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_google_api_sources_get(
        &self,
        args: &EventarcProjectsLocationsGoogleApiSourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleApiSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_google_api_sources_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_google_api_sources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations google api sources get iam policy.
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
    pub fn eventarc_projects_locations_google_api_sources_get_iam_policy(
        &self,
        args: &EventarcProjectsLocationsGoogleApiSourcesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_google_api_sources_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_google_api_sources_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations google api sources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGoogleApiSourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_google_api_sources_list(
        &self,
        args: &EventarcProjectsLocationsGoogleApiSourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGoogleApiSourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_google_api_sources_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_google_api_sources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Eventarc projects locations message buses get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MessageBus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_message_buses_get(
        &self,
        args: &EventarcProjectsLocationsMessageBusesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MessageBus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations message buses get iam policy.
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
    pub fn eventarc_projects_locations_message_buses_get_iam_policy(
        &self,
        args: &EventarcProjectsLocationsMessageBusesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations message buses list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMessageBusesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_message_buses_list(
        &self,
        args: &EventarcProjectsLocationsMessageBusesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMessageBusesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations message buses list enrollments.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMessageBusEnrollmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_message_buses_list_enrollments(
        &self,
        args: &EventarcProjectsLocationsMessageBusesListEnrollmentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMessageBusEnrollmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_message_buses_list_enrollments_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_message_buses_list_enrollments_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Eventarc projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_operations_get(
        &self,
        args: &EventarcProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_operations_list(
        &self,
        args: &EventarcProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Eventarc projects locations pipelines get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Pipeline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_pipelines_get(
        &self,
        args: &EventarcProjectsLocationsPipelinesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Pipeline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_pipelines_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_pipelines_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations pipelines get iam policy.
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
    pub fn eventarc_projects_locations_pipelines_get_iam_policy(
        &self,
        args: &EventarcProjectsLocationsPipelinesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_pipelines_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_pipelines_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations pipelines list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPipelinesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_pipelines_list(
        &self,
        args: &EventarcProjectsLocationsPipelinesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPipelinesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_pipelines_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_pipelines_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations providers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Provider result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_providers_get(
        &self,
        args: &EventarcProjectsLocationsProvidersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Provider, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_providers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_providers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations providers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProvidersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_providers_list(
        &self,
        args: &EventarcProjectsLocationsProvidersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProvidersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_providers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_providers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Eventarc projects locations triggers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Trigger result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_triggers_get(
        &self,
        args: &EventarcProjectsLocationsTriggersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Trigger, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_triggers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_triggers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations triggers get iam policy.
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
    pub fn eventarc_projects_locations_triggers_get_iam_policy(
        &self,
        args: &EventarcProjectsLocationsTriggersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_triggers_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_triggers_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Eventarc projects locations triggers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTriggersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn eventarc_projects_locations_triggers_list(
        &self,
        args: &EventarcProjectsLocationsTriggersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTriggersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = eventarc_projects_locations_triggers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = eventarc_projects_locations_triggers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
