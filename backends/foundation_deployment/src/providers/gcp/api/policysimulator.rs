//! PolicysimulatorProvider - State-aware policysimulator API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       policysimulator API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::policysimulator::{
    policysimulator_folders_locations_access_policy_simulations_operations_get_builder, policysimulator_folders_locations_access_policy_simulations_operations_get_task,
    policysimulator_folders_locations_org_policy_violations_previews_operations_get_builder, policysimulator_folders_locations_org_policy_violations_previews_operations_get_task,
    policysimulator_folders_locations_replays_create_builder, policysimulator_folders_locations_replays_create_task,
    policysimulator_folders_locations_replays_get_builder, policysimulator_folders_locations_replays_get_task,
    policysimulator_folders_locations_replays_operations_get_builder, policysimulator_folders_locations_replays_operations_get_task,
    policysimulator_folders_locations_replays_operations_list_builder, policysimulator_folders_locations_replays_operations_list_task,
    policysimulator_folders_locations_replays_results_list_builder, policysimulator_folders_locations_replays_results_list_task,
    policysimulator_operations_get_builder, policysimulator_operations_get_task,
    policysimulator_operations_list_builder, policysimulator_operations_list_task,
    policysimulator_organizations_locations_access_policy_simulations_operations_get_builder, policysimulator_organizations_locations_access_policy_simulations_operations_get_task,
    policysimulator_organizations_locations_org_policy_violations_previews_create_builder, policysimulator_organizations_locations_org_policy_violations_previews_create_task,
    policysimulator_organizations_locations_org_policy_violations_previews_get_builder, policysimulator_organizations_locations_org_policy_violations_previews_get_task,
    policysimulator_organizations_locations_org_policy_violations_previews_list_builder, policysimulator_organizations_locations_org_policy_violations_previews_list_task,
    policysimulator_organizations_locations_org_policy_violations_previews_operations_get_builder, policysimulator_organizations_locations_org_policy_violations_previews_operations_get_task,
    policysimulator_organizations_locations_org_policy_violations_previews_org_policy_violations_list_builder, policysimulator_organizations_locations_org_policy_violations_previews_org_policy_violations_list_task,
    policysimulator_organizations_locations_replays_create_builder, policysimulator_organizations_locations_replays_create_task,
    policysimulator_organizations_locations_replays_get_builder, policysimulator_organizations_locations_replays_get_task,
    policysimulator_organizations_locations_replays_operations_get_builder, policysimulator_organizations_locations_replays_operations_get_task,
    policysimulator_organizations_locations_replays_operations_list_builder, policysimulator_organizations_locations_replays_operations_list_task,
    policysimulator_organizations_locations_replays_results_list_builder, policysimulator_organizations_locations_replays_results_list_task,
    policysimulator_projects_locations_access_policy_simulations_operations_get_builder, policysimulator_projects_locations_access_policy_simulations_operations_get_task,
    policysimulator_projects_locations_org_policy_violations_previews_operations_get_builder, policysimulator_projects_locations_org_policy_violations_previews_operations_get_task,
    policysimulator_projects_locations_replays_create_builder, policysimulator_projects_locations_replays_create_task,
    policysimulator_projects_locations_replays_get_builder, policysimulator_projects_locations_replays_get_task,
    policysimulator_projects_locations_replays_operations_get_builder, policysimulator_projects_locations_replays_operations_get_task,
    policysimulator_projects_locations_replays_operations_list_builder, policysimulator_projects_locations_replays_operations_list_task,
    policysimulator_projects_locations_replays_results_list_builder, policysimulator_projects_locations_replays_results_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::policysimulator::GoogleCloudPolicysimulatorV1ListOrgPolicyViolationsPreviewsResponse;
