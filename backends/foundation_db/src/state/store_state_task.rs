//! Task wrapper that stores state changes on completion.
//!
//! WHY: API operations need to automatically persist their results
//!      to the state store without boilerplate in every method.
//!
//! WHAT: Wraps any `TaskIterator` with a state machine that:
//!   1. Spawns the inner task
//!   2. On success, creates state and calls `state_store.set()`
//!   3. Polls the returned stream until exhausted
//!   4. Yields the original output
//!
//! HOW: Uses explicit `state` enum to track progress through inner task
//!      reception, state creation, and state store stream polling.

use crate::errors::StorageError;
use crate::state::resource_identifier::ResourceIdentifier;
use crate::state::traits::{StateStore, StateStoreStream};
use crate::state::{config_hash, ResourceState, StateStatus};
use foundation_core::valtron::{TaskIterator, TaskStatus, ThreadedValue};
use serde::Serialize;
use std::sync::Arc;

/// Provider-specific error wrapper.
///
/// WHY: Provider methods need to report errors from both:
///   1. API client operations (generic error type)
///   2. State store operations (`StorageError`)
///
/// WHAT: Single enum that can hold either error type, with Display + Error impls.
///
/// HOW: Generic over the inner error type E, with variants for state store
///      errors, serialization failures, and execution failures.
#[derive(Debug)]
pub enum ProviderError<E> {
    /// API request/operation failed
    Api(E),

    /// State store operation failed
    State(StorageError),

    /// Serialization failed (input/output to JSON)
    SerializeFailed(String),

    /// Hash computation failed
    HashFailed(String),

    /// Valtron execution failed (scheduling error)
    ExecuteFailed(String),
}

impl<E> ProviderError<E> {
    /// Wrap an API error into `ProviderError`.
    pub fn api(err: E) -> Self {
        Self::Api(err)
    }

    /// Wrap a [`StorageError`] into `ProviderError`.
    #[must_use]
    pub fn state(err: StorageError) -> Self {
        Self::State(err)
    }

    /// Map the error type.
    pub fn map<F>(self, f: impl FnOnce(E) -> F) -> ProviderError<F> {
        match self {
            ProviderError::Api(e) => ProviderError::Api(f(e)),
            ProviderError::State(e) => ProviderError::State(e),
            ProviderError::SerializeFailed(s) => ProviderError::SerializeFailed(s),
            ProviderError::HashFailed(s) => ProviderError::HashFailed(s),
            ProviderError::ExecuteFailed(s) => ProviderError::ExecuteFailed(s),
        }
    }
}

impl<E> From<StorageError> for ProviderError<E> {
    fn from(err: StorageError) -> Self {
        Self::State(err)
    }
}

impl<E> std::fmt::Display for ProviderError<E>
where
    E: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Api(e) => write!(f, "API error: {e}"),
            Self::State(e) => write!(f, "state store error: {e}"),
            Self::SerializeFailed(e) => write!(f, "serialization failed: {e}"),
            Self::HashFailed(e) => write!(f, "hash failed: {e}"),
            Self::ExecuteFailed(e) => write!(f, "execution failed: {e}"),
        }
    }
}

impl<E> std::error::Error for ProviderError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Api(e) => Some(e),
            Self::State(e) => Some(e),
            _ => None,
        }
    }
}

/// Pending states for `StoreStateTask`.
#[derive(Debug, Clone)]
pub enum StoreStatePending<P> {
    /// Waiting for inner task to produce a result
    WaitingInner(P),
    /// Waiting for state store stream to finish storing
    WaitingStore,
}

/// State machine for `StoreStateTask` with pre-computed resource info.
enum StoreStatePrecomputed<Inner, St, I, D>
where
    Inner: TaskIterator,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    D: Serialize + Send + 'static,
{
    /// Initial state - delegate to inner task
    Inner {
        inner: Inner,
        state_store: Arc<St>,
        resource_id: String,
        resource_kind: String,
        provider: String,
        input: I,
        environment: Option<String>,
    },
    /// Inner task succeeded - storing result
    Storing {
        output: D,
        stream: StateStoreStream<()>,
    },
    /// Done - no more states
    Done,
}

impl<Inner, St, I, D> std::fmt::Debug for StoreStatePrecomputed<Inner, St, I, D>
where
    Inner: TaskIterator + std::fmt::Debug,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    D: Serialize + Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inner { resource_id, .. } => f
                .debug_struct("StoreStatePrecomputed::Inner")
                .field("resource_id", resource_id)
                .finish(),
            Self::Storing { .. } => f.debug_struct("StoreStatePrecomputed::Storing").finish(),
            Self::Done => write!(f, "StoreStatePrecomputed::Done"),
        }
    }
}

