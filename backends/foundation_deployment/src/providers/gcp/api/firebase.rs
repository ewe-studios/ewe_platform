//! FirebaseProvider - State-aware firebase API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebase API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebase::{
    firebase_available_projects_list_builder, firebase_available_projects_list_task,
    firebase_operations_get_builder, firebase_operations_get_task,
    firebase_projects_add_firebase_builder, firebase_projects_add_firebase_task,
    firebase_projects_add_google_analytics_builder, firebase_projects_add_google_analytics_task,
    firebase_projects_get_builder, firebase_projects_get_task,
    firebase_projects_get_admin_sdk_config_builder, firebase_projects_get_admin_sdk_config_task,
    firebase_projects_get_analytics_details_builder, firebase_projects_get_analytics_details_task,
    firebase_projects_list_builder, firebase_projects_list_task,
    firebase_projects_patch_builder, firebase_projects_patch_task,
    firebase_projects_remove_analytics_builder, firebase_projects_remove_analytics_task,
    firebase_projects_search_apps_builder, firebase_projects_search_apps_task,
    firebase_projects_android_apps_create_builder, firebase_projects_android_apps_create_task,
    firebase_projects_android_apps_get_builder, firebase_projects_android_apps_get_task,
    firebase_projects_android_apps_get_config_builder, firebase_projects_android_apps_get_config_task,
    firebase_projects_android_apps_list_builder, firebase_projects_android_apps_list_task,
    firebase_projects_android_apps_patch_builder, firebase_projects_android_apps_patch_task,
    firebase_projects_android_apps_remove_builder, firebase_projects_android_apps_remove_task,
    firebase_projects_android_apps_undelete_builder, firebase_projects_android_apps_undelete_task,
    firebase_projects_android_apps_sha_create_builder, firebase_projects_android_apps_sha_create_task,
    firebase_projects_android_apps_sha_delete_builder, firebase_projects_android_apps_sha_delete_task,
    firebase_projects_android_apps_sha_list_builder, firebase_projects_android_apps_sha_list_task,
    firebase_projects_available_locations_list_builder, firebase_projects_available_locations_list_task,
    firebase_projects_default_location_finalize_builder, firebase_projects_default_location_finalize_task,
    firebase_projects_ios_apps_create_builder, firebase_projects_ios_apps_create_task,
    firebase_projects_ios_apps_get_builder, firebase_projects_ios_apps_get_task,
    firebase_projects_ios_apps_get_config_builder, firebase_projects_ios_apps_get_config_task,
    firebase_projects_ios_apps_list_builder, firebase_projects_ios_apps_list_task,
    firebase_projects_ios_apps_patch_builder, firebase_projects_ios_apps_patch_task,
    firebase_projects_ios_apps_remove_builder, firebase_projects_ios_apps_remove_task,
    firebase_projects_ios_apps_undelete_builder, firebase_projects_ios_apps_undelete_task,
    firebase_projects_web_apps_create_builder, firebase_projects_web_apps_create_task,
    firebase_projects_web_apps_get_builder, firebase_projects_web_apps_get_task,
    firebase_projects_web_apps_get_config_builder, firebase_projects_web_apps_get_config_task,
    firebase_projects_web_apps_list_builder, firebase_projects_web_apps_list_task,
    firebase_projects_web_apps_patch_builder, firebase_projects_web_apps_patch_task,
    firebase_projects_web_apps_remove_builder, firebase_projects_web_apps_remove_task,
    firebase_projects_web_apps_undelete_builder, firebase_projects_web_apps_undelete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebase::AdminSdkConfig;
