//! GamesManagementProvider - State-aware gamesManagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       gamesManagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::gamesManagement::{
    games_management_achievements_reset_builder, games_management_achievements_reset_task,
    games_management_achievements_reset_all_builder, games_management_achievements_reset_all_task,
    games_management_achievements_reset_all_for_all_players_builder, games_management_achievements_reset_all_for_all_players_task,
    games_management_achievements_reset_for_all_players_builder, games_management_achievements_reset_for_all_players_task,
    games_management_achievements_reset_multiple_for_all_players_builder, games_management_achievements_reset_multiple_for_all_players_task,
    games_management_applications_list_hidden_builder, games_management_applications_list_hidden_task,
    games_management_events_reset_builder, games_management_events_reset_task,
    games_management_events_reset_all_builder, games_management_events_reset_all_task,
    games_management_events_reset_all_for_all_players_builder, games_management_events_reset_all_for_all_players_task,
    games_management_events_reset_for_all_players_builder, games_management_events_reset_for_all_players_task,
    games_management_events_reset_multiple_for_all_players_builder, games_management_events_reset_multiple_for_all_players_task,
    games_management_players_hide_builder, games_management_players_hide_task,
    games_management_players_unhide_builder, games_management_players_unhide_task,
    games_management_scores_reset_builder, games_management_scores_reset_task,
    games_management_scores_reset_all_builder, games_management_scores_reset_all_task,
    games_management_scores_reset_all_for_all_players_builder, games_management_scores_reset_all_for_all_players_task,
    games_management_scores_reset_for_all_players_builder, games_management_scores_reset_for_all_players_task,
    games_management_scores_reset_multiple_for_all_players_builder, games_management_scores_reset_multiple_for_all_players_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::gamesManagement::AchievementResetAllResponse;
use crate::providers::gcp::clients::gamesManagement::AchievementResetResponse;
use crate::providers::gcp::clients::gamesManagement::HiddenPlayerList;
use crate::providers::gcp::clients::gamesManagement::PlayerScoreResetAllResponse;
use crate::providers::gcp::clients::gamesManagement::PlayerScoreResetResponse;
use crate::providers::gcp::clients::gamesManagement::GamesManagementAchievementsResetAllArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementAchievementsResetAllForAllPlayersArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementAchievementsResetArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementAchievementsResetForAllPlayersArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementAchievementsResetMultipleForAllPlayersArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementApplicationsListHiddenArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementEventsResetAllArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementEventsResetAllForAllPlayersArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementEventsResetArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementEventsResetForAllPlayersArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementEventsResetMultipleForAllPlayersArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementPlayersHideArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementPlayersUnhideArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementScoresResetAllArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementScoresResetAllForAllPlayersArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementScoresResetArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementScoresResetForAllPlayersArgs;
use crate::providers::gcp::clients::gamesManagement::GamesManagementScoresResetMultipleForAllPlayersArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GamesManagementProvider with automatic state tracking.
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
/// let provider = GamesManagementProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct GamesManagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> GamesManagementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new GamesManagementProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Games management achievements reset.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementResetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_management_achievements_reset(
        &self,
        args: &GamesManagementAchievementsResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementResetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_achievements_reset_builder(
            &self.http_client,
            &args.achievementId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_achievements_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management achievements reset all.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementResetAllResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_management_achievements_reset_all(
        &self,
        args: &GamesManagementAchievementsResetAllArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementResetAllResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_achievements_reset_all_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_achievements_reset_all_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management achievements reset all for all players.
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
    pub fn games_management_achievements_reset_all_for_all_players(
        &self,
        args: &GamesManagementAchievementsResetAllForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_achievements_reset_all_for_all_players_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_achievements_reset_all_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management achievements reset for all players.
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
    pub fn games_management_achievements_reset_for_all_players(
        &self,
        args: &GamesManagementAchievementsResetForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_achievements_reset_for_all_players_builder(
            &self.http_client,
            &args.achievementId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_achievements_reset_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management achievements reset multiple for all players.
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
    pub fn games_management_achievements_reset_multiple_for_all_players(
        &self,
        args: &GamesManagementAchievementsResetMultipleForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_achievements_reset_multiple_for_all_players_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_achievements_reset_multiple_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management applications list hidden.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HiddenPlayerList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_management_applications_list_hidden(
        &self,
        args: &GamesManagementApplicationsListHiddenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HiddenPlayerList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_applications_list_hidden_builder(
            &self.http_client,
            &args.applicationId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_applications_list_hidden_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management events reset.
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
    pub fn games_management_events_reset(
        &self,
        args: &GamesManagementEventsResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_events_reset_builder(
            &self.http_client,
            &args.eventId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_events_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management events reset all.
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
    pub fn games_management_events_reset_all(
        &self,
        args: &GamesManagementEventsResetAllArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_events_reset_all_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_events_reset_all_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management events reset all for all players.
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
    pub fn games_management_events_reset_all_for_all_players(
        &self,
        args: &GamesManagementEventsResetAllForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_events_reset_all_for_all_players_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_events_reset_all_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management events reset for all players.
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
    pub fn games_management_events_reset_for_all_players(
        &self,
        args: &GamesManagementEventsResetForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_events_reset_for_all_players_builder(
            &self.http_client,
            &args.eventId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_events_reset_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management events reset multiple for all players.
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
    pub fn games_management_events_reset_multiple_for_all_players(
        &self,
        args: &GamesManagementEventsResetMultipleForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_events_reset_multiple_for_all_players_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_events_reset_multiple_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management players hide.
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
    pub fn games_management_players_hide(
        &self,
        args: &GamesManagementPlayersHideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_players_hide_builder(
            &self.http_client,
            &args.applicationId,
            &args.playerId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_players_hide_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management players unhide.
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
    pub fn games_management_players_unhide(
        &self,
        args: &GamesManagementPlayersUnhideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_players_unhide_builder(
            &self.http_client,
            &args.applicationId,
            &args.playerId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_players_unhide_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management scores reset.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlayerScoreResetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_management_scores_reset(
        &self,
        args: &GamesManagementScoresResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlayerScoreResetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_scores_reset_builder(
            &self.http_client,
            &args.leaderboardId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_scores_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management scores reset all.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlayerScoreResetAllResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_management_scores_reset_all(
        &self,
        args: &GamesManagementScoresResetAllArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlayerScoreResetAllResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_scores_reset_all_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_scores_reset_all_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management scores reset all for all players.
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
    pub fn games_management_scores_reset_all_for_all_players(
        &self,
        args: &GamesManagementScoresResetAllForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_scores_reset_all_for_all_players_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_scores_reset_all_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management scores reset for all players.
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
    pub fn games_management_scores_reset_for_all_players(
        &self,
        args: &GamesManagementScoresResetForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_scores_reset_for_all_players_builder(
            &self.http_client,
            &args.leaderboardId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_scores_reset_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games management scores reset multiple for all players.
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
    pub fn games_management_scores_reset_multiple_for_all_players(
        &self,
        args: &GamesManagementScoresResetMultipleForAllPlayersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_management_scores_reset_multiple_for_all_players_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_management_scores_reset_multiple_for_all_players_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
