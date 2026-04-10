//! BigqueryreservationProvider - State-aware bigqueryreservation API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       bigqueryreservation API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::bigqueryreservation::{
    bigqueryreservation_projects_locations_get_bi_reservation_builder, bigqueryreservation_projects_locations_get_bi_reservation_task,
    bigqueryreservation_projects_locations_search_all_assignments_builder, bigqueryreservation_projects_locations_search_all_assignments_task,
    bigqueryreservation_projects_locations_search_assignments_builder, bigqueryreservation_projects_locations_search_assignments_task,
    bigqueryreservation_projects_locations_update_bi_reservation_builder, bigqueryreservation_projects_locations_update_bi_reservation_task,
    bigqueryreservation_projects_locations_capacity_commitments_create_builder, bigqueryreservation_projects_locations_capacity_commitments_create_task,
    bigqueryreservation_projects_locations_capacity_commitments_delete_builder, bigqueryreservation_projects_locations_capacity_commitments_delete_task,
    bigqueryreservation_projects_locations_capacity_commitments_get_builder, bigqueryreservation_projects_locations_capacity_commitments_get_task,
    bigqueryreservation_projects_locations_capacity_commitments_list_builder, bigqueryreservation_projects_locations_capacity_commitments_list_task,
    bigqueryreservation_projects_locations_capacity_commitments_merge_builder, bigqueryreservation_projects_locations_capacity_commitments_merge_task,
    bigqueryreservation_projects_locations_capacity_commitments_patch_builder, bigqueryreservation_projects_locations_capacity_commitments_patch_task,
    bigqueryreservation_projects_locations_capacity_commitments_split_builder, bigqueryreservation_projects_locations_capacity_commitments_split_task,
    bigqueryreservation_projects_locations_reservation_groups_create_builder, bigqueryreservation_projects_locations_reservation_groups_create_task,
    bigqueryreservation_projects_locations_reservation_groups_delete_builder, bigqueryreservation_projects_locations_reservation_groups_delete_task,
    bigqueryreservation_projects_locations_reservation_groups_get_builder, bigqueryreservation_projects_locations_reservation_groups_get_task,
    bigqueryreservation_projects_locations_reservation_groups_list_builder, bigqueryreservation_projects_locations_reservation_groups_list_task,
    bigqueryreservation_projects_locations_reservations_create_builder, bigqueryreservation_projects_locations_reservations_create_task,
    bigqueryreservation_projects_locations_reservations_delete_builder, bigqueryreservation_projects_locations_reservations_delete_task,
    bigqueryreservation_projects_locations_reservations_failover_reservation_builder, bigqueryreservation_projects_locations_reservations_failover_reservation_task,
    bigqueryreservation_projects_locations_reservations_get_builder, bigqueryreservation_projects_locations_reservations_get_task,
    bigqueryreservation_projects_locations_reservations_get_iam_policy_builder, bigqueryreservation_projects_locations_reservations_get_iam_policy_task,
    bigqueryreservation_projects_locations_reservations_list_builder, bigqueryreservation_projects_locations_reservations_list_task,
    bigqueryreservation_projects_locations_reservations_patch_builder, bigqueryreservation_projects_locations_reservations_patch_task,
    bigqueryreservation_projects_locations_reservations_set_iam_policy_builder, bigqueryreservation_projects_locations_reservations_set_iam_policy_task,
    bigqueryreservation_projects_locations_reservations_test_iam_permissions_builder, bigqueryreservation_projects_locations_reservations_test_iam_permissions_task,
    bigqueryreservation_projects_locations_reservations_assignments_create_builder, bigqueryreservation_projects_locations_reservations_assignments_create_task,
    bigqueryreservation_projects_locations_reservations_assignments_delete_builder, bigqueryreservation_projects_locations_reservations_assignments_delete_task,
    bigqueryreservation_projects_locations_reservations_assignments_get_iam_policy_builder, bigqueryreservation_projects_locations_reservations_assignments_get_iam_policy_task,
    bigqueryreservation_projects_locations_reservations_assignments_list_builder, bigqueryreservation_projects_locations_reservations_assignments_list_task,
    bigqueryreservation_projects_locations_reservations_assignments_move_builder, bigqueryreservation_projects_locations_reservations_assignments_move_task,
    bigqueryreservation_projects_locations_reservations_assignments_patch_builder, bigqueryreservation_projects_locations_reservations_assignments_patch_task,
    bigqueryreservation_projects_locations_reservations_assignments_set_iam_policy_builder, bigqueryreservation_projects_locations_reservations_assignments_set_iam_policy_task,
    bigqueryreservation_projects_locations_reservations_assignments_test_iam_permissions_builder, bigqueryreservation_projects_locations_reservations_assignments_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::bigqueryreservation::Assignment;
