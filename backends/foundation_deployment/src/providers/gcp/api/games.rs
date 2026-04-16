//! GamesProvider - State-aware games API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       games API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::games::{
    games_accesstokens_generate_play_grouping_api_token_builder, games_accesstokens_generate_play_grouping_api_token_task,
    games_accesstokens_generate_recall_play_grouping_api_token_builder, games_accesstokens_generate_recall_play_grouping_api_token_task,
    games_achievement_definitions_list_builder, games_achievement_definitions_list_task,
    games_achievements_increment_builder, games_achievements_increment_task,
    games_achievements_list_builder, games_achievements_list_task,
    games_achievements_reveal_builder, games_achievements_reveal_task,
    games_achievements_set_steps_at_least_builder, games_achievements_set_steps_at_least_task,
    games_achievements_unlock_builder, games_achievements_unlock_task,
    games_achievements_update_multiple_builder, games_achievements_update_multiple_task,
    games_applications_get_builder, games_applications_get_task,
    games_applications_get_end_point_builder, games_applications_get_end_point_task,
    games_applications_played_builder, games_applications_played_task,
    games_applications_verify_builder, games_applications_verify_task,
    games_events_list_by_player_builder, games_events_list_by_player_task,
    games_events_list_definitions_builder, games_events_list_definitions_task,
    games_events_record_builder, games_events_record_task,
    games_leaderboards_get_builder, games_leaderboards_get_task,
    games_leaderboards_list_builder, games_leaderboards_list_task,
    games_metagame_get_metagame_config_builder, games_metagame_get_metagame_config_task,
    games_metagame_list_categories_by_player_builder, games_metagame_list_categories_by_player_task,
    games_players_get_builder, games_players_get_task,
    games_players_get_multiple_application_player_ids_builder, games_players_get_multiple_application_player_ids_task,
    games_players_get_scoped_player_ids_builder, games_players_get_scoped_player_ids_task,
    games_players_list_builder, games_players_list_task,
    games_recall_games_player_tokens_builder, games_recall_games_player_tokens_task,
    games_recall_last_token_from_all_developer_games_builder, games_recall_last_token_from_all_developer_games_task,
    games_recall_link_persona_builder, games_recall_link_persona_task,
    games_recall_reset_persona_builder, games_recall_reset_persona_task,
    games_recall_retrieve_tokens_builder, games_recall_retrieve_tokens_task,
    games_recall_unlink_persona_builder, games_recall_unlink_persona_task,
    games_revisions_check_builder, games_revisions_check_task,
    games_scores_get_builder, games_scores_get_task,
    games_scores_list_builder, games_scores_list_task,
    games_scores_list_window_builder, games_scores_list_window_task,
    games_scores_submit_builder, games_scores_submit_task,
    games_scores_submit_multiple_builder, games_scores_submit_multiple_task,
    games_snapshots_get_builder, games_snapshots_get_task,
    games_snapshots_list_builder, games_snapshots_list_task,
    games_stats_get_builder, games_stats_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::games::AchievementDefinitionsListResponse;
