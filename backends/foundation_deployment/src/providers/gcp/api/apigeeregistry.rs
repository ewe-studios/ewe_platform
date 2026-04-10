//! ApigeeregistryProvider - State-aware apigeeregistry API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       apigeeregistry API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::apigeeregistry::{
    apigeeregistry_projects_locations_apis_create_builder, apigeeregistry_projects_locations_apis_create_task,
    apigeeregistry_projects_locations_apis_delete_builder, apigeeregistry_projects_locations_apis_delete_task,
    apigeeregistry_projects_locations_apis_patch_builder, apigeeregistry_projects_locations_apis_patch_task,
    apigeeregistry_projects_locations_apis_set_iam_policy_builder, apigeeregistry_projects_locations_apis_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_artifacts_create_builder, apigeeregistry_projects_locations_apis_artifacts_create_task,
    apigeeregistry_projects_locations_apis_artifacts_delete_builder, apigeeregistry_projects_locations_apis_artifacts_delete_task,
    apigeeregistry_projects_locations_apis_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_apis_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_apis_artifacts_set_iam_policy_builder, apigeeregistry_projects_locations_apis_artifacts_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_deployments_create_builder, apigeeregistry_projects_locations_apis_deployments_create_task,
    apigeeregistry_projects_locations_apis_deployments_delete_builder, apigeeregistry_projects_locations_apis_deployments_delete_task,
    apigeeregistry_projects_locations_apis_deployments_delete_revision_builder, apigeeregistry_projects_locations_apis_deployments_delete_revision_task,
    apigeeregistry_projects_locations_apis_deployments_patch_builder, apigeeregistry_projects_locations_apis_deployments_patch_task,
    apigeeregistry_projects_locations_apis_deployments_rollback_builder, apigeeregistry_projects_locations_apis_deployments_rollback_task,
    apigeeregistry_projects_locations_apis_deployments_set_iam_policy_builder, apigeeregistry_projects_locations_apis_deployments_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_deployments_tag_revision_builder, apigeeregistry_projects_locations_apis_deployments_tag_revision_task,
    apigeeregistry_projects_locations_apis_deployments_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_deployments_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_create_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_create_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_delete_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_delete_task,
    apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_apis_versions_create_builder, apigeeregistry_projects_locations_apis_versions_create_task,
    apigeeregistry_projects_locations_apis_versions_delete_builder, apigeeregistry_projects_locations_apis_versions_delete_task,
    apigeeregistry_projects_locations_apis_versions_patch_builder, apigeeregistry_projects_locations_apis_versions_patch_task,
    apigeeregistry_projects_locations_apis_versions_set_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_versions_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_create_builder, apigeeregistry_projects_locations_apis_versions_artifacts_create_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_delete_builder, apigeeregistry_projects_locations_apis_versions_artifacts_delete_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_versions_specs_create_builder, apigeeregistry_projects_locations_apis_versions_specs_create_task,
    apigeeregistry_projects_locations_apis_versions_specs_delete_builder, apigeeregistry_projects_locations_apis_versions_specs_delete_task,
    apigeeregistry_projects_locations_apis_versions_specs_delete_revision_builder, apigeeregistry_projects_locations_apis_versions_specs_delete_revision_task,
    apigeeregistry_projects_locations_apis_versions_specs_patch_builder, apigeeregistry_projects_locations_apis_versions_specs_patch_task,
    apigeeregistry_projects_locations_apis_versions_specs_rollback_builder, apigeeregistry_projects_locations_apis_versions_specs_rollback_task,
    apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_specs_tag_revision_builder, apigeeregistry_projects_locations_apis_versions_specs_tag_revision_task,
    apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_create_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_create_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy_task,
    apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions_builder, apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions_task,
    apigeeregistry_projects_locations_artifacts_create_builder, apigeeregistry_projects_locations_artifacts_create_task,
    apigeeregistry_projects_locations_artifacts_delete_builder, apigeeregistry_projects_locations_artifacts_delete_task,
    apigeeregistry_projects_locations_artifacts_replace_artifact_builder, apigeeregistry_projects_locations_artifacts_replace_artifact_task,
    apigeeregistry_projects_locations_artifacts_set_iam_policy_builder, apigeeregistry_projects_locations_artifacts_set_iam_policy_task,
    apigeeregistry_projects_locations_artifacts_test_iam_permissions_builder, apigeeregistry_projects_locations_artifacts_test_iam_permissions_task,
    apigeeregistry_projects_locations_documents_set_iam_policy_builder, apigeeregistry_projects_locations_documents_set_iam_policy_task,
    apigeeregistry_projects_locations_documents_test_iam_permissions_builder, apigeeregistry_projects_locations_documents_test_iam_permissions_task,
    apigeeregistry_projects_locations_instances_create_builder, apigeeregistry_projects_locations_instances_create_task,
    apigeeregistry_projects_locations_instances_delete_builder, apigeeregistry_projects_locations_instances_delete_task,
    apigeeregistry_projects_locations_instances_set_iam_policy_builder, apigeeregistry_projects_locations_instances_set_iam_policy_task,
    apigeeregistry_projects_locations_instances_test_iam_permissions_builder, apigeeregistry_projects_locations_instances_test_iam_permissions_task,
    apigeeregistry_projects_locations_operations_cancel_builder, apigeeregistry_projects_locations_operations_cancel_task,
    apigeeregistry_projects_locations_operations_delete_builder, apigeeregistry_projects_locations_operations_delete_task,
    apigeeregistry_projects_locations_runtime_set_iam_policy_builder, apigeeregistry_projects_locations_runtime_set_iam_policy_task,
    apigeeregistry_projects_locations_runtime_test_iam_permissions_builder, apigeeregistry_projects_locations_runtime_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::apigeeregistry::Api;
