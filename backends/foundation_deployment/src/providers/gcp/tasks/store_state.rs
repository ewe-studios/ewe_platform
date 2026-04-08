//! Task wrapper that stores state changes on completion.
//!
//! WHY: API operations need to automatically persist their results
//!      to the state store without boilerplate in every method.
//!
//! WHAT: Wraps any TaskIterator, intercepts completion, and stores
//!       the resource state if successful.
//!
//! HOW: Uses valtron combinators to transform Ready values - on first
//!      Ready value, store state then pass through the value unchanged.

use foundation_core::valtron::{TaskIterator, TaskIteratorExt, TaskStatus};
use foundation_db::state::traits::StateStore;
use foundation_db::state::{config_hash, ResourceState, StateStatus};
use serde::Serialize;
use std::sync::Arc;

use crate::providers::gcp::clients::types::ApiError;
use foundation_db::errors::StorageError;

/// Provider-specific error wrapper.
#[derive(Debug)]
pub enum ProviderError<S>
where
    S: std::error::Error + Send + Sync + 'static,
{
    /// State store operation failed
    StateStoreError(S),
    /// API request failed
    ApiFailed(String),
    /// Serialization failed
    SerializeFailed(String),
    /// Hash computation failed
    HashFailed(String),
    /// Execution failed
    ExecuteFailed(String),
}

impl<S> std::fmt::Display for ProviderError<S>
where
    S: std::error::Error + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StateStoreError(e) => write!(f, "state store error: {}", e),
            Self::ApiFailed(e) => write!(f, "API failed: {}", e),
            Self::SerializeFailed(e) => write!(f, "serialization failed: {}", e),
            Self::HashFailed(e) => write!(f, "hash failed: {}", e),
            Self::ExecuteFailed(e) => write!(f, "execution failed: {}", e),
        }
    }
}

impl<S> std::error::Error for ProviderError<S> where
    S: std::error::Error + Send + Sync + 'static
{
}

/// Task wrapper that stores state changes on completion.
#[derive(Debug)]
pub struct StoreStateTask<Inner, St, I, F>
where
    Inner: TaskIterator,
    St: StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    F: Fn(&Inner::Ready) -> String + Send + 'static,
{
    /// Inner task (e.g., SendRequestTask)
    inner: Inner,
    /// State store for persistence
    state_store: Arc<St>,
    /// Resource ID for state tracking
    resource_id: String,
    /// Resource kind (e.g., "gcp::cloudkms::KeyRing")
    resource_kind: String,
    /// Provider name (e.g., "gcp")
    provider: String,
    /// Input configuration (stored as snapshot)
    input: I,
    /// Environment (optional)
    environment: Option<String>,
    /// Function to extract output from Ready value for storage
    extract_output: F,
}

impl<Inner, St, I, F, D, P> StoreStateTask<Inner, St, I, F>
where
    Inner: TaskIterator<Ready = D, Pending = P>,
    St: StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    F: Fn(&D) -> String + Send + 'static,
    D: Serialize + Send + 'static,
    P: Clone + Send + 'static,
{
    /// Create new store state task.
    ///
    /// # Arguments
    ///
    /// * `inner` - The inner task to wrap (e.g., from `_task()` method)
    /// * `state_store` - State store for persistence
    /// * `resource_id` - Unique identifier for this resource
    /// * `resource_kind` - Type of resource (e.g., "gcp::cloudkms::AutoKeyConfig")
    /// * `provider` - Provider name (e.g., "gcp")
    /// * `input` - Input configuration to store as snapshot
    /// * `environment` - Optional environment name
    /// * `extract_output` - Function to extract output value from Ready for storage
    pub fn new(
        inner: Inner,
        state_store: Arc<St>,
        resource_id: String,
        resource_kind: String,
        provider: String,
        input: I,
        environment: Option<String>,
        extract_output: F,
    ) -> Self {
        Self {
            inner,
            state_store,
            resource_id,
            resource_kind,
            provider,
            input,
            environment,
            extract_output,
        }
    }
}

impl<Inner, St, I, F, D, P> TaskIterator for StoreStateTask<Inner, St, I, F>
where
    Inner: TaskIterator<Ready = Result<D, ApiError>, Pending = P>,
    St: StateStore<Error = foundation_db::errors::StorageError> + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    F: Fn(&Result<D, ApiError>) -> String + Send + 'static,
    D: Serialize + Send + 'static,
    P: Clone + Send + 'static,
{
    type Ready = Result<D, ProviderError<StorageError>>;
    type Pending = P;
    type Spawner = Inner::Spawner;

    fn next_status(
        &mut self,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.inner.next_status()? {
            TaskStatus::Ready(Ok(output)) => {
                // Task completed successfully - store state
                let config_snapshot = match serde_json::to_value(&self.input) {
                    Ok(v) => v,
                    Err(e) => {
                        return Some(TaskStatus::Ready(Err(ProviderError::SerializeFailed(
                            e.to_string(),
                        ))))
                    }
                };

                let output_str = (self.extract_output)(&Ok(output));
                let output_value: serde_json::Value =
                    match serde_json::from_str(&output_str) {
                        Ok(v) => v,
                        Err(e) => {
                            return Some(TaskStatus::Ready(Err(ProviderError::SerializeFailed(
                                e.to_string(),
                            ))))
                        }
                    };

                let config_hash_value = match config_hash(&config_snapshot) {
                    Ok(h) => h,
                    Err(e) => {
                        return Some(TaskStatus::Ready(Err(ProviderError::HashFailed(
                            e.to_string(),
                        ))))
                    }
                };

                let now = chrono::Utc::now();

                let state = ResourceState {
                    id: self.resource_id.clone(),
                    kind: self.resource_kind.clone(),
                    provider: self.provider.clone(),
                    status: StateStatus::Created,
                    environment: self.environment.clone(),
                    config_hash: config_hash_value,
                    output: output_value,
                    config_snapshot,
                    created_at: now,
                    updated_at: now,
                };

                // Store state - FileStateStore returns a stream, we need to consume it
                match self.state_store.set(&self.resource_id, &state) {
                    Ok(mut stream) => {
                        // Consume the stream (for FileStateStore it's a single value)
                        for item in stream {
                            if let foundation_core::valtron::ThreadedValue::Value(Err(e)) = item {
                                return Some(TaskStatus::Ready(Err(ProviderError::StateStoreError(
                                    e,
                                ))));
                            }
                        }
                    }
                    Err(e) => {
                        return Some(TaskStatus::Ready(Err(ProviderError::StateStoreError(e))))
                    }
                }

                Some(TaskStatus::Ready(Ok(output)))
            }
            TaskStatus::Ready(Err(e)) => {
                // Task failed - propagate error
                Some(TaskStatus::Ready(Err(ProviderError::ApiFailed(e.to_string()))))
            }
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Spawn(action) => Some(TaskStatus::Spawn(action)),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
        }
    }
}
