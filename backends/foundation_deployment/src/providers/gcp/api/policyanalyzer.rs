//! PolicyanalyzerProvider - State-aware policyanalyzer API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       policyanalyzer API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::policyanalyzer::{
    policyanalyzer_folders_locations_activity_types_activities_query_builder, policyanalyzer_folders_locations_activity_types_activities_query_task,
    policyanalyzer_organizations_locations_activity_types_activities_query_builder, policyanalyzer_organizations_locations_activity_types_activities_query_task,
    policyanalyzer_projects_locations_activity_types_activities_query_builder, policyanalyzer_projects_locations_activity_types_activities_query_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::policyanalyzer::GoogleCloudPolicyanalyzerV1QueryActivityResponse;
use crate::providers::gcp::clients::policyanalyzer::PolicyanalyzerFoldersLocationsActivityTypesActivitiesQueryArgs;
use crate::providers::gcp::clients::policyanalyzer::PolicyanalyzerOrganizationsLocationsActivityTypesActivitiesQueryArgs;
use crate::providers::gcp::clients::policyanalyzer::PolicyanalyzerProjectsLocationsActivityTypesActivitiesQueryArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PolicyanalyzerProvider with automatic state tracking.
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
/// let provider = PolicyanalyzerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PolicyanalyzerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PolicyanalyzerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PolicyanalyzerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Policyanalyzer folders locations activity types activities query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicyanalyzerV1QueryActivityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policyanalyzer_folders_locations_activity_types_activities_query(
        &self,
        args: &PolicyanalyzerFoldersLocationsActivityTypesActivitiesQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicyanalyzerV1QueryActivityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policyanalyzer_folders_locations_activity_types_activities_query_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = policyanalyzer_folders_locations_activity_types_activities_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policyanalyzer organizations locations activity types activities query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicyanalyzerV1QueryActivityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policyanalyzer_organizations_locations_activity_types_activities_query(
        &self,
        args: &PolicyanalyzerOrganizationsLocationsActivityTypesActivitiesQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicyanalyzerV1QueryActivityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policyanalyzer_organizations_locations_activity_types_activities_query_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = policyanalyzer_organizations_locations_activity_types_activities_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Policyanalyzer projects locations activity types activities query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicyanalyzerV1QueryActivityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn policyanalyzer_projects_locations_activity_types_activities_query(
        &self,
        args: &PolicyanalyzerProjectsLocationsActivityTypesActivitiesQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicyanalyzerV1QueryActivityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policyanalyzer_projects_locations_activity_types_activities_query_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = policyanalyzer_projects_locations_activity_types_activities_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
