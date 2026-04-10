//! PubsubliteProvider - State-aware pubsublite API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       pubsublite API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::pubsublite::{
    pubsublite_admin_projects_locations_operations_cancel_builder, pubsublite_admin_projects_locations_operations_cancel_task,
    pubsublite_admin_projects_locations_operations_delete_builder, pubsublite_admin_projects_locations_operations_delete_task,
    pubsublite_admin_projects_locations_operations_get_builder, pubsublite_admin_projects_locations_operations_get_task,
    pubsublite_admin_projects_locations_operations_list_builder, pubsublite_admin_projects_locations_operations_list_task,
    pubsublite_admin_projects_locations_reservations_create_builder, pubsublite_admin_projects_locations_reservations_create_task,
    pubsublite_admin_projects_locations_reservations_delete_builder, pubsublite_admin_projects_locations_reservations_delete_task,
    pubsublite_admin_projects_locations_reservations_get_builder, pubsublite_admin_projects_locations_reservations_get_task,
    pubsublite_admin_projects_locations_reservations_list_builder, pubsublite_admin_projects_locations_reservations_list_task,
    pubsublite_admin_projects_locations_reservations_patch_builder, pubsublite_admin_projects_locations_reservations_patch_task,
    pubsublite_admin_projects_locations_reservations_topics_list_builder, pubsublite_admin_projects_locations_reservations_topics_list_task,
    pubsublite_admin_projects_locations_subscriptions_create_builder, pubsublite_admin_projects_locations_subscriptions_create_task,
    pubsublite_admin_projects_locations_subscriptions_delete_builder, pubsublite_admin_projects_locations_subscriptions_delete_task,
    pubsublite_admin_projects_locations_subscriptions_get_builder, pubsublite_admin_projects_locations_subscriptions_get_task,
    pubsublite_admin_projects_locations_subscriptions_list_builder, pubsublite_admin_projects_locations_subscriptions_list_task,
    pubsublite_admin_projects_locations_subscriptions_patch_builder, pubsublite_admin_projects_locations_subscriptions_patch_task,
    pubsublite_admin_projects_locations_subscriptions_seek_builder, pubsublite_admin_projects_locations_subscriptions_seek_task,
    pubsublite_admin_projects_locations_topics_create_builder, pubsublite_admin_projects_locations_topics_create_task,
    pubsublite_admin_projects_locations_topics_delete_builder, pubsublite_admin_projects_locations_topics_delete_task,
    pubsublite_admin_projects_locations_topics_get_builder, pubsublite_admin_projects_locations_topics_get_task,
    pubsublite_admin_projects_locations_topics_get_partitions_builder, pubsublite_admin_projects_locations_topics_get_partitions_task,
    pubsublite_admin_projects_locations_topics_list_builder, pubsublite_admin_projects_locations_topics_list_task,
    pubsublite_admin_projects_locations_topics_patch_builder, pubsublite_admin_projects_locations_topics_patch_task,
    pubsublite_admin_projects_locations_topics_subscriptions_list_builder, pubsublite_admin_projects_locations_topics_subscriptions_list_task,
    pubsublite_cursor_projects_locations_subscriptions_commit_cursor_builder, pubsublite_cursor_projects_locations_subscriptions_commit_cursor_task,
    pubsublite_cursor_projects_locations_subscriptions_cursors_list_builder, pubsublite_cursor_projects_locations_subscriptions_cursors_list_task,
    pubsublite_topic_stats_projects_locations_topics_compute_head_cursor_builder, pubsublite_topic_stats_projects_locations_topics_compute_head_cursor_task,
    pubsublite_topic_stats_projects_locations_topics_compute_message_stats_builder, pubsublite_topic_stats_projects_locations_topics_compute_message_stats_task,
    pubsublite_topic_stats_projects_locations_topics_compute_time_cursor_builder, pubsublite_topic_stats_projects_locations_topics_compute_time_cursor_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::pubsublite::CommitCursorResponse;
