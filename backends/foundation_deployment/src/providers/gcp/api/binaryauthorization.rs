//! BinaryauthorizationProvider - State-aware binaryauthorization API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       binaryauthorization API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::binaryauthorization::{
    binaryauthorization_projects_update_policy_builder, binaryauthorization_projects_update_policy_task,
    binaryauthorization_projects_attestors_create_builder, binaryauthorization_projects_attestors_create_task,
    binaryauthorization_projects_attestors_delete_builder, binaryauthorization_projects_attestors_delete_task,
    binaryauthorization_projects_attestors_set_iam_policy_builder, binaryauthorization_projects_attestors_set_iam_policy_task,
    binaryauthorization_projects_attestors_test_iam_permissions_builder, binaryauthorization_projects_attestors_test_iam_permissions_task,
    binaryauthorization_projects_attestors_update_builder, binaryauthorization_projects_attestors_update_task,
    binaryauthorization_projects_attestors_validate_attestation_occurrence_builder, binaryauthorization_projects_attestors_validate_attestation_occurrence_task,
    binaryauthorization_projects_platforms_gke_policies_evaluate_builder, binaryauthorization_projects_platforms_gke_policies_evaluate_task,
    binaryauthorization_projects_platforms_policies_create_builder, binaryauthorization_projects_platforms_policies_create_task,
    binaryauthorization_projects_platforms_policies_delete_builder, binaryauthorization_projects_platforms_policies_delete_task,
    binaryauthorization_projects_platforms_policies_replace_platform_policy_builder, binaryauthorization_projects_platforms_policies_replace_platform_policy_task,
    binaryauthorization_projects_policy_set_iam_policy_builder, binaryauthorization_projects_policy_set_iam_policy_task,
    binaryauthorization_projects_policy_test_iam_permissions_builder, binaryauthorization_projects_policy_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::binaryauthorization::Attestor;
use crate::providers::gcp::clients::binaryauthorization::Empty;
use crate::providers::gcp::clients::binaryauthorization::EvaluateGkePolicyResponse;
use crate::providers::gcp::clients::binaryauthorization::IamPolicy;
use crate::providers::gcp::clients::binaryauthorization::PlatformPolicy;
use crate::providers::gcp::clients::binaryauthorization::Policy;
use crate::providers::gcp::clients::binaryauthorization::TestIamPermissionsResponse;
use crate::providers::gcp::clients::binaryauthorization::ValidateAttestationOccurrenceResponse;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsAttestorsCreateArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsAttestorsDeleteArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsAttestorsSetIamPolicyArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsAttestorsTestIamPermissionsArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsAttestorsUpdateArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsAttestorsValidateAttestationOccurrenceArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsPlatformsGkePoliciesEvaluateArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsPlatformsPoliciesCreateArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsPlatformsPoliciesDeleteArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsPlatformsPoliciesReplacePlatformPolicyArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsPolicySetIamPolicyArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsPolicyTestIamPermissionsArgs;
use crate::providers::gcp::clients::binaryauthorization::BinaryauthorizationProjectsUpdatePolicyArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BinaryauthorizationProvider with automatic state tracking.
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
/// let provider = BinaryauthorizationProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BinaryauthorizationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BinaryauthorizationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BinaryauthorizationProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Binaryauthorization projects update policy.
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
    pub fn binaryauthorization_projects_update_policy(
        &self,
        args: &BinaryauthorizationProjectsUpdatePolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_update_policy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_update_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects attestors create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Attestor result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn binaryauthorization_projects_attestors_create(
        &self,
        args: &BinaryauthorizationProjectsAttestorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Attestor, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_attestors_create_builder(
            &self.http_client,
            &args.parent,
            &args.attestorId,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_attestors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects attestors delete.
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
    pub fn binaryauthorization_projects_attestors_delete(
        &self,
        args: &BinaryauthorizationProjectsAttestorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_attestors_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_attestors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects attestors set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IamPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn binaryauthorization_projects_attestors_set_iam_policy(
        &self,
        args: &BinaryauthorizationProjectsAttestorsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IamPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_attestors_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_attestors_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects attestors test iam permissions.
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
    pub fn binaryauthorization_projects_attestors_test_iam_permissions(
        &self,
        args: &BinaryauthorizationProjectsAttestorsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_attestors_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_attestors_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects attestors update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Attestor result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn binaryauthorization_projects_attestors_update(
        &self,
        args: &BinaryauthorizationProjectsAttestorsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Attestor, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_attestors_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_attestors_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects attestors validate attestation occurrence.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValidateAttestationOccurrenceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn binaryauthorization_projects_attestors_validate_attestation_occurrence(
        &self,
        args: &BinaryauthorizationProjectsAttestorsValidateAttestationOccurrenceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValidateAttestationOccurrenceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_attestors_validate_attestation_occurrence_builder(
            &self.http_client,
            &args.attestor,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_attestors_validate_attestation_occurrence_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects platforms gke policies evaluate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EvaluateGkePolicyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn binaryauthorization_projects_platforms_gke_policies_evaluate(
        &self,
        args: &BinaryauthorizationProjectsPlatformsGkePoliciesEvaluateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EvaluateGkePolicyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_platforms_gke_policies_evaluate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_platforms_gke_policies_evaluate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects platforms policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlatformPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn binaryauthorization_projects_platforms_policies_create(
        &self,
        args: &BinaryauthorizationProjectsPlatformsPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlatformPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_platforms_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.policyId,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_platforms_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects platforms policies delete.
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
    pub fn binaryauthorization_projects_platforms_policies_delete(
        &self,
        args: &BinaryauthorizationProjectsPlatformsPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_platforms_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_platforms_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects platforms policies replace platform policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PlatformPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn binaryauthorization_projects_platforms_policies_replace_platform_policy(
        &self,
        args: &BinaryauthorizationProjectsPlatformsPoliciesReplacePlatformPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PlatformPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_platforms_policies_replace_platform_policy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_platforms_policies_replace_platform_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects policy set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IamPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn binaryauthorization_projects_policy_set_iam_policy(
        &self,
        args: &BinaryauthorizationProjectsPolicySetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IamPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_policy_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_policy_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Binaryauthorization projects policy test iam permissions.
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
    pub fn binaryauthorization_projects_policy_test_iam_permissions(
        &self,
        args: &BinaryauthorizationProjectsPolicyTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = binaryauthorization_projects_policy_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = binaryauthorization_projects_policy_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
