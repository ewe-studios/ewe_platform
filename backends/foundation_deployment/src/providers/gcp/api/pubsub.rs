//! PubsubProvider - State-aware pubsub API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       pubsub API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::pubsub::{
    pubsub_projects_schemas_commit_builder, pubsub_projects_schemas_commit_task,
    pubsub_projects_schemas_create_builder, pubsub_projects_schemas_create_task,
    pubsub_projects_schemas_delete_builder, pubsub_projects_schemas_delete_task,
    pubsub_projects_schemas_delete_revision_builder, pubsub_projects_schemas_delete_revision_task,
    pubsub_projects_schemas_rollback_builder, pubsub_projects_schemas_rollback_task,
    pubsub_projects_schemas_set_iam_policy_builder, pubsub_projects_schemas_set_iam_policy_task,
    pubsub_projects_schemas_test_iam_permissions_builder, pubsub_projects_schemas_test_iam_permissions_task,
    pubsub_projects_schemas_validate_builder, pubsub_projects_schemas_validate_task,
    pubsub_projects_schemas_validate_message_builder, pubsub_projects_schemas_validate_message_task,
    pubsub_projects_snapshots_create_builder, pubsub_projects_snapshots_create_task,
    pubsub_projects_snapshots_delete_builder, pubsub_projects_snapshots_delete_task,
    pubsub_projects_snapshots_patch_builder, pubsub_projects_snapshots_patch_task,
    pubsub_projects_snapshots_set_iam_policy_builder, pubsub_projects_snapshots_set_iam_policy_task,
    pubsub_projects_snapshots_test_iam_permissions_builder, pubsub_projects_snapshots_test_iam_permissions_task,
    pubsub_projects_subscriptions_acknowledge_builder, pubsub_projects_subscriptions_acknowledge_task,
    pubsub_projects_subscriptions_create_builder, pubsub_projects_subscriptions_create_task,
    pubsub_projects_subscriptions_delete_builder, pubsub_projects_subscriptions_delete_task,
    pubsub_projects_subscriptions_detach_builder, pubsub_projects_subscriptions_detach_task,
    pubsub_projects_subscriptions_modify_ack_deadline_builder, pubsub_projects_subscriptions_modify_ack_deadline_task,
    pubsub_projects_subscriptions_modify_push_config_builder, pubsub_projects_subscriptions_modify_push_config_task,
    pubsub_projects_subscriptions_patch_builder, pubsub_projects_subscriptions_patch_task,
    pubsub_projects_subscriptions_pull_builder, pubsub_projects_subscriptions_pull_task,
    pubsub_projects_subscriptions_seek_builder, pubsub_projects_subscriptions_seek_task,
    pubsub_projects_subscriptions_set_iam_policy_builder, pubsub_projects_subscriptions_set_iam_policy_task,
    pubsub_projects_subscriptions_test_iam_permissions_builder, pubsub_projects_subscriptions_test_iam_permissions_task,
    pubsub_projects_topics_create_builder, pubsub_projects_topics_create_task,
    pubsub_projects_topics_delete_builder, pubsub_projects_topics_delete_task,
    pubsub_projects_topics_patch_builder, pubsub_projects_topics_patch_task,
    pubsub_projects_topics_publish_builder, pubsub_projects_topics_publish_task,
    pubsub_projects_topics_set_iam_policy_builder, pubsub_projects_topics_set_iam_policy_task,
    pubsub_projects_topics_test_iam_permissions_builder, pubsub_projects_topics_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::pubsub::DetachSubscriptionResponse;