use crate::providers::gcp::clients::policysimulator::GoogleCloudPolicysimulatorV1ListOrgPolicyViolationsResponse;
use crate::providers::gcp::clients::policysimulator::GoogleCloudPolicysimulatorV1ListReplayResultsResponse;
use crate::providers::gcp::clients::policysimulator::GoogleCloudPolicysimulatorV1OrgPolicyViolationsPreview;
use crate::providers::gcp::clients::policysimulator::GoogleCloudPolicysimulatorV1Replay;
use crate::providers::gcp::clients::policysimulator::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::policysimulator::GoogleLongrunningOperation;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorFoldersLocationsAccessPolicySimulationsOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorFoldersLocationsOrgPolicyViolationsPreviewsOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorFoldersLocationsReplaysCreateArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorFoldersLocationsReplaysGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorFoldersLocationsReplaysOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorFoldersLocationsReplaysOperationsListArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorFoldersLocationsReplaysResultsListArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOperationsListArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsAccessPolicySimulationsOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsCreateArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsListArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsOrgPolicyViolationsListArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsReplaysCreateArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsReplaysGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsReplaysOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsReplaysOperationsListArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorOrganizationsLocationsReplaysResultsListArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorProjectsLocationsAccessPolicySimulationsOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorProjectsLocationsOrgPolicyViolationsPreviewsOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorProjectsLocationsReplaysCreateArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorProjectsLocationsReplaysGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorProjectsLocationsReplaysOperationsGetArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorProjectsLocationsReplaysOperationsListArgs;
use crate::providers::gcp::clients::policysimulator::PolicysimulatorProjectsLocationsReplaysResultsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PolicysimulatorProvider with automatic state tracking.
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
/// let provider = PolicysimulatorProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PolicysimulatorProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PolicysimulatorProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PolicysimulatorProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PolicysimulatorProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Policysimulator folders locations access policy simulations operations get.
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
    pub fn policysimulator_folders_locations_access_policy_simulations_operations_get(
        &self,
        args: &PolicysimulatorFoldersLocationsAccessPolicySimulationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_folders_locations_access_policy_simulations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_folders_locations_access_policy_simulations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator folders locations org policy violations previews operations get.
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
    pub fn policysimulator_folders_locations_org_policy_violations_previews_operations_get(
        &self,
        args: &PolicysimulatorFoldersLocationsOrgPolicyViolationsPreviewsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_folders_locations_org_policy_violations_previews_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_folders_locations_org_policy_violations_previews_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator folders locations replays create.
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
    pub fn policysimulator_folders_locations_replays_create(
        &self,
        args: &PolicysimulatorFoldersLocationsReplaysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_folders_locations_replays_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_folders_locations_replays_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator folders locations replays get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1Replay result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_folders_locations_replays_get(
        &self,
        args: &PolicysimulatorFoldersLocationsReplaysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1Replay, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_folders_locations_replays_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_folders_locations_replays_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator folders locations replays operations get.
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
    pub fn policysimulator_folders_locations_replays_operations_get(
        &self,
        args: &PolicysimulatorFoldersLocationsReplaysOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_folders_locations_replays_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_folders_locations_replays_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator folders locations replays operations list.
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
    pub fn policysimulator_folders_locations_replays_operations_list(
        &self,
        args: &PolicysimulatorFoldersLocationsReplaysOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_folders_locations_replays_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_folders_locations_replays_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator folders locations replays results list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1ListReplayResultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_folders_locations_replays_results_list(
        &self,
        args: &PolicysimulatorFoldersLocationsReplaysResultsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1ListReplayResultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_folders_locations_replays_results_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_folders_locations_replays_results_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator operations get.
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
    pub fn policysimulator_operations_get(
        &self,
        args: &PolicysimulatorOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator operations list.
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
    pub fn policysimulator_operations_list(
        &self,
        args: &PolicysimulatorOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations access policy simulations operations get.
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
    pub fn policysimulator_organizations_locations_access_policy_simulations_operations_get(
        &self,
        args: &PolicysimulatorOrganizationsLocationsAccessPolicySimulationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_access_policy_simulations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_access_policy_simulations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations org policy violations previews create.
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
    pub fn policysimulator_organizations_locations_org_policy_violations_previews_create(
        &self,
        args: &PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_org_policy_violations_previews_create_builder(
            &self.http_client,
            &args.parent,
            &args.orgPolicyViolationsPreviewId,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_org_policy_violations_previews_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations org policy violations previews get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1OrgPolicyViolationsPreview result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_organizations_locations_org_policy_violations_previews_get(
        &self,
        args: &PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1OrgPolicyViolationsPreview, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_org_policy_violations_previews_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_org_policy_violations_previews_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations org policy violations previews list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1ListOrgPolicyViolationsPreviewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_organizations_locations_org_policy_violations_previews_list(
        &self,
        args: &PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1ListOrgPolicyViolationsPreviewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_org_policy_violations_previews_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_org_policy_violations_previews_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations org policy violations previews operations get.
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
    pub fn policysimulator_organizations_locations_org_policy_violations_previews_operations_get(
        &self,
        args: &PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_org_policy_violations_previews_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_org_policy_violations_previews_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations org policy violations previews org policy violations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1ListOrgPolicyViolationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_organizations_locations_org_policy_violations_previews_org_policy_violations_list(
        &self,
        args: &PolicysimulatorOrganizationsLocationsOrgPolicyViolationsPreviewsOrgPolicyViolationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1ListOrgPolicyViolationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_org_policy_violations_previews_org_policy_violations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_org_policy_violations_previews_org_policy_violations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations replays create.
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
    pub fn policysimulator_organizations_locations_replays_create(
        &self,
        args: &PolicysimulatorOrganizationsLocationsReplaysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_replays_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_replays_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations replays get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1Replay result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_organizations_locations_replays_get(
        &self,
        args: &PolicysimulatorOrganizationsLocationsReplaysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1Replay, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_replays_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_replays_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations replays operations get.
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
    pub fn policysimulator_organizations_locations_replays_operations_get(
        &self,
        args: &PolicysimulatorOrganizationsLocationsReplaysOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_replays_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_replays_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations replays operations list.
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
    pub fn policysimulator_organizations_locations_replays_operations_list(
        &self,
        args: &PolicysimulatorOrganizationsLocationsReplaysOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_replays_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_replays_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator organizations locations replays results list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1ListReplayResultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_organizations_locations_replays_results_list(
        &self,
        args: &PolicysimulatorOrganizationsLocationsReplaysResultsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1ListReplayResultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_organizations_locations_replays_results_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_organizations_locations_replays_results_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator projects locations access policy simulations operations get.
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
    pub fn policysimulator_projects_locations_access_policy_simulations_operations_get(
        &self,
        args: &PolicysimulatorProjectsLocationsAccessPolicySimulationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_projects_locations_access_policy_simulations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_projects_locations_access_policy_simulations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator projects locations org policy violations previews operations get.
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
    pub fn policysimulator_projects_locations_org_policy_violations_previews_operations_get(
        &self,
        args: &PolicysimulatorProjectsLocationsOrgPolicyViolationsPreviewsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_projects_locations_org_policy_violations_previews_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_projects_locations_org_policy_violations_previews_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator projects locations replays create.
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
    pub fn policysimulator_projects_locations_replays_create(
        &self,
        args: &PolicysimulatorProjectsLocationsReplaysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_projects_locations_replays_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_projects_locations_replays_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator projects locations replays get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1Replay result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_projects_locations_replays_get(
        &self,
        args: &PolicysimulatorProjectsLocationsReplaysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1Replay, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_projects_locations_replays_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_projects_locations_replays_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator projects locations replays operations get.
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
    pub fn policysimulator_projects_locations_replays_operations_get(
        &self,
        args: &PolicysimulatorProjectsLocationsReplaysOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_projects_locations_replays_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_projects_locations_replays_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator projects locations replays operations list.
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
    pub fn policysimulator_projects_locations_replays_operations_list(
        &self,
        args: &PolicysimulatorProjectsLocationsReplaysOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_projects_locations_replays_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_projects_locations_replays_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policysimulator projects locations replays results list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicysimulatorV1ListReplayResultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policysimulator_projects_locations_replays_results_list(
        &self,
        args: &PolicysimulatorProjectsLocationsReplaysResultsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicysimulatorV1ListReplayResultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policysimulator_projects_locations_replays_results_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = policysimulator_projects_locations_replays_results_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
