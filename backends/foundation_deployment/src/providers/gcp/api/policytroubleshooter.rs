//! PolicytroubleshooterProvider - State-aware policytroubleshooter API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       policytroubleshooter API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::policytroubleshooter::{
    policytroubleshooter_iam_troubleshoot_builder, policytroubleshooter_iam_troubleshoot_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::policytroubleshooter::GoogleCloudPolicytroubleshooterIamV3TroubleshootIamPolicyResponse;
use crate::providers::gcp::clients::policytroubleshooter::PolicytroubleshooterIamTroubleshootArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PolicytroubleshooterProvider with automatic state tracking.
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
/// let provider = PolicytroubleshooterProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PolicytroubleshooterProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PolicytroubleshooterProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PolicytroubleshooterProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Policytroubleshooter iam troubleshoot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudPolicytroubleshooterIamV3TroubleshootIamPolicyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn policytroubleshooter_iam_troubleshoot(
        &self,
        args: &PolicytroubleshooterIamTroubleshootArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudPolicytroubleshooterIamV3TroubleshootIamPolicyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = policytroubleshooter_iam_troubleshoot_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = policytroubleshooter_iam_troubleshoot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