use crate::providers::gcp::clients::pubsub::Empty;
use crate::providers::gcp::clients::pubsub::Policy;
use crate::providers::gcp::clients::pubsub::PublishResponse;
use crate::providers::gcp::clients::pubsub::PullResponse;
use crate::providers::gcp::clients::pubsub::Schema;
use crate::providers::gcp::clients::pubsub::SeekResponse;
use crate::providers::gcp::clients::pubsub::Snapshot;
use crate::providers::gcp::clients::pubsub::Subscription;
use crate::providers::gcp::clients::pubsub::TestIamPermissionsResponse;
use crate::providers::gcp::clients::pubsub::Topic;
use crate::providers::gcp::clients::pubsub::ValidateMessageResponse;
use crate::providers::gcp::clients::pubsub::ValidateSchemaResponse;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasCommitArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasCreateArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasDeleteArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasDeleteRevisionArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasRollbackArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasSetIamPolicyArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasTestIamPermissionsArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasValidateArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSchemasValidateMessageArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSnapshotsCreateArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSnapshotsDeleteArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSnapshotsPatchArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSnapshotsSetIamPolicyArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSnapshotsTestIamPermissionsArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsAcknowledgeArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsCreateArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsDeleteArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsDetachArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsModifyAckDeadlineArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsModifyPushConfigArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsPatchArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsPullArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsSeekArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsSetIamPolicyArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsSubscriptionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsTopicsCreateArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsTopicsDeleteArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsTopicsPatchArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsTopicsPublishArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsTopicsSetIamPolicyArgs;
use crate::providers::gcp::clients::pubsub::PubsubProjectsTopicsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PubsubProvider with automatic state tracking.
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
/// let provider = PubsubProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PubsubProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PubsubProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PubsubProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Pubsub projects schemas commit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Schema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_schemas_commit(
        &self,
        args: &PubsubProjectsSchemasCommitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Schema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_commit_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_commit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects schemas create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Schema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_schemas_create(
        &self,
        args: &PubsubProjectsSchemasCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Schema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_create_builder(
            &self.http_client,
            &args.parent,
            &args.schemaId,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects schemas delete.
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
    pub fn pubsub_projects_schemas_delete(
        &self,
        args: &PubsubProjectsSchemasDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects schemas delete revision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Schema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_schemas_delete_revision(
        &self,
        args: &PubsubProjectsSchemasDeleteRevisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Schema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_delete_revision_builder(
            &self.http_client,
            &args.name,
            &args.revisionId,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_delete_revision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects schemas rollback.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Schema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_schemas_rollback(
        &self,
        args: &PubsubProjectsSchemasRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Schema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects schemas set iam policy.
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
    pub fn pubsub_projects_schemas_set_iam_policy(
        &self,
        args: &PubsubProjectsSchemasSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects schemas test iam permissions.
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
    pub fn pubsub_projects_schemas_test_iam_permissions(
        &self,
        args: &PubsubProjectsSchemasTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects schemas validate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValidateSchemaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_schemas_validate(
        &self,
        args: &PubsubProjectsSchemasValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValidateSchemaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_validate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_validate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects schemas validate message.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValidateMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_schemas_validate_message(
        &self,
        args: &PubsubProjectsSchemasValidateMessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValidateMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_schemas_validate_message_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_schemas_validate_message_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects snapshots create.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_snapshots_create(
        &self,
        args: &PubsubProjectsSnapshotsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snapshot, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_snapshots_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_snapshots_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects snapshots delete.
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
    pub fn pubsub_projects_snapshots_delete(
        &self,
        args: &PubsubProjectsSnapshotsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_snapshots_delete_builder(
            &self.http_client,
            &args.snapshot,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_snapshots_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects snapshots patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_snapshots_patch(
        &self,
        args: &PubsubProjectsSnapshotsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snapshot, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_snapshots_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_snapshots_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects snapshots set iam policy.
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
    pub fn pubsub_projects_snapshots_set_iam_policy(
        &self,
        args: &PubsubProjectsSnapshotsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_snapshots_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_snapshots_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects snapshots test iam permissions.
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
    pub fn pubsub_projects_snapshots_test_iam_permissions(
        &self,
        args: &PubsubProjectsSnapshotsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_snapshots_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_snapshots_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions acknowledge.
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
    pub fn pubsub_projects_subscriptions_acknowledge(
        &self,
        args: &PubsubProjectsSubscriptionsAcknowledgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_acknowledge_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_acknowledge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_subscriptions_create(
        &self,
        args: &PubsubProjectsSubscriptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions delete.
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
    pub fn pubsub_projects_subscriptions_delete(
        &self,
        args: &PubsubProjectsSubscriptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_delete_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions detach.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DetachSubscriptionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_subscriptions_detach(
        &self,
        args: &PubsubProjectsSubscriptionsDetachArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DetachSubscriptionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_detach_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_detach_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions modify ack deadline.
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
    pub fn pubsub_projects_subscriptions_modify_ack_deadline(
        &self,
        args: &PubsubProjectsSubscriptionsModifyAckDeadlineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_modify_ack_deadline_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_modify_ack_deadline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions modify push config.
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
    pub fn pubsub_projects_subscriptions_modify_push_config(
        &self,
        args: &PubsubProjectsSubscriptionsModifyPushConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_modify_push_config_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_modify_push_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_subscriptions_patch(
        &self,
        args: &PubsubProjectsSubscriptionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions pull.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PullResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_subscriptions_pull(
        &self,
        args: &PubsubProjectsSubscriptionsPullArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PullResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_pull_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_pull_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions seek.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SeekResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_subscriptions_seek(
        &self,
        args: &PubsubProjectsSubscriptionsSeekArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SeekResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_seek_builder(
            &self.http_client,
            &args.subscription,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_seek_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions set iam policy.
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
    pub fn pubsub_projects_subscriptions_set_iam_policy(
        &self,
        args: &PubsubProjectsSubscriptionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects subscriptions test iam permissions.
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
    pub fn pubsub_projects_subscriptions_test_iam_permissions(
        &self,
        args: &PubsubProjectsSubscriptionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_subscriptions_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_subscriptions_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects topics create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_topics_create(
        &self,
        args: &PubsubProjectsTopicsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_topics_create_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_topics_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects topics delete.
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
    pub fn pubsub_projects_topics_delete(
        &self,
        args: &PubsubProjectsTopicsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_topics_delete_builder(
            &self.http_client,
            &args.topic,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_topics_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects topics patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Topic result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_topics_patch(
        &self,
        args: &PubsubProjectsTopicsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Topic, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_topics_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_topics_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects topics publish.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PublishResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn pubsub_projects_topics_publish(
        &self,
        args: &PubsubProjectsTopicsPublishArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PublishResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_topics_publish_builder(
            &self.http_client,
            &args.topic,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_topics_publish_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects topics set iam policy.
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
    pub fn pubsub_projects_topics_set_iam_policy(
        &self,
        args: &PubsubProjectsTopicsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_topics_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_topics_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pubsub projects topics test iam permissions.
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
    pub fn pubsub_projects_topics_test_iam_permissions(
        &self,
        args: &PubsubProjectsTopicsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pubsub_projects_topics_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = pubsub_projects_topics_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