use crate::providers::gcp::clients::apigeeregistry::ApiDeployment;
use crate::providers::gcp::clients::apigeeregistry::ApiSpec;
use crate::providers::gcp::clients::apigeeregistry::ApiVersion;
use crate::providers::gcp::clients::apigeeregistry::Artifact;
use crate::providers::gcp::clients::apigeeregistry::Empty;
use crate::providers::gcp::clients::apigeeregistry::Operation;
use crate::providers::gcp::clients::apigeeregistry::Policy;
use crate::providers::gcp::clients::apigeeregistry::TestIamPermissionsResponse;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisArtifactsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsDeleteRevisionArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsPatchArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsRollbackArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsTagRevisionArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisDeploymentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisPatchArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsArtifactsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsPatchArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsDeleteRevisionArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsPatchArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsRollbackArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsTagRevisionArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsSpecsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsApisVersionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsReplaceArtifactArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsArtifactsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsDocumentsSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsDocumentsTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesCreateArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsInstancesTestIamPermissionsArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsRuntimeSetIamPolicyArgs;
use crate::providers::gcp::clients::apigeeregistry::ApigeeregistryProjectsLocationsRuntimeTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ApigeeregistryProvider with automatic state tracking.
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
/// let provider = ApigeeregistryProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ApigeeregistryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ApigeeregistryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ApigeeregistryProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Apigeeregistry projects locations apis create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Api result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Api, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis delete.
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
    pub fn apigeeregistry_projects_locations_apis_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Api result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_patch(
        &self,
        args: &ApigeeregistryProjectsLocationsApisPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Api, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts delete.
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
    pub fn apigeeregistry_projects_locations_apis_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_artifacts_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis artifacts test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisArtifactsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_artifacts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiDeploymentId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments delete.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments delete revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_delete_revision(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsDeleteRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_delete_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_delete_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_patch(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments rollback.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_rollback(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments tag revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_tag_revision(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsTagRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_tag_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_tag_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts delete.
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
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis deployments artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsApisDeploymentsArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_deployments_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiVersionId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions delete.
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
    pub fn apigeeregistry_projects_locations_apis_versions_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_patch(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_versions_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts delete.
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
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions artifacts test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsArtifactsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_artifacts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_create_builder(
            &self.http_client,
            &args.parent,
            &args.apiSpecId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs delete.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs delete revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_delete_revision(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsDeleteRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_delete_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_delete_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_patch(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs rollback.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_rollback(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs tag revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiSpec result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_tag_revision(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsTagRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiSpec, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_tag_revision_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_tag_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts delete.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts set iam policy.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations apis versions specs artifacts test iam permissions.
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
    pub fn apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsApisVersionsSpecsArtifactsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_apis_versions_specs_artifacts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_artifacts_create(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_create_builder(
            &self.http_client,
            &args.parent,
            &args.artifactId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts delete.
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
    pub fn apigeeregistry_projects_locations_artifacts_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts replace artifact.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Artifact result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apigeeregistry_projects_locations_artifacts_replace_artifact(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsReplaceArtifactArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Artifact, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_replace_artifact_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_replace_artifact_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts set iam policy.
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
    pub fn apigeeregistry_projects_locations_artifacts_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations artifacts test iam permissions.
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
    pub fn apigeeregistry_projects_locations_artifacts_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsArtifactsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_artifacts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_artifacts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations documents set iam policy.
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
    pub fn apigeeregistry_projects_locations_documents_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsDocumentsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_documents_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_documents_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations documents test iam permissions.
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
    pub fn apigeeregistry_projects_locations_documents_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsDocumentsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_documents_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_documents_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances create.
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
    pub fn apigeeregistry_projects_locations_instances_create(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.instanceId,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances delete.
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
    pub fn apigeeregistry_projects_locations_instances_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances set iam policy.
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
    pub fn apigeeregistry_projects_locations_instances_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations instances test iam permissions.
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
    pub fn apigeeregistry_projects_locations_instances_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsInstancesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_instances_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_instances_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations operations cancel.
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
    pub fn apigeeregistry_projects_locations_operations_cancel(
        &self,
        args: &ApigeeregistryProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations operations delete.
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
    pub fn apigeeregistry_projects_locations_operations_delete(
        &self,
        args: &ApigeeregistryProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations runtime set iam policy.
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
    pub fn apigeeregistry_projects_locations_runtime_set_iam_policy(
        &self,
        args: &ApigeeregistryProjectsLocationsRuntimeSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_runtime_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_runtime_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apigeeregistry projects locations runtime test iam permissions.
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
    pub fn apigeeregistry_projects_locations_runtime_test_iam_permissions(
        &self,
        args: &ApigeeregistryProjectsLocationsRuntimeTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apigeeregistry_projects_locations_runtime_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = apigeeregistry_projects_locations_runtime_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