use crate::providers::gcp::clients::games::AchievementIncrementResponse;
use crate::providers::gcp::clients::games::AchievementRevealResponse;
use crate::providers::gcp::clients::games::AchievementSetStepsAtLeastResponse;
use crate::providers::gcp::clients::games::AchievementUnlockResponse;
use crate::providers::gcp::clients::games::AchievementUpdateMultipleResponse;
use crate::providers::gcp::clients::games::Application;
use crate::providers::gcp::clients::games::ApplicationVerifyResponse;
use crate::providers::gcp::clients::games::CategoryListResponse;
use crate::providers::gcp::clients::games::EndPoint;
use crate::providers::gcp::clients::games::EventDefinitionListResponse;
use crate::providers::gcp::clients::games::EventUpdateResponse;
use crate::providers::gcp::clients::games::GeneratePlayGroupingApiTokenResponse;
use crate::providers::gcp::clients::games::GenerateRecallPlayGroupingApiTokenResponse;
use crate::providers::gcp::clients::games::GetMultipleApplicationPlayerIdsResponse;
use crate::providers::gcp::clients::games::Leaderboard;
use crate::providers::gcp::clients::games::LeaderboardListResponse;
use crate::providers::gcp::clients::games::LeaderboardScores;
use crate::providers::gcp::clients::games::LinkPersonaResponse;
use crate::providers::gcp::clients::games::MetagameConfig;
use crate::providers::gcp::clients::games::Player;
use crate::providers::gcp::clients::games::PlayerAchievementListResponse;
use crate::providers::gcp::clients::games::PlayerEventListResponse;
use crate::providers::gcp::clients::games::PlayerLeaderboardScoreListResponse;
use crate::providers::gcp::clients::games::PlayerListResponse;
use crate::providers::gcp::clients::games::PlayerScoreListResponse;
use crate::providers::gcp::clients::games::PlayerScoreResponse;
use crate::providers::gcp::clients::games::ResetPersonaResponse;
use crate::providers::gcp::clients::games::RetrieveDeveloperGamesLastPlayerTokenResponse;
use crate::providers::gcp::clients::games::RetrieveGamesPlayerTokensResponse;
use crate::providers::gcp::clients::games::RetrievePlayerTokensResponse;
use crate::providers::gcp::clients::games::RevisionCheckResponse;
use crate::providers::gcp::clients::games::ScopedPlayerIds;
use crate::providers::gcp::clients::games::Snapshot;
use crate::providers::gcp::clients::games::SnapshotListResponse;
use crate::providers::gcp::clients::games::StatsResponse;
use crate::providers::gcp::clients::games::UnlinkPersonaResponse;
use crate::providers::gcp::clients::games::GamesAccesstokensGeneratePlayGroupingApiTokenArgs;
use crate::providers::gcp::clients::games::GamesAccesstokensGenerateRecallPlayGroupingApiTokenArgs;
use crate::providers::gcp::clients::games::GamesAchievementDefinitionsListArgs;
use crate::providers::gcp::clients::games::GamesAchievementsIncrementArgs;
use crate::providers::gcp::clients::games::GamesAchievementsListArgs;
use crate::providers::gcp::clients::games::GamesAchievementsRevealArgs;
use crate::providers::gcp::clients::games::GamesAchievementsSetStepsAtLeastArgs;
use crate::providers::gcp::clients::games::GamesAchievementsUnlockArgs;
use crate::providers::gcp::clients::games::GamesApplicationsGetArgs;
use crate::providers::gcp::clients::games::GamesApplicationsGetEndPointArgs;
use crate::providers::gcp::clients::games::GamesApplicationsVerifyArgs;
use crate::providers::gcp::clients::games::GamesEventsListByPlayerArgs;
use crate::providers::gcp::clients::games::GamesEventsListDefinitionsArgs;
use crate::providers::gcp::clients::games::GamesEventsRecordArgs;
use crate::providers::gcp::clients::games::GamesLeaderboardsGetArgs;
use crate::providers::gcp::clients::games::GamesLeaderboardsListArgs;
use crate::providers::gcp::clients::games::GamesMetagameListCategoriesByPlayerArgs;
use crate::providers::gcp::clients::games::GamesPlayersGetArgs;
use crate::providers::gcp::clients::games::GamesPlayersGetMultipleApplicationPlayerIdsArgs;
use crate::providers::gcp::clients::games::GamesPlayersListArgs;
use crate::providers::gcp::clients::games::GamesRecallGamesPlayerTokensArgs;
use crate::providers::gcp::clients::games::GamesRecallLastTokenFromAllDeveloperGamesArgs;
use crate::providers::gcp::clients::games::GamesRecallRetrieveTokensArgs;
use crate::providers::gcp::clients::games::GamesRevisionsCheckArgs;
use crate::providers::gcp::clients::games::GamesScoresGetArgs;
use crate::providers::gcp::clients::games::GamesScoresListArgs;
use crate::providers::gcp::clients::games::GamesScoresListWindowArgs;
use crate::providers::gcp::clients::games::GamesScoresSubmitArgs;
use crate::providers::gcp::clients::games::GamesScoresSubmitMultipleArgs;
use crate::providers::gcp::clients::games::GamesSnapshotsGetArgs;
use crate::providers::gcp::clients::games::GamesSnapshotsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GamesProvider with automatic state tracking.
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
/// let provider = GamesProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct GamesProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> GamesProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new GamesProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new GamesProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Games accesstokens generate play grouping api token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GeneratePlayGroupingApiTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_accesstokens_generate_play_grouping_api_token(
        &self,
        args: &GamesAccesstokensGeneratePlayGroupingApiTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GeneratePlayGroupingApiTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_accesstokens_generate_play_grouping_api_token_builder(
            &self.http_client,
            &args.packageName,
            &args.persona,
        )
        .map_err(ProviderError::Api)?;

        let task = games_accesstokens_generate_play_grouping_api_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games accesstokens generate recall play grouping api token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateRecallPlayGroupingApiTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_accesstokens_generate_recall_play_grouping_api_token(
        &self,
        args: &GamesAccesstokensGenerateRecallPlayGroupingApiTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateRecallPlayGroupingApiTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_accesstokens_generate_recall_play_grouping_api_token_builder(
            &self.http_client,
            &args.packageName,
            &args.persona,
            &args.recallSessionId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_accesstokens_generate_recall_play_grouping_api_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games achievement definitions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementDefinitionsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_achievement_definitions_list(
        &self,
        args: &GamesAchievementDefinitionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementDefinitionsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_achievement_definitions_list_builder(
            &self.http_client,
            &args.language,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_achievement_definitions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games achievements increment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementIncrementResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_achievements_increment(
        &self,
        args: &GamesAchievementsIncrementArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementIncrementResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_achievements_increment_builder(
            &self.http_client,
            &args.achievementId,
            &args.requestId,
            &args.stepsToIncrement,
        )
        .map_err(ProviderError::Api)?;

        let task = games_achievements_increment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games achievements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlayerAchievementListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_achievements_list(
        &self,
        args: &GamesAchievementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlayerAchievementListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_achievements_list_builder(
            &self.http_client,
            &args.playerId,
            &args.language,
            &args.maxResults,
            &args.pageToken,
            &args.state,
        )
        .map_err(ProviderError::Api)?;

        let task = games_achievements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games achievements reveal.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementRevealResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_achievements_reveal(
        &self,
        args: &GamesAchievementsRevealArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementRevealResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_achievements_reveal_builder(
            &self.http_client,
            &args.achievementId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_achievements_reveal_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games achievements set steps at least.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementSetStepsAtLeastResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_achievements_set_steps_at_least(
        &self,
        args: &GamesAchievementsSetStepsAtLeastArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementSetStepsAtLeastResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_achievements_set_steps_at_least_builder(
            &self.http_client,
            &args.achievementId,
            &args.steps,
        )
        .map_err(ProviderError::Api)?;

        let task = games_achievements_set_steps_at_least_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games achievements unlock.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementUnlockResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_achievements_unlock(
        &self,
        args: &GamesAchievementsUnlockArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementUnlockResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_achievements_unlock_builder(
            &self.http_client,
            &args.achievementId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_achievements_unlock_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games achievements update multiple.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AchievementUpdateMultipleResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_achievements_update_multiple(
        &self,
        args: &GamesAchievementsUpdateMultipleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AchievementUpdateMultipleResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_achievements_update_multiple_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_achievements_update_multiple_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games applications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Application result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_applications_get(
        &self,
        args: &GamesApplicationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Application, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_applications_get_builder(
            &self.http_client,
            &args.applicationId,
            &args.language,
            &args.platformType,
        )
        .map_err(ProviderError::Api)?;

        let task = games_applications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games applications get end point.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EndPoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_applications_get_end_point(
        &self,
        args: &GamesApplicationsGetEndPointArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndPoint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_applications_get_end_point_builder(
            &self.http_client,
            &args.applicationId,
            &args.endPointType,
        )
        .map_err(ProviderError::Api)?;

        let task = games_applications_get_end_point_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games applications played.
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
    pub fn games_applications_played(
        &self,
        args: &GamesApplicationsPlayedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_applications_played_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_applications_played_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games applications verify.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApplicationVerifyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_applications_verify(
        &self,
        args: &GamesApplicationsVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApplicationVerifyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_applications_verify_builder(
            &self.http_client,
            &args.applicationId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_applications_verify_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games events list by player.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlayerEventListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_events_list_by_player(
        &self,
        args: &GamesEventsListByPlayerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlayerEventListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_events_list_by_player_builder(
            &self.http_client,
            &args.language,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_events_list_by_player_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games events list definitions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventDefinitionListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_events_list_definitions(
        &self,
        args: &GamesEventsListDefinitionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventDefinitionListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_events_list_definitions_builder(
            &self.http_client,
            &args.language,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_events_list_definitions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games events record.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EventUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_events_record(
        &self,
        args: &GamesEventsRecordArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EventUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_events_record_builder(
            &self.http_client,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = games_events_record_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games leaderboards get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Leaderboard result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_leaderboards_get(
        &self,
        args: &GamesLeaderboardsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Leaderboard, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_leaderboards_get_builder(
            &self.http_client,
            &args.leaderboardId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = games_leaderboards_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games leaderboards list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaderboardListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_leaderboards_list(
        &self,
        args: &GamesLeaderboardsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaderboardListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_leaderboards_list_builder(
            &self.http_client,
            &args.language,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_leaderboards_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games metagame get metagame config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MetagameConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_metagame_get_metagame_config(
        &self,
        args: &GamesMetagameGetMetagameConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MetagameConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_metagame_get_metagame_config_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_metagame_get_metagame_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games metagame list categories by player.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CategoryListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_metagame_list_categories_by_player(
        &self,
        args: &GamesMetagameListCategoriesByPlayerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CategoryListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_metagame_list_categories_by_player_builder(
            &self.http_client,
            &args.playerId,
            &args.collection,
            &args.language,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_metagame_list_categories_by_player_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games players get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Player result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_players_get(
        &self,
        args: &GamesPlayersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Player, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_players_get_builder(
            &self.http_client,
            &args.playerId,
            &args.language,
            &args.playerIdConsistencyToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_players_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games players get multiple application player ids.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetMultipleApplicationPlayerIdsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_players_get_multiple_application_player_ids(
        &self,
        args: &GamesPlayersGetMultipleApplicationPlayerIdsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetMultipleApplicationPlayerIdsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_players_get_multiple_application_player_ids_builder(
            &self.http_client,
            &args.applicationIds,
        )
        .map_err(ProviderError::Api)?;

        let task = games_players_get_multiple_application_player_ids_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games players get scoped player ids.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScopedPlayerIds result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_players_get_scoped_player_ids(
        &self,
        args: &GamesPlayersGetScopedPlayerIdsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScopedPlayerIds, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_players_get_scoped_player_ids_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_players_get_scoped_player_ids_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games players list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlayerListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_players_list(
        &self,
        args: &GamesPlayersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlayerListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_players_list_builder(
            &self.http_client,
            &args.collection,
            &args.language,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_players_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games recall games player tokens.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetrieveGamesPlayerTokensResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_recall_games_player_tokens(
        &self,
        args: &GamesRecallGamesPlayerTokensArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetrieveGamesPlayerTokensResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_recall_games_player_tokens_builder(
            &self.http_client,
            &args.sessionId,
            &args.applicationIds,
        )
        .map_err(ProviderError::Api)?;

        let task = games_recall_games_player_tokens_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games recall last token from all developer games.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetrieveDeveloperGamesLastPlayerTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_recall_last_token_from_all_developer_games(
        &self,
        args: &GamesRecallLastTokenFromAllDeveloperGamesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetrieveDeveloperGamesLastPlayerTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_recall_last_token_from_all_developer_games_builder(
            &self.http_client,
            &args.sessionId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_recall_last_token_from_all_developer_games_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games recall link persona.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LinkPersonaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_recall_link_persona(
        &self,
        args: &GamesRecallLinkPersonaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LinkPersonaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_recall_link_persona_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_recall_link_persona_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games recall reset persona.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResetPersonaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_recall_reset_persona(
        &self,
        args: &GamesRecallResetPersonaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResetPersonaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_recall_reset_persona_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_recall_reset_persona_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games recall retrieve tokens.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetrievePlayerTokensResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_recall_retrieve_tokens(
        &self,
        args: &GamesRecallRetrieveTokensArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetrievePlayerTokensResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_recall_retrieve_tokens_builder(
            &self.http_client,
            &args.sessionId,
        )
        .map_err(ProviderError::Api)?;

        let task = games_recall_retrieve_tokens_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games recall unlink persona.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UnlinkPersonaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_recall_unlink_persona(
        &self,
        args: &GamesRecallUnlinkPersonaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UnlinkPersonaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_recall_unlink_persona_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_recall_unlink_persona_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games revisions check.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RevisionCheckResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_revisions_check(
        &self,
        args: &GamesRevisionsCheckArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RevisionCheckResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_revisions_check_builder(
            &self.http_client,
            &args.clientRevision,
        )
        .map_err(ProviderError::Api)?;

        let task = games_revisions_check_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games scores get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlayerLeaderboardScoreListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_scores_get(
        &self,
        args: &GamesScoresGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlayerLeaderboardScoreListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_scores_get_builder(
            &self.http_client,
            &args.playerId,
            &args.leaderboardId,
            &args.timeSpan,
            &args.includeRankType,
            &args.language,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_scores_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games scores list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaderboardScores result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_scores_list(
        &self,
        args: &GamesScoresListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaderboardScores, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_scores_list_builder(
            &self.http_client,
            &args.leaderboardId,
            &args.collection,
            &args.language,
            &args.maxResults,
            &args.pageToken,
            &args.timeSpan,
        )
        .map_err(ProviderError::Api)?;

        let task = games_scores_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games scores list window.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaderboardScores result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_scores_list_window(
        &self,
        args: &GamesScoresListWindowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaderboardScores, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_scores_list_window_builder(
            &self.http_client,
            &args.leaderboardId,
            &args.collection,
            &args.language,
            &args.maxResults,
            &args.pageToken,
            &args.resultsAbove,
            &args.returnTopIfAbsent,
            &args.timeSpan,
        )
        .map_err(ProviderError::Api)?;

        let task = games_scores_list_window_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games scores submit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlayerScoreResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_scores_submit(
        &self,
        args: &GamesScoresSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlayerScoreResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_scores_submit_builder(
            &self.http_client,
            &args.leaderboardId,
            &args.language,
            &args.score,
            &args.scoreTag,
        )
        .map_err(ProviderError::Api)?;

        let task = games_scores_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games scores submit multiple.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlayerScoreListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn games_scores_submit_multiple(
        &self,
        args: &GamesScoresSubmitMultipleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlayerScoreListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_scores_submit_multiple_builder(
            &self.http_client,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = games_scores_submit_multiple_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games snapshots get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Snapshot result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_snapshots_get(
        &self,
        args: &GamesSnapshotsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snapshot, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_snapshots_get_builder(
            &self.http_client,
            &args.snapshotId,
            &args.language,
        )
        .map_err(ProviderError::Api)?;

        let task = games_snapshots_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games snapshots list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SnapshotListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_snapshots_list(
        &self,
        args: &GamesSnapshotsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SnapshotListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_snapshots_list_builder(
            &self.http_client,
            &args.playerId,
            &args.language,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = games_snapshots_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Games stats get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn games_stats_get(
        &self,
        args: &GamesStatsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = games_stats_get_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = games_stats_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