use crate::providers::gcp::clients::firebase::AnalyticsDetails;
use crate::providers::gcp::clients::firebase::AndroidApp;
use crate::providers::gcp::clients::firebase::AndroidAppConfig;
use crate::providers::gcp::clients::firebase::Empty;
use crate::providers::gcp::clients::firebase::FirebaseProject;
use crate::providers::gcp::clients::firebase::IosApp;
use crate::providers::gcp::clients::firebase::IosAppConfig;
use crate::providers::gcp::clients::firebase::ListAndroidAppsResponse;
use crate::providers::gcp::clients::firebase::ListAvailableLocationsResponse;
use crate::providers::gcp::clients::firebase::ListAvailableProjectsResponse;
use crate::providers::gcp::clients::firebase::ListFirebaseProjectsResponse;
use crate::providers::gcp::clients::firebase::ListIosAppsResponse;
use crate::providers::gcp::clients::firebase::ListShaCertificatesResponse;
use crate::providers::gcp::clients::firebase::ListWebAppsResponse;
use crate::providers::gcp::clients::firebase::Operation;
use crate::providers::gcp::clients::firebase::SearchFirebaseAppsResponse;
use crate::providers::gcp::clients::firebase::ShaCertificate;
use crate::providers::gcp::clients::firebase::WebApp;
use crate::providers::gcp::clients::firebase::WebAppConfig;
use crate::providers::gcp::clients::firebase::FirebaseAvailableProjectsListArgs;
use crate::providers::gcp::clients::firebase::FirebaseOperationsGetArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAddFirebaseArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAddGoogleAnalyticsArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsCreateArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsGetArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsGetConfigArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsListArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsPatchArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsRemoveArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsShaCreateArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsShaDeleteArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsShaListArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAndroidAppsUndeleteArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsAvailableLocationsListArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsDefaultLocationFinalizeArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsGetAdminSdkConfigArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsGetAnalyticsDetailsArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsGetArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsIosAppsCreateArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsIosAppsGetArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsIosAppsGetConfigArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsIosAppsListArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsIosAppsPatchArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsIosAppsRemoveArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsIosAppsUndeleteArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsListArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsPatchArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsRemoveAnalyticsArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsSearchAppsArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsWebAppsCreateArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsWebAppsGetArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsWebAppsGetConfigArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsWebAppsListArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsWebAppsPatchArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsWebAppsRemoveArgs;
use crate::providers::gcp::clients::firebase::FirebaseProjectsWebAppsUndeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebaseProvider with automatic state tracking.
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
/// let provider = FirebaseProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebaseProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebase available projects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAvailableProjectsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_available_projects_list(
        &self,
        args: &FirebaseAvailableProjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAvailableProjectsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_available_projects_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_available_projects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase operations get.
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
    pub fn firebase_operations_get(
        &self,
        args: &FirebaseOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects add firebase.
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
    pub fn firebase_projects_add_firebase(
        &self,
        args: &FirebaseProjectsAddFirebaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_add_firebase_builder(
            &self.http_client,
            &args.project,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_add_firebase_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects add google analytics.
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
    pub fn firebase_projects_add_google_analytics(
        &self,
        args: &FirebaseProjectsAddGoogleAnalyticsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_add_google_analytics_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_add_google_analytics_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FirebaseProject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_get(
        &self,
        args: &FirebaseProjectsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FirebaseProject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects get admin sdk config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdminSdkConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_get_admin_sdk_config(
        &self,
        args: &FirebaseProjectsGetAdminSdkConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdminSdkConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_get_admin_sdk_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_get_admin_sdk_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects get analytics details.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyticsDetails result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_get_analytics_details(
        &self,
        args: &FirebaseProjectsGetAnalyticsDetailsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyticsDetails, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_get_analytics_details_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_get_analytics_details_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFirebaseProjectsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_list(
        &self,
        args: &FirebaseProjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFirebaseProjectsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FirebaseProject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebase_projects_patch(
        &self,
        args: &FirebaseProjectsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FirebaseProject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects remove analytics.
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
    pub fn firebase_projects_remove_analytics(
        &self,
        args: &FirebaseProjectsRemoveAnalyticsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_remove_analytics_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_remove_analytics_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects search apps.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchFirebaseAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_search_apps(
        &self,
        args: &FirebaseProjectsSearchAppsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchFirebaseAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_search_apps_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_search_apps_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps create.
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
    pub fn firebase_projects_android_apps_create(
        &self,
        args: &FirebaseProjectsAndroidAppsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AndroidApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_android_apps_get(
        &self,
        args: &FirebaseProjectsAndroidAppsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AndroidApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps get config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AndroidAppConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_android_apps_get_config(
        &self,
        args: &FirebaseProjectsAndroidAppsGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AndroidAppConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAndroidAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_android_apps_list(
        &self,
        args: &FirebaseProjectsAndroidAppsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAndroidAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AndroidApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebase_projects_android_apps_patch(
        &self,
        args: &FirebaseProjectsAndroidAppsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AndroidApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps remove.
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
    pub fn firebase_projects_android_apps_remove(
        &self,
        args: &FirebaseProjectsAndroidAppsRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_remove_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps undelete.
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
    pub fn firebase_projects_android_apps_undelete(
        &self,
        args: &FirebaseProjectsAndroidAppsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps sha create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShaCertificate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebase_projects_android_apps_sha_create(
        &self,
        args: &FirebaseProjectsAndroidAppsShaCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShaCertificate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_sha_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_sha_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps sha delete.
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
    pub fn firebase_projects_android_apps_sha_delete(
        &self,
        args: &FirebaseProjectsAndroidAppsShaDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_sha_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_sha_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects android apps sha list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListShaCertificatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_android_apps_sha_list(
        &self,
        args: &FirebaseProjectsAndroidAppsShaListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListShaCertificatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_android_apps_sha_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_android_apps_sha_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects available locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAvailableLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_available_locations_list(
        &self,
        args: &FirebaseProjectsAvailableLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAvailableLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_available_locations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_available_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects default location finalize.
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
    pub fn firebase_projects_default_location_finalize(
        &self,
        args: &FirebaseProjectsDefaultLocationFinalizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_default_location_finalize_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_default_location_finalize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects ios apps create.
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
    pub fn firebase_projects_ios_apps_create(
        &self,
        args: &FirebaseProjectsIosAppsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_ios_apps_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_ios_apps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects ios apps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IosApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_ios_apps_get(
        &self,
        args: &FirebaseProjectsIosAppsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IosApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_ios_apps_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_ios_apps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects ios apps get config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IosAppConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_ios_apps_get_config(
        &self,
        args: &FirebaseProjectsIosAppsGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IosAppConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_ios_apps_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_ios_apps_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects ios apps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListIosAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_ios_apps_list(
        &self,
        args: &FirebaseProjectsIosAppsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListIosAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_ios_apps_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_ios_apps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects ios apps patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IosApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebase_projects_ios_apps_patch(
        &self,
        args: &FirebaseProjectsIosAppsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IosApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_ios_apps_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_ios_apps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects ios apps remove.
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
    pub fn firebase_projects_ios_apps_remove(
        &self,
        args: &FirebaseProjectsIosAppsRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_ios_apps_remove_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_ios_apps_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects ios apps undelete.
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
    pub fn firebase_projects_ios_apps_undelete(
        &self,
        args: &FirebaseProjectsIosAppsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_ios_apps_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_ios_apps_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects web apps create.
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
    pub fn firebase_projects_web_apps_create(
        &self,
        args: &FirebaseProjectsWebAppsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_web_apps_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_web_apps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects web apps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_web_apps_get(
        &self,
        args: &FirebaseProjectsWebAppsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_web_apps_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_web_apps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects web apps get config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebAppConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_web_apps_get_config(
        &self,
        args: &FirebaseProjectsWebAppsGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebAppConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_web_apps_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_web_apps_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects web apps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWebAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebase_projects_web_apps_list(
        &self,
        args: &FirebaseProjectsWebAppsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWebAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_web_apps_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_web_apps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects web apps patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WebApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebase_projects_web_apps_patch(
        &self,
        args: &FirebaseProjectsWebAppsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WebApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_web_apps_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_web_apps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects web apps remove.
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
    pub fn firebase_projects_web_apps_remove(
        &self,
        args: &FirebaseProjectsWebAppsRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_web_apps_remove_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_web_apps_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebase projects web apps undelete.
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
    pub fn firebase_projects_web_apps_undelete(
        &self,
        args: &FirebaseProjectsWebAppsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebase_projects_web_apps_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebase_projects_web_apps_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
