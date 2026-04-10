//! ContaineranalysisProvider - State-aware containeranalysis API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       containeranalysis API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::containeranalysis::{
    containeranalysis_projects_locations_notes_batch_create_builder, containeranalysis_projects_locations_notes_batch_create_task,
    containeranalysis_projects_locations_notes_create_builder, containeranalysis_projects_locations_notes_create_task,
    containeranalysis_projects_locations_notes_delete_builder, containeranalysis_projects_locations_notes_delete_task,
    containeranalysis_projects_locations_notes_get_iam_policy_builder, containeranalysis_projects_locations_notes_get_iam_policy_task,
    containeranalysis_projects_locations_notes_patch_builder, containeranalysis_projects_locations_notes_patch_task,
    containeranalysis_projects_locations_notes_set_iam_policy_builder, containeranalysis_projects_locations_notes_set_iam_policy_task,
    containeranalysis_projects_locations_notes_test_iam_permissions_builder, containeranalysis_projects_locations_notes_test_iam_permissions_task,
    containeranalysis_projects_locations_occurrences_batch_create_builder, containeranalysis_projects_locations_occurrences_batch_create_task,
    containeranalysis_projects_locations_occurrences_create_builder, containeranalysis_projects_locations_occurrences_create_task,
    containeranalysis_projects_locations_occurrences_delete_builder, containeranalysis_projects_locations_occurrences_delete_task,
    containeranalysis_projects_locations_occurrences_get_iam_policy_builder, containeranalysis_projects_locations_occurrences_get_iam_policy_task,
    containeranalysis_projects_locations_occurrences_patch_builder, containeranalysis_projects_locations_occurrences_patch_task,
    containeranalysis_projects_locations_occurrences_set_iam_policy_builder, containeranalysis_projects_locations_occurrences_set_iam_policy_task,
    containeranalysis_projects_locations_occurrences_test_iam_permissions_builder, containeranalysis_projects_locations_occurrences_test_iam_permissions_task,
    containeranalysis_projects_locations_resources_export_s_b_o_m_builder, containeranalysis_projects_locations_resources_export_s_b_o_m_task,
    containeranalysis_projects_notes_batch_create_builder, containeranalysis_projects_notes_batch_create_task,
    containeranalysis_projects_notes_create_builder, containeranalysis_projects_notes_create_task,
    containeranalysis_projects_notes_delete_builder, containeranalysis_projects_notes_delete_task,
    containeranalysis_projects_notes_get_iam_policy_builder, containeranalysis_projects_notes_get_iam_policy_task,
    containeranalysis_projects_notes_patch_builder, containeranalysis_projects_notes_patch_task,
    containeranalysis_projects_notes_set_iam_policy_builder, containeranalysis_projects_notes_set_iam_policy_task,
    containeranalysis_projects_notes_test_iam_permissions_builder, containeranalysis_projects_notes_test_iam_permissions_task,
    containeranalysis_projects_occurrences_batch_create_builder, containeranalysis_projects_occurrences_batch_create_task,
    containeranalysis_projects_occurrences_create_builder, containeranalysis_projects_occurrences_create_task,
    containeranalysis_projects_occurrences_delete_builder, containeranalysis_projects_occurrences_delete_task,
    containeranalysis_projects_occurrences_get_iam_policy_builder, containeranalysis_projects_occurrences_get_iam_policy_task,
    containeranalysis_projects_occurrences_patch_builder, containeranalysis_projects_occurrences_patch_task,
    containeranalysis_projects_occurrences_set_iam_policy_builder, containeranalysis_projects_occurrences_set_iam_policy_task,
    containeranalysis_projects_occurrences_test_iam_permissions_builder, containeranalysis_projects_occurrences_test_iam_permissions_task,
    containeranalysis_projects_resources_export_s_b_o_m_builder, containeranalysis_projects_resources_export_s_b_o_m_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::containeranalysis::BatchCreateNotesResponse;
