//! GamesConfigurationProvider - State-aware gamesConfiguration API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       gamesConfiguration API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::gamesConfiguration::{
    games_configuration_achievement_configurations_delete_builder, games_configuration_achievement_configurations_delete_task,
    games_configuration_achievement_configurations_get_builder, games_configuration_achievement_configurations_get_task,
    games_configuration_achievement_configurations_insert_builder, games_configuration_achievement_configurations_insert_task,
    games_configuration_achievement_configurations_list_builder, games_configuration_achievement_configurations_list_task,
    games_configuration_achievement_configurations_update_builder, games_configuration_achievement_configurations_update_task,
    games_configuration_leaderboard_configurations_delete_builder, games_configuration_leaderboard_configurations_delete_task,
    games_configuration_leaderboard_configurations_get_builder, games_configuration_leaderboard_configurations_get_task,
    games_configuration_leaderboard_configurations_insert_builder, games_configuration_leaderboard_configurations_insert_task,
    games_configuration_leaderboard_configurations_list_builder, games_configuration_leaderboard_configurations_list_task,
    games_configuration_leaderboard_configurations_update_builder, games_configuration_leaderboard_configurations_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::gamesConfiguration::AchievementConfiguration;
use crate::providers::gcp::clients::gamesConfiguration::AchievementConfigurationListResponse;
use crate::providers::gcp::clients::gamesConfiguration::LeaderboardConfiguration;
use crate::providers::gcp::clients::gamesConfiguration::LeaderboardConfigurationListResponse;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationAchievementConfigurationsDeleteArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationAchievementConfigurationsGetArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationAchievementConfigurationsInsertArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationAchievementConfigurationsListArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationAchievementConfigurationsUpdateArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationLeaderboardConfigurationsDeleteArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationLeaderboardConfigurationsGetArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationLeaderboardConfigurationsInsertArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationLeaderboardConfigurationsListArgs;
use crate::providers::gcp::clients::gamesConfiguration::GamesConfigurationLeaderboardConfigurationsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GamesConfigurationProvider with automatic state tracking.
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
/// let provider = GamesConfigurationProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct GamesConfigurationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> GamesConfigurationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new GamesConfigurationProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Games configuration achievement configurations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_configuration_achievement_configurations_delete(
        &self,
        args: &GamesConfigurationAchievementConfigurationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_achievement_configurations_delete_builder(
            &self.http_client,
            &args.achievementId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_achievement_configurations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration achievement configurations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_configuration_achievement_configurations_get(
        &self,
        args: &GamesConfigurationAchievementConfigurationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_achievement_configurations_get_builder(
            &self.http_client,
            &args.achievementId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_achievement_configurations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration achievement configurations insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_configuration_achievement_configurations_insert(
        &self,
        args: &GamesConfigurationAchievementConfigurationsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_achievement_configurations_insert_builder(
            &self.http_client,
            &args.applicationId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_achievement_configurations_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration achievement configurations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementConfigurationListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_configuration_achievement_configurations_list(
        &self,
        args: &GamesConfigurationAchievementConfigurationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementConfigurationListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_achievement_configurations_list_builder(
            &self.http_client,
            &args.applicationId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_achievement_configurations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration achievement configurations update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_configuration_achievement_configurations_update(
        &self,
        args: &GamesConfigurationAchievementConfigurationsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_achievement_configurations_update_builder(
            &self.http_client,
            &args.achievementId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_achievement_configurations_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration leaderboard configurations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_configuration_leaderboard_configurations_delete(
        &self,
        args: &GamesConfigurationLeaderboardConfigurationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_leaderboard_configurations_delete_builder(
            &self.http_client,
            &args.leaderboardId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_leaderboard_configurations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration leaderboard configurations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaderboardConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_configuration_leaderboard_configurations_get(
        &self,
        args: &GamesConfigurationLeaderboardConfigurationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaderboardConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_leaderboard_configurations_get_builder(
            &self.http_client,
            &args.leaderboardId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_leaderboard_configurations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration leaderboard configurations insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaderboardConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_configuration_leaderboard_configurations_insert(
        &self,
        args: &GamesConfigurationLeaderboardConfigurationsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaderboardConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_leaderboard_configurations_insert_builder(
            &self.http_client,
            &args.applicationId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_leaderboard_configurations_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration leaderboard configurations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaderboardConfigurationListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_configuration_leaderboard_configurations_list(
        &self,
        args: &GamesConfigurationLeaderboardConfigurationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaderboardConfigurationListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_leaderboard_configurations_list_builder(
            &self.http_client,
            &args.applicationId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_leaderboard_configurations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games configuration leaderboard configurations update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaderboardConfiguration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_configuration_leaderboard_configurations_update(
        &self,
        args: &GamesConfigurationLeaderboardConfigurationsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaderboardConfiguration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_configuration_leaderboard_configurations_update_builder(
            &self.http_client,
            &args.leaderboardId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_configuration_leaderboard_configurations_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