/// Task wrapper that stores state changes on completion (pre-computed resource info).
///
/// WHY: Operations need to automatically persist their results
///      to the state store without boilerplate in every method.
///
/// WHAT: Wraps any `TaskIterator` with pre-computed resource info,
///       spawns it, monitors for successful completion,
///       calls `state_store.set()` and processes the returned stream, then yields
///       the original output.
///
/// HOW: State machine with three states:
///   1. `Inner` - delegate to inner task
///   2. `Storing` - poll the state store stream
///   3. `Done` - complete
#[derive(Debug)]
pub struct StoreStateTask<Inner, St, I, D>
where
    Inner: TaskIterator,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    D: Serialize + Send + 'static,
{
    state: Option<StoreStatePrecomputed<Inner, St, I, D>>,
}

impl<Inner, St, I, D> StoreStateTask<Inner, St, I, D>
where
    Inner: TaskIterator,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    D: Serialize + Send + 'static,
{
    /// Create new store state task with pre-computed resource info.
    ///
    /// # Arguments
    ///
    /// * `inner` - The inner task to wrap (e.g., from `_task()` method)
    /// * `state_store` - State store for persistence
    /// * `resource_id` - Unique identifier for this resource
    /// * `resource_kind` - Type of resource (e.g., `gcp::cloudkms::AutoKeyConfig`)
    /// * `provider` - Provider name (e.g., `gcp`)
    /// * `input` - Input configuration to store as snapshot
    /// * `environment` - Optional environment name
    pub fn new(
        inner: Inner,
        state_store: Arc<St>,
        resource_id: String,
        resource_kind: String,
        provider: String,
        input: I,
        environment: Option<String>,
    ) -> Self {
        Self {
            state: Some(StoreStatePrecomputed::Inner {
                inner,
                state_store,
                resource_id,
                resource_kind,
                provider,
                input,
                environment,
            }),
        }
    }
}

