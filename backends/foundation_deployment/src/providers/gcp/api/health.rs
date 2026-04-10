//! HealthProvider - State-aware health API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       health API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::health::{
    health_users_get_identity_builder, health_users_get_identity_task,
    health_users_get_profile_builder, health_users_get_profile_task,
    health_users_get_settings_builder, health_users_get_settings_task,
    health_users_update_profile_builder, health_users_update_profile_task,
    health_users_update_settings_builder, health_users_update_settings_task,
    health_users_data_types_data_points_batch_delete_builder, health_users_data_types_data_points_batch_delete_task,
    health_users_data_types_data_points_create_builder, health_users_data_types_data_points_create_task,
    health_users_data_types_data_points_daily_roll_up_builder, health_users_data_types_data_points_daily_roll_up_task,
    health_users_data_types_data_points_export_exercise_tcx_builder, health_users_data_types_data_points_export_exercise_tcx_task,
    health_users_data_types_data_points_list_builder, health_users_data_types_data_points_list_task,
    health_users_data_types_data_points_patch_builder, health_users_data_types_data_points_patch_task,
    health_users_data_types_data_points_reconcile_builder, health_users_data_types_data_points_reconcile_task,
    health_users_data_types_data_points_roll_up_builder, health_users_data_types_data_points_roll_up_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::health::DailyRollUpDataPointsResponse;
use crate::providers::gcp::clients::health::ExportExerciseTcxResponse;
use crate::providers::gcp::clients::health::Identity;
use crate::providers::gcp::clients::health::ListDataPointsResponse;
use crate::providers::gcp::clients::health::Operation;
use crate::providers::gcp::clients::health::Profile;
use crate::providers::gcp::clients::health::ReconcileDataPointsResponse;
use crate::providers::gcp::clients::health::RollUpDataPointsResponse;
use crate::providers::gcp::clients::health::Settings;
use crate::providers::gcp::clients::health::HealthUsersDataTypesDataPointsBatchDeleteArgs;
use crate::providers::gcp::clients::health::HealthUsersDataTypesDataPointsCreateArgs;
use crate::providers::gcp::clients::health::HealthUsersDataTypesDataPointsDailyRollUpArgs;
use crate::providers::gcp::clients::health::HealthUsersDataTypesDataPointsExportExerciseTcxArgs;
use crate::providers::gcp::clients::health::HealthUsersDataTypesDataPointsListArgs;
use crate::providers::gcp::clients::health::HealthUsersDataTypesDataPointsPatchArgs;
use crate::providers::gcp::clients::health::HealthUsersDataTypesDataPointsReconcileArgs;
use crate::providers::gcp::clients::health::HealthUsersDataTypesDataPointsRollUpArgs;
use crate::providers::gcp::clients::health::HealthUsersGetIdentityArgs;
use crate::providers::gcp::clients::health::HealthUsersGetProfileArgs;
use crate::providers::gcp::clients::health::HealthUsersGetSettingsArgs;
use crate::providers::gcp::clients::health::HealthUsersUpdateProfileArgs;
use crate::providers::gcp::clients::health::HealthUsersUpdateSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// HealthProvider with automatic state tracking.
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
/// let provider = HealthProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct HealthProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> HealthProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new HealthProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Health users get identity.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Identity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn health_users_get_identity(
        &self,
        args: &HealthUsersGetIdentityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Identity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_get_identity_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_get_identity_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users get profile.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Profile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn health_users_get_profile(
        &self,
        args: &HealthUsersGetProfileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Profile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_get_profile_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_get_profile_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn health_users_get_settings(
        &self,
        args: &HealthUsersGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_get_settings_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users update profile.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Profile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn health_users_update_profile(
        &self,
        args: &HealthUsersUpdateProfileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Profile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_update_profile_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_update_profile_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users update settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Settings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn health_users_update_settings(
        &self,
        args: &HealthUsersUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Settings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_update_settings_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users data types data points batch delete.
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
    pub fn health_users_data_types_data_points_batch_delete(
        &self,
        args: &HealthUsersDataTypesDataPointsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_data_types_data_points_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_data_types_data_points_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users data types data points create.
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
    pub fn health_users_data_types_data_points_create(
        &self,
        args: &HealthUsersDataTypesDataPointsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_data_types_data_points_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_data_types_data_points_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users data types data points daily roll up.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DailyRollUpDataPointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn health_users_data_types_data_points_daily_roll_up(
        &self,
        args: &HealthUsersDataTypesDataPointsDailyRollUpArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DailyRollUpDataPointsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_data_types_data_points_daily_roll_up_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_data_types_data_points_daily_roll_up_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users data types data points export exercise tcx.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExportExerciseTcxResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn health_users_data_types_data_points_export_exercise_tcx(
        &self,
        args: &HealthUsersDataTypesDataPointsExportExerciseTcxArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExportExerciseTcxResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_data_types_data_points_export_exercise_tcx_builder(
            &self.http_client,
            &args.name,
            &args.partialData,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_data_types_data_points_export_exercise_tcx_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users data types data points list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDataPointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn health_users_data_types_data_points_list(
        &self,
        args: &HealthUsersDataTypesDataPointsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDataPointsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_data_types_data_points_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_data_types_data_points_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users data types data points patch.
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
    pub fn health_users_data_types_data_points_patch(
        &self,
        args: &HealthUsersDataTypesDataPointsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_data_types_data_points_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_data_types_data_points_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users data types data points reconcile.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReconcileDataPointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn health_users_data_types_data_points_reconcile(
        &self,
        args: &HealthUsersDataTypesDataPointsReconcileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReconcileDataPointsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_data_types_data_points_reconcile_builder(
            &self.http_client,
            &args.parent,
            &args.dataSourceFamily,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_data_types_data_points_reconcile_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Health users data types data points roll up.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RollUpDataPointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn health_users_data_types_data_points_roll_up(
        &self,
        args: &HealthUsersDataTypesDataPointsRollUpArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RollUpDataPointsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = health_users_data_types_data_points_roll_up_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = health_users_data_types_data_points_roll_up_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
