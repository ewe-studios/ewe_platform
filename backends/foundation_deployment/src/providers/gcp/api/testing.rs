//! TestingProvider - State-aware testing API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       testing API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::testing::{
    testing_application_detail_service_get_apk_details_builder, testing_application_detail_service_get_apk_details_task,
    testing_projects_device_sessions_cancel_builder, testing_projects_device_sessions_cancel_task,
    testing_projects_device_sessions_create_builder, testing_projects_device_sessions_create_task,
    testing_projects_device_sessions_patch_builder, testing_projects_device_sessions_patch_task,
    testing_projects_test_matrices_cancel_builder, testing_projects_test_matrices_cancel_task,
    testing_projects_test_matrices_create_builder, testing_projects_test_matrices_create_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::testing::CancelTestMatrixResponse;
use crate::providers::gcp::clients::testing::DeviceSession;
use crate::providers::gcp::clients::testing::Empty;
use crate::providers::gcp::clients::testing::GetApkDetailsResponse;
use crate::providers::gcp::clients::testing::TestMatrix;
use crate::providers::gcp::clients::testing::TestingApplicationDetailServiceGetApkDetailsArgs;
use crate::providers::gcp::clients::testing::TestingProjectsDeviceSessionsCancelArgs;
use crate::providers::gcp::clients::testing::TestingProjectsDeviceSessionsCreateArgs;
use crate::providers::gcp::clients::testing::TestingProjectsDeviceSessionsPatchArgs;
use crate::providers::gcp::clients::testing::TestingProjectsTestMatricesCancelArgs;
use crate::providers::gcp::clients::testing::TestingProjectsTestMatricesCreateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// TestingProvider with automatic state tracking.
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
/// let provider = TestingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct TestingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> TestingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new TestingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Testing application detail service get apk details.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetApkDetailsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn testing_application_detail_service_get_apk_details(
        &self,
        args: &TestingApplicationDetailServiceGetApkDetailsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetApkDetailsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = testing_application_detail_service_get_apk_details_builder(
            &self.http_client,
            &args.bundleLocation.gcsPath,
        )
        .map_err(ProviderError::Api)?;

        let task = testing_application_detail_service_get_apk_details_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Testing projects device sessions cancel.
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
    pub fn testing_projects_device_sessions_cancel(
        &self,
        args: &TestingProjectsDeviceSessionsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = testing_projects_device_sessions_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = testing_projects_device_sessions_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Testing projects device sessions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeviceSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn testing_projects_device_sessions_create(
        &self,
        args: &TestingProjectsDeviceSessionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeviceSession, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = testing_projects_device_sessions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = testing_projects_device_sessions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Testing projects device sessions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeviceSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn testing_projects_device_sessions_patch(
        &self,
        args: &TestingProjectsDeviceSessionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeviceSession, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = testing_projects_device_sessions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = testing_projects_device_sessions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Testing projects test matrices cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelTestMatrixResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn testing_projects_test_matrices_cancel(
        &self,
        args: &TestingProjectsTestMatricesCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelTestMatrixResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = testing_projects_test_matrices_cancel_builder(
            &self.http_client,
            &args.projectId,
            &args.testMatrixId,
        )
        .map_err(ProviderError::Api)?;

        let task = testing_projects_test_matrices_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Testing projects test matrices create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestMatrix result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn testing_projects_test_matrices_create(
        &self,
        args: &TestingProjectsTestMatricesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestMatrix, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = testing_projects_test_matrices_create_builder(
            &self.http_client,
            &args.projectId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = testing_projects_test_matrices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