impl<Inner, St, I, D, E, P> TaskIterator for StoreStateTask<Inner, St, I, D>
where
    Inner: TaskIterator<Ready = Result<D, E>, Pending = P>,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    D: Serialize + Send + 'static,
    P: Clone + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    type Ready = Result<D, ProviderError<E>>;
    type Pending = StoreStatePending<P>;
    type Spawner = Inner::Spawner;

    #[allow(clippy::missing_panics_doc, clippy::too_many_lines)]
    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state.take()? {
            StoreStatePrecomputed::Inner {
                mut inner,
                state_store,
                resource_id,
                resource_kind,
                provider,
                input,
                environment,
            } => {
                match inner.next_status()? {
                    TaskStatus::Ready(Ok(output)) => {
                        // Inner task succeeded - build state and call state_store.set()
                        let config_snapshot = match serde_json::to_value(&input) {
                            Ok(v) => v,
                            Err(e) => {
                                return Some(TaskStatus::Ready(Err(
                                    ProviderError::SerializeFailed(e.to_string()),
                                )))
                            }
                        };

                        let output_value: serde_json::Value = match serde_json::to_value(&output) {
                            Ok(v) => v,
                            Err(e) => {
                                return Some(TaskStatus::Ready(Err(
                                    ProviderError::SerializeFailed(e.to_string()),
                                )))
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
                            id: resource_id.clone(),
                            kind: resource_kind.clone(),
                            provider: provider.clone(),
                            status: StateStatus::Created,
                            environment: environment.clone(),
                            config_hash: config_hash_value,
                            output: output_value,
                            config_snapshot,
                            created_at: now,
                            updated_at: now,
                        };

                        match state_store.set(&resource_id, &state) {
                            Ok(stream) => {
                                self.state =
                                    Some(StoreStatePrecomputed::Storing { output, stream });
                                Some(TaskStatus::Pending(StoreStatePending::WaitingStore))
                            }
                            Err(e) => {
                                self.state = Some(StoreStatePrecomputed::Done);
                                Some(TaskStatus::Ready(Err(ProviderError::State(e))))
                            }
                        }
                    }
                    TaskStatus::Ready(Err(e)) => {
                        self.state = Some(StoreStatePrecomputed::Done);
                        Some(TaskStatus::Ready(Err(ProviderError::ExecuteFailed(
                            e.to_string(),
                        ))))
                    }
                    TaskStatus::Pending(p) => {
                        self.state = Some(StoreStatePrecomputed::Inner {
                            inner,
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Pending(StoreStatePending::WaitingInner(p)))
                    }
                    TaskStatus::Init => {
                        self.state = Some(StoreStatePrecomputed::Inner {
                            inner,
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Init)
                    }
                    TaskStatus::Delayed(d) => {
                        self.state = Some(StoreStatePrecomputed::Inner {
                            inner,
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Delayed(d))
                    }
                    TaskStatus::Spawn(action) => {
                        self.state = Some(StoreStatePrecomputed::Inner {
                            inner,
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Spawn(action))
                    }
                    TaskStatus::Ignore => {
                        self.state = Some(StoreStatePrecomputed::Inner {
                            inner,
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Ignore)
                    }
                }
            }
            StoreStatePrecomputed::Storing { output, mut stream } => {
                // Poll the state store stream
                match stream.next() {
                    Some(ThreadedValue::Value(Ok(()))) => {
                        // Stream yielded success - continue polling
                        self.state = Some(StoreStatePrecomputed::Storing { output, stream });
                        Some(TaskStatus::Pending(StoreStatePending::WaitingStore))
                    }
                    Some(ThreadedValue::Value(Err(e))) => {
                        // Stream yielded error
                        self.state = Some(StoreStatePrecomputed::Done);
                        Some(TaskStatus::Ready(Err(ProviderError::State(e))))
                    }
                    None => {
                        // Stream exhausted - storage complete
                        self.state = Some(StoreStatePrecomputed::Done);
                        Some(TaskStatus::Ready(Ok(output)))
                    }
                }
            }
            StoreStatePrecomputed::Done => None,
        }
    }
}

/// State machine for `StoreStateIdentifierTask` (uses `ResourceIdentifier` trait).
enum StoreStateIdentifierInner<Inner, St, I, O>
where
    Inner: TaskIterator,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static,
{
    /// Initial state - delegate to inner task
    Inner {
        inner: Inner,
        state_store: Arc<St>,
        input: I,
        environment: Option<String>,
    },
    /// Inner task succeeded - storing result
    Storing {
        output: O,
        stream: StateStoreStream<()>,
    },
    /// Done - no more states
    Done,
}

impl<Inner, St, I, O> std::fmt::Debug for StoreStateIdentifierInner<Inner, St, I, O>
where
    Inner: TaskIterator + std::fmt::Debug,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inner { .. } => f.debug_struct("StoreStateIdentifierInner::Inner").finish(),
            Self::Storing { .. } => f
                .debug_struct("StoreStateIdentifierInner::Storing")
                .finish(),
            Self::Done => write!(f, "StoreStateIdentifierInner::Done"),
        }
    }
}

/// Task wrapper that stores state changes on completion using `ResourceIdentifier` trait.
///
/// WHY: For output types that implement `ResourceIdentifier<Input>`, we can defer resource ID
///      computation until the output is available, avoiding the need to pre-compute it.
///
/// WHAT: Wraps any `TaskIterator` where the output implements `ResourceIdentifier<Input>`.
///
/// HOW: On successful completion, calls `generate_resource_id` on the output to get
///      the resource ID, then stores state via a polled stream.
#[derive(Debug)]
pub struct StoreStateIdentifierTask<Inner, St, I, O>
where
    Inner: TaskIterator,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static,
{
    state: Option<StoreStateIdentifierInner<Inner, St, I, O>>,
}

impl<Inner, St, I, O> StoreStateIdentifierTask<Inner, St, I, O>
where
    Inner: TaskIterator,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static,
{
    /// Create new store state task using `ResourceIdentifier` trait.
    ///
    /// This constructor defers resource ID computation until the output is available.
    /// The output type must implement `ResourceIdentifier<Input>`.
    ///
    /// # Arguments
    ///
    /// * `inner` - The inner task to wrap (e.g., from `_task()` method)
    /// * `state_store` - State store for persistence
    /// * `input` - Input configuration to store as snapshot
    /// * `environment` - Optional environment name
    pub fn new(inner: Inner, state_store: Arc<St>, input: I, environment: Option<String>) -> Self {
        Self {
            state: Some(StoreStateIdentifierInner::Inner {
                inner,
                state_store,
                input,
                environment,
            }),
        }
    }
}

impl<Inner, St, I, O, E, P> TaskIterator for StoreStateIdentifierTask<Inner, St, I, O>
where
    Inner: TaskIterator<Ready = Result<O, E>, Pending = P>,
    St: StateStore,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static + ResourceIdentifier<I>,
    P: Clone + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    type Ready = Result<O, ProviderError<E>>;
    type Pending = StoreStatePending<P>;
    type Spawner = Inner::Spawner;

    #[allow(clippy::missing_panics_doc, clippy::too_many_lines)]
    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state.take()? {
            StoreStateIdentifierInner::Inner {
                mut inner,
                state_store,
                input,
                environment,
            } => {
                match inner.next_status()? {
                    TaskStatus::Ready(Ok(output)) => {
                        // Inner task succeeded - compute resource info from output
                        let resource_id = output.generate_resource_id(&input);
                        let resource_kind = output.resource_kind();
                        let provider = output.provider();

                        let config_snapshot = match serde_json::to_value(&input) {
                            Ok(v) => v,
                            Err(e) => {
                                return Some(TaskStatus::Ready(Err(
                                    ProviderError::SerializeFailed(e.to_string()),
                                )))
                            }
                        };

                        let output_str = match serde_json::to_string(&output) {
                            Ok(s) => s,
                            Err(e) => {
                                return Some(TaskStatus::Ready(Err(
                                    ProviderError::SerializeFailed(e.to_string()),
                                )))
                            }
                        };

                        let output_value: serde_json::Value =
                            match serde_json::from_str(&output_str) {
                                Ok(v) => v,
                                Err(e) => {
                                    return Some(TaskStatus::Ready(Err(
                                        ProviderError::SerializeFailed(e.to_string()),
                                    )))
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
                            id: resource_id,
                            kind: resource_kind.to_string(),
                            provider: provider.to_string(),
                            status: StateStatus::Created,
                            environment: environment.clone(),
                            config_hash: config_hash_value,
                            output: output_value,
                            config_snapshot,
                            created_at: now,
                            updated_at: now,
                        };

                        match state_store.set(&state.id, &state) {
                            Ok(stream) => {
                                self.state =
                                    Some(StoreStateIdentifierInner::Storing { output, stream });
                                Some(TaskStatus::Pending(StoreStatePending::WaitingStore))
                            }
                            Err(e) => {
                                self.state = Some(StoreStateIdentifierInner::Done);
                                Some(TaskStatus::Ready(Err(ProviderError::State(e))))
                            }
                        }
                    }
                    TaskStatus::Ready(Err(e)) => {
                        self.state = Some(StoreStateIdentifierInner::Done);
                        Some(TaskStatus::Ready(Err(ProviderError::ExecuteFailed(
                            e.to_string(),
                        ))))
                    }
                    TaskStatus::Pending(p) => {
                        self.state = Some(StoreStateIdentifierInner::Inner {
                            inner,
                            state_store,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Pending(StoreStatePending::WaitingInner(p)))
                    }
                    TaskStatus::Init => {
                        self.state = Some(StoreStateIdentifierInner::Inner {
                            inner,
                            state_store,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Init)
                    }
                    TaskStatus::Delayed(d) => {
                        self.state = Some(StoreStateIdentifierInner::Inner {
                            inner,
                            state_store,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Delayed(d))
                    }
                    TaskStatus::Spawn(action) => {
                        self.state = Some(StoreStateIdentifierInner::Inner {
                            inner,
                            state_store,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Spawn(action))
                    }
                    TaskStatus::Ignore => {
                        self.state = Some(StoreStateIdentifierInner::Inner {
                            inner,
                            state_store,
                            input,
                            environment,
                        });
                        Some(TaskStatus::Ignore)
                    }
                }
            }
            StoreStateIdentifierInner::Storing { output, mut stream } => {
                // Poll the state store stream
                match stream.next() {
                    Some(ThreadedValue::Value(Ok(()))) => {
                        // Stream yielded success - continue polling
                        self.state = Some(StoreStateIdentifierInner::Storing { output, stream });
                        Some(TaskStatus::Pending(StoreStatePending::WaitingStore))
                    }
                    Some(ThreadedValue::Value(Err(e))) => {
                        // Stream yielded error
                        self.state = Some(StoreStateIdentifierInner::Done);
                        Some(TaskStatus::Ready(Err(ProviderError::State(e))))
                    }
                    None => {
                        // Stream exhausted - storage complete
                        self.state = Some(StoreStateIdentifierInner::Done);
                        Some(TaskStatus::Ready(Ok(output)))
                    }
                }
            }
            StoreStateIdentifierInner::Done => None,
        }
    }
}