use crate::providers::gcp::clients::containeranalysis::BatchCreateOccurrencesResponse;
use crate::providers::gcp::clients::containeranalysis::Empty;
use crate::providers::gcp::clients::containeranalysis::ExportSBOMResponse;
use crate::providers::gcp::clients::containeranalysis::Note;
use crate::providers::gcp::clients::containeranalysis::Occurrence;
use crate::providers::gcp::clients::containeranalysis::Policy;
use crate::providers::gcp::clients::containeranalysis::TestIamPermissionsResponse;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsNotesBatchCreateArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsNotesCreateArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsNotesDeleteArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsNotesGetIamPolicyArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsNotesPatchArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsNotesSetIamPolicyArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsNotesTestIamPermissionsArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsOccurrencesBatchCreateArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsOccurrencesCreateArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsOccurrencesDeleteArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsOccurrencesGetIamPolicyArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsOccurrencesPatchArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsOccurrencesSetIamPolicyArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsOccurrencesTestIamPermissionsArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsLocationsResourcesExportSBOMArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsNotesBatchCreateArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsNotesCreateArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsNotesDeleteArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsNotesGetIamPolicyArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsNotesPatchArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsNotesSetIamPolicyArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsNotesTestIamPermissionsArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsOccurrencesBatchCreateArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsOccurrencesCreateArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsOccurrencesDeleteArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsOccurrencesGetIamPolicyArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsOccurrencesPatchArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsOccurrencesSetIamPolicyArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsOccurrencesTestIamPermissionsArgs;
use crate::providers::gcp::clients::containeranalysis::ContaineranalysisProjectsResourcesExportSBOMArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ContaineranalysisProvider with automatic state tracking.
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
/// let provider = ContaineranalysisProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ContaineranalysisProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ContaineranalysisProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ContaineranalysisProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Containeranalysis projects locations notes batch create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchCreateNotesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_notes_batch_create(
        &self,
        args: &ContaineranalysisProjectsLocationsNotesBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchCreateNotesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_notes_batch_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_notes_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations notes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Note result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_notes_create(
        &self,
        args: &ContaineranalysisProjectsLocationsNotesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Note, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_notes_create_builder(
            &self.http_client,
            &args.parent,
            &args.noteId,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_notes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations notes delete.
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
    pub fn containeranalysis_projects_locations_notes_delete(
        &self,
        args: &ContaineranalysisProjectsLocationsNotesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_notes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_notes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations notes get iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_notes_get_iam_policy(
        &self,
        args: &ContaineranalysisProjectsLocationsNotesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_notes_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_notes_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations notes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Note result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_notes_patch(
        &self,
        args: &ContaineranalysisProjectsLocationsNotesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Note, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_notes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_notes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations notes set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_notes_set_iam_policy(
        &self,
        args: &ContaineranalysisProjectsLocationsNotesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_notes_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_notes_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations notes test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_notes_test_iam_permissions(
        &self,
        args: &ContaineranalysisProjectsLocationsNotesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_notes_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_notes_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations occurrences batch create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchCreateOccurrencesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_occurrences_batch_create(
        &self,
        args: &ContaineranalysisProjectsLocationsOccurrencesBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchCreateOccurrencesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_occurrences_batch_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_occurrences_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations occurrences create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Occurrence result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_occurrences_create(
        &self,
        args: &ContaineranalysisProjectsLocationsOccurrencesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Occurrence, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_occurrences_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_occurrences_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations occurrences delete.
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
    pub fn containeranalysis_projects_locations_occurrences_delete(
        &self,
        args: &ContaineranalysisProjectsLocationsOccurrencesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_occurrences_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_occurrences_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations occurrences get iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_occurrences_get_iam_policy(
        &self,
        args: &ContaineranalysisProjectsLocationsOccurrencesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_occurrences_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_occurrences_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations occurrences patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Occurrence result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_occurrences_patch(
        &self,
        args: &ContaineranalysisProjectsLocationsOccurrencesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Occurrence, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_occurrences_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_occurrences_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations occurrences set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_occurrences_set_iam_policy(
        &self,
        args: &ContaineranalysisProjectsLocationsOccurrencesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_occurrences_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_occurrences_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations occurrences test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_occurrences_test_iam_permissions(
        &self,
        args: &ContaineranalysisProjectsLocationsOccurrencesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_occurrences_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_occurrences_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects locations resources export s b o m.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExportSBOMResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_locations_resources_export_s_b_o_m(
        &self,
        args: &ContaineranalysisProjectsLocationsResourcesExportSBOMArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExportSBOMResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_locations_resources_export_s_b_o_m_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_locations_resources_export_s_b_o_m_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects notes batch create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchCreateNotesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_notes_batch_create(
        &self,
        args: &ContaineranalysisProjectsNotesBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchCreateNotesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_notes_batch_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_notes_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects notes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Note result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_notes_create(
        &self,
        args: &ContaineranalysisProjectsNotesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Note, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_notes_create_builder(
            &self.http_client,
            &args.parent,
            &args.noteId,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_notes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects notes delete.
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
    pub fn containeranalysis_projects_notes_delete(
        &self,
        args: &ContaineranalysisProjectsNotesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_notes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_notes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects notes get iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_notes_get_iam_policy(
        &self,
        args: &ContaineranalysisProjectsNotesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_notes_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_notes_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects notes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Note result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_notes_patch(
        &self,
        args: &ContaineranalysisProjectsNotesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Note, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_notes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_notes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects notes set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_notes_set_iam_policy(
        &self,
        args: &ContaineranalysisProjectsNotesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_notes_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_notes_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects notes test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_notes_test_iam_permissions(
        &self,
        args: &ContaineranalysisProjectsNotesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_notes_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_notes_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects occurrences batch create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchCreateOccurrencesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_occurrences_batch_create(
        &self,
        args: &ContaineranalysisProjectsOccurrencesBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchCreateOccurrencesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_occurrences_batch_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_occurrences_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects occurrences create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Occurrence result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_occurrences_create(
        &self,
        args: &ContaineranalysisProjectsOccurrencesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Occurrence, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_occurrences_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_occurrences_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects occurrences delete.
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
    pub fn containeranalysis_projects_occurrences_delete(
        &self,
        args: &ContaineranalysisProjectsOccurrencesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_occurrences_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_occurrences_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects occurrences get iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_occurrences_get_iam_policy(
        &self,
        args: &ContaineranalysisProjectsOccurrencesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_occurrences_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_occurrences_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects occurrences patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Occurrence result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_occurrences_patch(
        &self,
        args: &ContaineranalysisProjectsOccurrencesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Occurrence, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_occurrences_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_occurrences_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects occurrences set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_occurrences_set_iam_policy(
        &self,
        args: &ContaineranalysisProjectsOccurrencesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_occurrences_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_occurrences_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects occurrences test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_occurrences_test_iam_permissions(
        &self,
        args: &ContaineranalysisProjectsOccurrencesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_occurrences_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_occurrences_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Containeranalysis projects resources export s b o m.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExportSBOMResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn containeranalysis_projects_resources_export_s_b_o_m(
        &self,
        args: &ContaineranalysisProjectsResourcesExportSBOMArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExportSBOMResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = containeranalysis_projects_resources_export_s_b_o_m_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = containeranalysis_projects_resources_export_s_b_o_m_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
