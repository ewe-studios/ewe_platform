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
    games_achievements_increment_builder, games_achievements_increment_task,
    games_achievements_reveal_builder, games_achievements_reveal_task,
    games_achievements_set_steps_at_least_builder, games_achievements_set_steps_at_least_task,
    games_achievements_unlock_builder, games_achievements_unlock_task,
    games_achievements_update_multiple_builder, games_achievements_update_multiple_task,
    games_applications_get_end_point_builder, games_applications_get_end_point_task,
    games_applications_played_builder, games_applications_played_task,
    games_events_record_builder, games_events_record_task,
    games_recall_link_persona_builder, games_recall_link_persona_task,
    games_recall_reset_persona_builder, games_recall_reset_persona_task,
    games_recall_unlink_persona_builder, games_recall_unlink_persona_task,
    games_scores_submit_builder, games_scores_submit_task,
    games_scores_submit_multiple_builder, games_scores_submit_multiple_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::games::AchievementIncrementResponse;
use crate::providers::gcp::clients::games::AchievementRevealResponse;
use crate::providers::gcp::clients::games::AchievementSetStepsAtLeastResponse;
use crate::providers::gcp::clients::games::AchievementUnlockResponse;
use crate::providers::gcp::clients::games::AchievementUpdateMultipleResponse;
use crate::providers::gcp::clients::games::EndPoint;
use crate::providers::gcp::clients::games::EventUpdateResponse;
use crate::providers::gcp::clients::games::GeneratePlayGroupingApiTokenResponse;
use crate::providers::gcp::clients::games::GenerateRecallPlayGroupingApiTokenResponse;
use crate::providers::gcp::clients::games::LinkPersonaResponse;
use crate::providers::gcp::clients::games::PlayerScoreListResponse;
use crate::providers::gcp::clients::games::PlayerScoreResponse;
use crate::providers::gcp::clients::games::ResetPersonaResponse;
use crate::providers::gcp::clients::games::UnlinkPersonaResponse;
use crate::providers::gcp::clients::games::GamesAccesstokensGeneratePlayGroupingApiTokenArgs;
use crate::providers::gcp::clients::games::GamesAccesstokensGenerateRecallPlayGroupingApiTokenArgs;
use crate::providers::gcp::clients::games::GamesAchievementsIncrementArgs;
use crate::providers::gcp::clients::games::GamesAchievementsRevealArgs;
use crate::providers::gcp::clients::games::GamesAchievementsSetStepsAtLeastArgs;
use crate::providers::gcp::clients::games::GamesAchievementsUnlockArgs;
use crate::providers::gcp::clients::games::GamesAchievementsUpdateMultipleArgs;
use crate::providers::gcp::clients::games::GamesApplicationsGetEndPointArgs;
use crate::providers::gcp::clients::games::GamesApplicationsPlayedArgs;
use crate::providers::gcp::clients::games::GamesEventsRecordArgs;
use crate::providers::gcp::clients::games::GamesRecallLinkPersonaArgs;
use crate::providers::gcp::clients::games::GamesRecallResetPersonaArgs;
use crate::providers::gcp::clients::games::GamesRecallUnlinkPersonaArgs;
use crate::providers::gcp::clients::games::GamesScoresSubmitArgs;
use crate::providers::gcp::clients::games::GamesScoresSubmitMultipleArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GamesProvider with automatic state tracking.
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
/// let provider = GamesProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct GamesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> GamesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new GamesProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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
            &args.stepsToIncrement,
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

    /// Games applications get end point.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.score,
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

}