use crate::providers::gcp::clients::bigqueryreservation::BiReservation;
use crate::providers::gcp::clients::bigqueryreservation::CapacityCommitment;
use crate::providers::gcp::clients::bigqueryreservation::Empty;
use crate::providers::gcp::clients::bigqueryreservation::ListAssignmentsResponse;
use crate::providers::gcp::clients::bigqueryreservation::ListCapacityCommitmentsResponse;
use crate::providers::gcp::clients::bigqueryreservation::ListReservationGroupsResponse;
use crate::providers::gcp::clients::bigqueryreservation::ListReservationsResponse;
use crate::providers::gcp::clients::bigqueryreservation::Policy;
use crate::providers::gcp::clients::bigqueryreservation::Reservation;
use crate::providers::gcp::clients::bigqueryreservation::ReservationGroup;
use crate::providers::gcp::clients::bigqueryreservation::SearchAllAssignmentsResponse;
use crate::providers::gcp::clients::bigqueryreservation::SearchAssignmentsResponse;
use crate::providers::gcp::clients::bigqueryreservation::SplitCapacityCommitmentResponse;
use crate::providers::gcp::clients::bigqueryreservation::TestIamPermissionsResponse;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsCapacityCommitmentsCreateArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsCapacityCommitmentsDeleteArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsCapacityCommitmentsGetArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsCapacityCommitmentsListArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsCapacityCommitmentsMergeArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsCapacityCommitmentsPatchArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsCapacityCommitmentsSplitArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsGetBiReservationArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationGroupsCreateArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationGroupsDeleteArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationGroupsGetArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationGroupsListArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsAssignmentsCreateArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsAssignmentsDeleteArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsAssignmentsGetIamPolicyArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsAssignmentsListArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsAssignmentsMoveArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsAssignmentsPatchArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsAssignmentsSetIamPolicyArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsAssignmentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsCreateArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsDeleteArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsFailoverReservationArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsGetArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsGetIamPolicyArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsListArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsPatchArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsSetIamPolicyArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsReservationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsSearchAllAssignmentsArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsSearchAssignmentsArgs;
use crate::providers::gcp::clients::bigqueryreservation::BigqueryreservationProjectsLocationsUpdateBiReservationArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BigqueryreservationProvider with automatic state tracking.
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
/// let provider = BigqueryreservationProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BigqueryreservationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BigqueryreservationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BigqueryreservationProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Bigqueryreservation projects locations get bi reservation.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BiReservation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_get_bi_reservation(
        &self,
        args: &BigqueryreservationProjectsLocationsGetBiReservationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BiReservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_get_bi_reservation_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_get_bi_reservation_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations search all assignments.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchAllAssignmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_search_all_assignments(
        &self,
        args: &BigqueryreservationProjectsLocationsSearchAllAssignmentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchAllAssignmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_search_all_assignments_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_search_all_assignments_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations search assignments.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchAssignmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_search_assignments(
        &self,
        args: &BigqueryreservationProjectsLocationsSearchAssignmentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchAssignmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_search_assignments_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_search_assignments_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations update bi reservation.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BiReservation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_update_bi_reservation(
        &self,
        args: &BigqueryreservationProjectsLocationsUpdateBiReservationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BiReservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_update_bi_reservation_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_update_bi_reservation_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations capacity commitments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CapacityCommitment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigqueryreservation_projects_locations_capacity_commitments_create(
        &self,
        args: &BigqueryreservationProjectsLocationsCapacityCommitmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CapacityCommitment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_capacity_commitments_create_builder(
            &self.http_client,
            &args.parent,
            &args.capacityCommitmentId,
            &args.enforceSingleAdminProjectPerOrg,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_capacity_commitments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations capacity commitments delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_capacity_commitments_delete(
        &self,
        args: &BigqueryreservationProjectsLocationsCapacityCommitmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_capacity_commitments_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_capacity_commitments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations capacity commitments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CapacityCommitment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_capacity_commitments_get(
        &self,
        args: &BigqueryreservationProjectsLocationsCapacityCommitmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CapacityCommitment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_capacity_commitments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_capacity_commitments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations capacity commitments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCapacityCommitmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_capacity_commitments_list(
        &self,
        args: &BigqueryreservationProjectsLocationsCapacityCommitmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCapacityCommitmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_capacity_commitments_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_capacity_commitments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations capacity commitments merge.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CapacityCommitment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_capacity_commitments_merge(
        &self,
        args: &BigqueryreservationProjectsLocationsCapacityCommitmentsMergeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CapacityCommitment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_capacity_commitments_merge_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_capacity_commitments_merge_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations capacity commitments patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CapacityCommitment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_capacity_commitments_patch(
        &self,
        args: &BigqueryreservationProjectsLocationsCapacityCommitmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CapacityCommitment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_capacity_commitments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_capacity_commitments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations capacity commitments split.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SplitCapacityCommitmentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_capacity_commitments_split(
        &self,
        args: &BigqueryreservationProjectsLocationsCapacityCommitmentsSplitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SplitCapacityCommitmentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_capacity_commitments_split_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_capacity_commitments_split_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservation groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReservationGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigqueryreservation_projects_locations_reservation_groups_create(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReservationGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservation_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.reservationGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservation_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservation groups delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_reservation_groups_delete(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservation_groups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservation_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservation groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReservationGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_reservation_groups_get(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReservationGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservation_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservation_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservation groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReservationGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_reservation_groups_list(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReservationGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservation_groups_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservation_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations create.
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
    pub fn bigqueryreservation_projects_locations_reservations_create(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_create_builder(
            &self.http_client,
            &args.parent,
            &args.reservationId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_reservations_delete(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations failover reservation.
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
    pub fn bigqueryreservation_projects_locations_reservations_failover_reservation(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsFailoverReservationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_failover_reservation_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_failover_reservation_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations get.
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
    pub fn bigqueryreservation_projects_locations_reservations_get(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations get iam policy.
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
    pub fn bigqueryreservation_projects_locations_reservations_get_iam_policy(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations list.
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
    pub fn bigqueryreservation_projects_locations_reservations_list(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReservationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations patch.
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
    pub fn bigqueryreservation_projects_locations_reservations_patch(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Reservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations set iam policy.
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
    pub fn bigqueryreservation_projects_locations_reservations_set_iam_policy(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations test iam permissions.
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
    pub fn bigqueryreservation_projects_locations_reservations_test_iam_permissions(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations assignments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Assignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigqueryreservation_projects_locations_reservations_assignments_create(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsAssignmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Assignment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_assignments_create_builder(
            &self.http_client,
            &args.parent,
            &args.assignmentId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_assignments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations assignments delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_reservations_assignments_delete(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsAssignmentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_assignments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_assignments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations assignments get iam policy.
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
    pub fn bigqueryreservation_projects_locations_reservations_assignments_get_iam_policy(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsAssignmentsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_assignments_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_assignments_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations assignments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAssignmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_reservations_assignments_list(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsAssignmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAssignmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_assignments_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_assignments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations assignments move.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Assignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_reservations_assignments_move(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsAssignmentsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Assignment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_assignments_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_assignments_move_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations assignments patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Assignment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigqueryreservation_projects_locations_reservations_assignments_patch(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsAssignmentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Assignment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_assignments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_assignments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations assignments set iam policy.
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
    pub fn bigqueryreservation_projects_locations_reservations_assignments_set_iam_policy(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsAssignmentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_assignments_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_assignments_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigqueryreservation projects locations reservations assignments test iam permissions.
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
    pub fn bigqueryreservation_projects_locations_reservations_assignments_test_iam_permissions(
        &self,
        args: &BigqueryreservationProjectsLocationsReservationsAssignmentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigqueryreservation_projects_locations_reservations_assignments_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigqueryreservation_projects_locations_reservations_assignments_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