use crate::providers::gcp::clients::pubsublite::ComputeHeadCursorResponse;
use crate::providers::gcp::clients::pubsublite::ComputeMessageStatsResponse;
use crate::providers::gcp::clients::pubsublite::ComputeTimeCursorResponse;
use crate::providers::gcp::clients::pubsublite::Empty;
use crate::providers::gcp::clients::pubsublite::ListOperationsResponse;
use crate::providers::gcp::clients::pubsublite::ListPartitionCursorsResponse;
use crate::providers::gcp::clients::pubsublite::ListReservationTopicsResponse;
use crate::providers::gcp::clients::pubsublite::ListReservationsResponse;
use crate::providers::gcp::clients::pubsublite::ListSubscriptionsResponse;
use crate::providers::gcp::clients::pubsublite::ListTopicSubscriptionsResponse;
use crate::providers::gcp::clients::pubsublite::ListTopicsResponse;
use crate::providers::gcp::clients::pubsublite::Operation;
use crate::providers::gcp::clients::pubsublite::Reservation;
use crate::providers::gcp::clients::pubsublite::Subscription;
use crate::providers::gcp::clients::pubsublite::Topic;
use crate::providers::gcp::clients::pubsublite::TopicPartitions;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsReservationsCreateArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsReservationsDeleteArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsReservationsGetArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsReservationsListArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsReservationsPatchArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsReservationsTopicsListArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsSubscriptionsCreateArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsSubscriptionsDeleteArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsSubscriptionsGetArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsSubscriptionsListArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsSubscriptionsPatchArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsSubscriptionsSeekArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsTopicsCreateArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsTopicsDeleteArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsTopicsGetArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsTopicsGetPartitionsArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsTopicsListArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsTopicsPatchArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteAdminProjectsLocationsTopicsSubscriptionsListArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteCursorProjectsLocationsSubscriptionsCommitCursorArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteCursorProjectsLocationsSubscriptionsCursorsListArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteTopicStatsProjectsLocationsTopicsComputeHeadCursorArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteTopicStatsProjectsLocationsTopicsComputeMessageStatsArgs;
use crate::providers::gcp::clients::pubsublite::PubsubliteTopicStatsProjectsLocationsTopicsComputeTimeCursorArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PubsubliteProvider with automatic state tracking.
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
/// let provider = PubsubliteProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PubsubliteProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PubsubliteProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PubsubliteProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Pubsublite admin projects locations operations cancel.
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
    pub fn pubsublite_admin_projects_locations_operations_cancel(
        &self,
        args: &PubsubliteAdminProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations operations delete.
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
    pub fn pubsublite_admin_projects_locations_operations_delete(
        &self,
        args: &PubsubliteAdminProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations operations get.
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
    pub fn pubsublite_admin_projects_locations_operations_get(
        &self,
        args: &PubsubliteAdminProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations operations list.
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
    pub fn pubsublite_admin_projects_locations_operations_list(
        &self,
        args: &PubsubliteAdminProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations reservations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Reservation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsublite_admin_projects_locations_reservations_create(
        &self,
        args: &PubsubliteAdminProjectsLocationsReservationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_reservations_create_builder(
            &self.http_client,
            &args.parent,
            &args.reservationId,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_reservations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations reservations delete.
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
    pub fn pubsublite_admin_projects_locations_reservations_delete(
        &self,
        args: &PubsubliteAdminProjectsLocationsReservationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_reservations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_reservations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations reservations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Reservation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_reservations_get(
        &self,
        args: &PubsubliteAdminProjectsLocationsReservationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_reservations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_reservations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations reservations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReservationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_reservations_list(
        &self,
        args: &PubsubliteAdminProjectsLocationsReservationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReservationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_reservations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_reservations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations reservations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Reservation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsublite_admin_projects_locations_reservations_patch(
        &self,
        args: &PubsubliteAdminProjectsLocationsReservationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_reservations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_reservations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations reservations topics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReservationTopicsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_reservations_topics_list(
        &self,
        args: &PubsubliteAdminProjectsLocationsReservationsTopicsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReservationTopicsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_reservations_topics_list_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_reservations_topics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations subscriptions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsublite_admin_projects_locations_subscriptions_create(
        &self,
        args: &PubsubliteAdminProjectsLocationsSubscriptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_subscriptions_create_builder(
            &self.http_client,
            &args.parent,
            &args.skipBacklog,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_subscriptions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations subscriptions delete.
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
    pub fn pubsublite_admin_projects_locations_subscriptions_delete(
        &self,
        args: &PubsubliteAdminProjectsLocationsSubscriptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_subscriptions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_subscriptions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations subscriptions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_subscriptions_get(
        &self,
        args: &PubsubliteAdminProjectsLocationsSubscriptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_subscriptions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_subscriptions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations subscriptions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSubscriptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_subscriptions_list(
        &self,
        args: &PubsubliteAdminProjectsLocationsSubscriptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSubscriptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_subscriptions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_subscriptions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations subscriptions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsublite_admin_projects_locations_subscriptions_patch(
        &self,
        args: &PubsubliteAdminProjectsLocationsSubscriptionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_subscriptions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_subscriptions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations subscriptions seek.
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
    pub fn pubsublite_admin_projects_locations_subscriptions_seek(
        &self,
        args: &PubsubliteAdminProjectsLocationsSubscriptionsSeekArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_subscriptions_seek_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_subscriptions_seek_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations topics create.
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
    pub fn pubsublite_admin_projects_locations_topics_create(
        &self,
        args: &PubsubliteAdminProjectsLocationsTopicsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_topics_create_builder(
            &self.http_client,
            &args.parent,
            &args.topicId,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_topics_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations topics delete.
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
    pub fn pubsublite_admin_projects_locations_topics_delete(
        &self,
        args: &PubsubliteAdminProjectsLocationsTopicsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_topics_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_topics_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations topics get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_topics_get(
        &self,
        args: &PubsubliteAdminProjectsLocationsTopicsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_topics_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_topics_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations topics get partitions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TopicPartitions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_topics_get_partitions(
        &self,
        args: &PubsubliteAdminProjectsLocationsTopicsGetPartitionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TopicPartitions, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_topics_get_partitions_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_topics_get_partitions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations topics list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTopicsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_topics_list(
        &self,
        args: &PubsubliteAdminProjectsLocationsTopicsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTopicsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_topics_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_topics_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations topics patch.
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
    pub fn pubsublite_admin_projects_locations_topics_patch(
        &self,
        args: &PubsubliteAdminProjectsLocationsTopicsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_topics_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_topics_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite admin projects locations topics subscriptions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTopicSubscriptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_admin_projects_locations_topics_subscriptions_list(
        &self,
        args: &PubsubliteAdminProjectsLocationsTopicsSubscriptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTopicSubscriptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_admin_projects_locations_topics_subscriptions_list_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_admin_projects_locations_topics_subscriptions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite cursor projects locations subscriptions commit cursor.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommitCursorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsublite_cursor_projects_locations_subscriptions_commit_cursor(
        &self,
        args: &PubsubliteCursorProjectsLocationsSubscriptionsCommitCursorArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommitCursorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_cursor_projects_locations_subscriptions_commit_cursor_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_cursor_projects_locations_subscriptions_commit_cursor_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite cursor projects locations subscriptions cursors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPartitionCursorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn pubsublite_cursor_projects_locations_subscriptions_cursors_list(
        &self,
        args: &PubsubliteCursorProjectsLocationsSubscriptionsCursorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPartitionCursorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_cursor_projects_locations_subscriptions_cursors_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_cursor_projects_locations_subscriptions_cursors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite topic stats projects locations topics compute head cursor.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeHeadCursorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsublite_topic_stats_projects_locations_topics_compute_head_cursor(
        &self,
        args: &PubsubliteTopicStatsProjectsLocationsTopicsComputeHeadCursorArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeHeadCursorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_topic_stats_projects_locations_topics_compute_head_cursor_builder(
            &self.http_client,
            &args.topic,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_topic_stats_projects_locations_topics_compute_head_cursor_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite topic stats projects locations topics compute message stats.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeMessageStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsublite_topic_stats_projects_locations_topics_compute_message_stats(
        &self,
        args: &PubsubliteTopicStatsProjectsLocationsTopicsComputeMessageStatsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeMessageStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_topic_stats_projects_locations_topics_compute_message_stats_builder(
            &self.http_client,
            &args.topic,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_topic_stats_projects_locations_topics_compute_message_stats_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsublite topic stats projects locations topics compute time cursor.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeTimeCursorResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsublite_topic_stats_projects_locations_topics_compute_time_cursor(
        &self,
        args: &PubsubliteTopicStatsProjectsLocationsTopicsComputeTimeCursorArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeTimeCursorResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsublite_topic_stats_projects_locations_topics_compute_time_cursor_builder(
            &self.http_client,
            &args.topic,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsublite_topic_stats_projects_locations_topics_compute_time_cursor_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
