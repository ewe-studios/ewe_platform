//! FirebaserulesProvider - State-aware firebaserules API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebaserules API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebaserules::{
    firebaserules_projects_test_builder, firebaserules_projects_test_task,
    firebaserules_projects_releases_create_builder, firebaserules_projects_releases_create_task,
    firebaserules_projects_releases_delete_builder, firebaserules_projects_releases_delete_task,
    firebaserules_projects_releases_get_builder, firebaserules_projects_releases_get_task,
    firebaserules_projects_releases_get_executable_builder, firebaserules_projects_releases_get_executable_task,
    firebaserules_projects_releases_list_builder, firebaserules_projects_releases_list_task,
    firebaserules_projects_releases_patch_builder, firebaserules_projects_releases_patch_task,
    firebaserules_projects_rulesets_create_builder, firebaserules_projects_rulesets_create_task,
    firebaserules_projects_rulesets_delete_builder, firebaserules_projects_rulesets_delete_task,
    firebaserules_projects_rulesets_get_builder, firebaserules_projects_rulesets_get_task,
    firebaserules_projects_rulesets_list_builder, firebaserules_projects_rulesets_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebaserules::Empty;
use crate::providers::gcp::clients::firebaserules::GetReleaseExecutableResponse;
use crate::providers::gcp::clients::firebaserules::ListReleasesResponse;
use crate::providers::gcp::clients::firebaserules::ListRulesetsResponse;
use crate::providers::gcp::clients::firebaserules::Release;
use crate::providers::gcp::clients::firebaserules::Ruleset;
use crate::providers::gcp::clients::firebaserules::TestRulesetResponse;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsReleasesCreateArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsReleasesDeleteArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsReleasesGetArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsReleasesGetExecutableArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsReleasesListArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsReleasesPatchArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsRulesetsCreateArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsRulesetsDeleteArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsRulesetsGetArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsRulesetsListArgs;
use crate::providers::gcp::clients::firebaserules::FirebaserulesProjectsTestArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebaserulesProvider with automatic state tracking.
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
/// let provider = FirebaserulesProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebaserulesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebaserulesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebaserulesProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebaserules projects test.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestRulesetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaserules_projects_test(
        &self,
        args: &FirebaserulesProjectsTestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestRulesetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_test_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_test_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects releases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Release result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaserules_projects_releases_create(
        &self,
        args: &FirebaserulesProjectsReleasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Release, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_releases_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_releases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects releases delete.
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
    pub fn firebaserules_projects_releases_delete(
        &self,
        args: &FirebaserulesProjectsReleasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_releases_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_releases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects releases get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Release result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaserules_projects_releases_get(
        &self,
        args: &FirebaserulesProjectsReleasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Release, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_releases_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_releases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects releases get executable.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetReleaseExecutableResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaserules_projects_releases_get_executable(
        &self,
        args: &FirebaserulesProjectsReleasesGetExecutableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetReleaseExecutableResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_releases_get_executable_builder(
            &self.http_client,
            &args.name,
            &args.executableVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_releases_get_executable_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects releases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReleasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaserules_projects_releases_list(
        &self,
        args: &FirebaserulesProjectsReleasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReleasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_releases_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_releases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects releases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Release result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaserules_projects_releases_patch(
        &self,
        args: &FirebaserulesProjectsReleasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Release, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_releases_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_releases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects rulesets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Ruleset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebaserules_projects_rulesets_create(
        &self,
        args: &FirebaserulesProjectsRulesetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Ruleset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_rulesets_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_rulesets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects rulesets delete.
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
    pub fn firebaserules_projects_rulesets_delete(
        &self,
        args: &FirebaserulesProjectsRulesetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_rulesets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_rulesets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects rulesets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Ruleset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaserules_projects_rulesets_get(
        &self,
        args: &FirebaserulesProjectsRulesetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Ruleset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_rulesets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_rulesets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebaserules projects rulesets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRulesetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebaserules_projects_rulesets_list(
        &self,
        args: &FirebaserulesProjectsRulesetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRulesetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebaserules_projects_rulesets_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebaserules_projects_rulesets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
