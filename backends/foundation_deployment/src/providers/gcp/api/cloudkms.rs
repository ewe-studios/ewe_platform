//! CloudKmsProvider - State-aware GCP Cloud KMS API client.
//!
//! WHY: Users need automatic state tracking for Cloud KMS resources
//!      without boilerplate in every method call.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       Cloud KMS API endpoints that auto-store results.
//!
//! HOW: Each method wraps the generated task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudkms::{
    cloudkms_folders_get_autokey_config_builder,
    cloudkms_folders_get_autokey_config_task,
    cloudkms_folders_get_kaj_policy_config_builder,
    cloudkms_folders_get_kaj_policy_config_task,
    AutokeyConfig,
    CloudkmsFoldersGetAutokeyConfigArgs,
    CloudkmsFoldersGetKajPolicyConfigArgs,
    KeyAccessJustificationsPolicyConfig,
};
use crate::providers::gcp::clients::types::ApiError;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GCP Cloud Kms API provider with automatic state tracking.
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
/// let cloud_kms = CloudKmsProvider::new(client, http_client);
///
/// let args = CloudkmsFoldersGetAutokeyConfigArgs { name: "folders/123".to_string() };
/// let result = cloud_kms.folders_get_autokey_config(&args)?;
/// // State automatically stored in state store
/// ```
#[derive(Clone)]
pub struct CloudKmsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudKmsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudKmsProvider with provider client and HTTP client.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Get AutokeyConfig for a folder or project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments containing the folder/project name
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutokeyConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails or state storage fails.
    pub fn folders_get_autokey_config(
        &self,
        args: &CloudkmsFoldersGetAutokeyConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutokeyConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_folders_get_autokey_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_folders_get_autokey_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Get KeyAccessJustificationsPolicyConfig for a folder or project.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments containing the folder/project name
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the KeyAccessJustificationsPolicyConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails or state storage fails.
    pub fn folders_get_kaj_policy_config(
        &self,
        args: &CloudkmsFoldersGetKajPolicyConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<KeyAccessJustificationsPolicyConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudkms_folders_get_kaj_policy_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudkms_folders_get_kaj_policy_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }
}
