---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/28-provider-wrapper"
this_file: "specifications/11-foundation-deployment/features/28-provider-wrapper/feature.md"

status: internal
priority: medium
created: 2026-04-08
updated: 2026-04-11

depends_on: ["02-state-stores"]

tasks:
  completed: 3
  uncompleted: 5
  total: 8
  completion_percentage: 37%
---


# Provider Wrapper Pattern - State-Aware API Clients

## Status: INTERNAL IMPLEMENTATION

**This feature is an internal implementation detail, not the primary user-facing API.**

**Reason:** With Feature 35 (Trait-Based Deployments), users implement `Deployable` on their structs and call provider clients directly. Provider wrappers are **optional utilities** that providers may use internally for state tracking.

### User-Facing API (Feature 35)

Users implement `Deployable` directly:

```rust
impl Deployable for MyWorker {
    type Output = WorkerDeployment;
    type Error = DeploymentError;
    
    async fn deploy(&self) -> Result<Self::Output, Self::Error> {
        let client = CloudflareClient::from_env()?;
        client.put_worker_script(&self.name, &self.script).await
    }
}
```

### Internal Implementation (This Feature)

Provider wrappers with automatic state tracking can be used **internally** by provider implementations:

```rust
// Optional: Provider wrapper with automatic state tracking
let client = ProviderClient::new("my-project", "dev", state_store);
let cloud_kms = CloudKmsProvider::new(client);
let result = cloud_kms.folders_update_autokey_config(args)?;
// State automatically stored
```

**Benefits:**
- Automatic state tracking for resource management
- Useful for complex infrastructure with many resources
- Optional - users can call raw clients directly if preferred

## Architecture
- Optional - users can call raw clients directly if preferred

## Architecture

### Three-Layer Design

```
┌─────────────────────────────────────────────────────────────────┐
│                    User Code (Application)                       │
│  let result = cloud_kms.folders_update_autokey_config(args)?;   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Per-API Provider (CloudKmsProvider)                 │
│  - Takes ProviderClient<S>                                       │
│  - Provides methods per API endpoint                             │
│  - Wraps tasks with StoreStateTask                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   ProviderClient<S>                              │
│  - Wraps StateStore<S>                                           │
│  - Stores project + stage metadata                               │
│  - All methods public for direct access                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      StateStore<S>                               │
│  - FileStateStore, SqliteStateStore, TursoStateStore, etc.      │
│  - Namespaced by project + stage                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Task Flow

```
User calls cloud_kms.folders_update_autokey_config(args)
                    │
                    ▼
┌───────────────────────────────────────┐
│ 1. Build request via _builder()       │
│    - Creates ClientRequestBuilder     │
└───────────────┬───────────────────────┘
                │
                ▼
┌───────────────────────────────────────┐
│ 2. Create task via _task()            │
│    - Returns TaskIterator             │
│    - Contains SendRequestTask         │
└───────────────┬───────────────────────┘
                │
                ▼
┌───────────────────────────────────────┐
│ 3. Wrap with StoreStateTask           │
│    - Stores input config              │
│    - Will store output on success     │
│    - Propagates errors on failure     │
└───────────────┬───────────────────────┘
                │
                ▼
┌───────────────────────────────────────┐
│ 4. execute() returns StreamIterator   │
│    - Yields ApiResult<T, ProviderError>│
│    - State automatically updated      │
└───────────────────────────────────────┘
```

## Requirements

### ResourceIdentifier Trait

A trait for generating unique resource IDs from input/output pairs.

```rust
// foundation_db/src/state/resource_identifier.rs

use std::fmt::Debug;

/// Trait for generating unique resource identifiers.
///
/// WHY: Resources need unique IDs for state tracking, but the ID format
///      varies by API and operation. Some use output fields, some use
///      input fields, some combine both.
///
/// WHAT: Trait implemented by output types, with associated input type,
///       providing a method to generate the resource ID from both.
///
/// HOW: Generator analyzes OpenAPI spec to determine ID pattern for each
///      endpoint and generates appropriate trait implementations.
pub trait ResourceIdentifier<Input>: Debug + Send + Sync {
    /// Generate resource ID from input and output.
    ///
    /// # Arguments
    ///
    /// * `input` - The request input that produced this output
    ///
    /// # Returns
    ///
    /// Unique resource identifier (e.g., "gcp::cloudkms::AutoKeyConfig/folders/123")
    fn generate_resource_id(&self, input: &Input) -> String;
    
    /// Get the resource kind (e.g., "gcp::cloudkms::AutoKeyConfig").
    fn resource_kind(&self) -> &'static str;
    
    /// Get the provider name (e.g., "gcp").
    fn provider(&self) -> &'static str;
}

/// Helper function to compute resource info using ResourceIdentifier trait.
///
/// Used by StoreStateTaskWithResourceIdentifier to extract all resource info
/// from the output and input in one call.
pub fn compute_resource_info<Input, Output>(
    output: &Output,
    input: &Input,
) -> (String, &'static str, &'static str)
where
    Output: ResourceIdentifier<Input>,
{
    (
        output.generate_resource_id(input),
        output.resource_kind(),
        output.provider(),
    )
}
```

**Generator Responsibility:**

The code generator analyzes the OpenAPI spec to determine:
1. **Resource naming patterns** - e.g., `/v1/folders/{folderName}/autokeyConfig` → ID pattern: `folders/{folderName}/autokeyConfig`
2. **Which field(s) to use** - From input (e.g., `args.name`), output (e.g., `result.id`), or both
3. **Resource kind** - Based on the API and resource type

**Generated Implementation Example:**

```rust
// Generated for CloudKms.Folders.UpdateAutokeyConfig
// OpenAPI path: /v1/folders/{folderName}/autokeyConfig
// Generator determines: resource_id = format!("folders/{}", input.name)

impl ResourceIdentifier<CloudkmsFoldersUpdateAutokeyConfigArgs> 
    for CloudkmsAutoKeyConfig 
{
    fn generate_resource_id(&self, input: &CloudkmsFoldersUpdateAutokeyConfigArgs) -> String {
        format!("gcp::cloudkms::AutoKeyConfig/folders/{}", input.name)
    }
    
    fn resource_kind(&self) -> &'static str {
        "gcp::cloudkms::AutoKeyConfig"
    }
    
    fn provider(&self) -> &'static str {
        "gcp"
    }
}

// Generated for Compute.Zones.Insert where output has selfLink

impl ResourceIdentifier<ComputeZonesInsertArgs> for ComputeZone {
    fn generate_resource_id(&self, _input: &ComputeZonesInsertArgs) -> String {
        // Generator determined that output.name contains the resource identifier
        format!("gcp::compute::Zone/{}", self.name)
    }
    
    fn resource_kind(&self) -> &'static str {
        "gcp::compute::Zone"
    }
    
    fn provider(&self) -> &'static str {
        "gcp"
    }
}
```

### ProviderError Type

```rust
// providers/gcp/api/error.rs

use crate::providers::gcp::clients::types::ApiError;
use foundation_db::errors::StorageError;

/// Unified error type for provider operations.
///
/// WHY: Provider methods need to report errors from both:
///   1. API client operations (ApiError)
///   2. State store operations (StorageError)
///
/// WHAT: Single enum that can hold either error type, with Display + Error impls.
///
/// HOW: Two main variants wrapping the underlying errors, plus helper variants
///      for serialization/exec errors that don't fit either category.
#[derive(Debug)]
pub enum ProviderError {
    /// API request/operation failed
    Api(ApiError),
    
    /// State store operation failed
    State(StorageError),
    
    /// Serialization failed (input/output to JSON)
    SerializeFailed(String),
    
    /// Hash computation failed
    HashFailed(String),
    
    /// Valtron execution failed (scheduling error)
    ExecuteFailed(String),
}

impl ProviderError {
    /// Wrap an ApiError into ProviderError.
    pub fn api(err: ApiError) -> Self {
        Self::Api(err)
    }
    
    /// Wrap a StorageError into ProviderError.
    pub fn state(err: StorageError) -> Self {
        Self::State(err)
    }
}

impl From<ApiError> for ProviderError {
    fn from(err: ApiError) -> Self {
        Self::Api(err)
    }
}

impl From<StorageError> for ProviderError {
    fn from(err: StorageError) -> Self {
        Self::State(err)
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Api(e) => write!(f, "API error: {}", e),
            Self::State(e) => write!(f, "state store error: {}", e),
            Self::SerializeFailed(e) => write!(f, "serialization failed: {}", e),
            Self::HashFailed(e) => write!(f, "hash failed: {}", e),
            Self::ExecuteFailed(e) => write!(f, "execution failed: {}", e),
        }
    }
}

impl std::error::Error for ProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Api(e) => Some(e),
            Self::State(e) => Some(e),
            _ => None,
        }
    }
}
```

### ProviderClient Structure

```rust
// providers/gcp/api/mod.rs

use foundation_db::state::traits::StateStore;
use std::sync::Arc;

/// Central provider client wrapping a StateStore.
///
/// WHY: Users need a single entry point that manages state tracking
///      for all API operations in a project/stage context.
///
/// WHAT: Generic wrapper around any StateStore implementation,
///       storing project and stage metadata.
///
/// HOW: Holds Arc<StateStore> for thread-safe sharing across
///       per-API provider instances.
#[derive(Debug, Clone)]
pub struct ProviderClient<S>
where
    S: StateStore + Send + Sync + 'static,
{
    /// State store for persistence
    pub state_store: Arc<S>,
    /// Project name for namespacing
    pub project: String,
    /// Stage (dev/staging/prod)
    pub stage: String,
}

impl<S> ProviderClient<S>
where
    S: StateStore + Send + Sync + 'static,
{
    /// Create new provider client with state store.
    ///
    /// # Arguments
    ///
    /// * `project` - Project name for state namespacing
    /// * `stage` - Stage name (dev, staging, prod)
    /// * `state_store` - State store implementation
    ///
    /// # Example
    ///
    /// ```rust
    /// let state_store = FileStateStore::new("/path", "my-project", "dev");
    /// let client = ProviderClient::new("my-project", "dev", state_store);
    /// ```
    pub fn new(project: &str, stage: &str, state_store: S) -> Self {
        Self {
            state_store: Arc::new(state_store),
            project: project.to_string(),
            stage: stage.to_string(),
        }
    }

    /// Get reference to state store.
    pub fn state_store(&self) -> &S {
        &self.state_store
    }

    /// Get project name.
    pub fn project(&self) -> &str {
        &self.project
    }

    /// Get stage name.
    pub fn stage(&self) -> &str {
        &self.stage
    }
}
```

### StoreStateTask Wrapper

**Design Decision: Custom TaskIterator with State Machine**

We create a proper `TaskIterator` wrapper that uses a state machine to:
1. Spawn the inner API task via `inlined_task`
2. Receive results from the API task stream
3. On success, call `state_store.set()` directly and get back a stream
4. Process the state store stream until completion
5. Yield the original API output

```rust
// foundation_db/src/state/store_state_task.rs
//
// NOTE: This lives in foundation_db - the ResourceIdentifier trait is also
// defined here so we can use it generically without circular dependencies.

use foundation_core::valtron::{
    inlined_task, TaskIterator, TaskStatus,
    InlineSendActionBehaviour, DrivenRecvIterator, BoxedSendExecutionAction,
};
use crate::state::{config_hash, ResourceState, StateStatus, StateStoreStream};
use crate::errors::StorageError;
use foundation_core::valtron::ThreadedValue;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

/// Error type for store state task operations.
///
/// WHY: StoreStateTask needs to report errors from both:
///   1. Inner task operations (generic error type E)
///   2. State store operations (StorageError)
///   3. Serialization/hash errors
///
/// WHAT: Generic enum that can hold either the inner task's error type
///       or a StorageError, plus helper variants.
///
/// HOW: Generic over E (the inner task's error type) so it can wrap
///      any TaskIterator's error.
#[derive(Debug)]
pub enum StoreStateError<E> {
    /// Inner task/operation failed
    TaskFailed(E),
    /// State store operation failed
    State(StorageError),
    /// Serialization failed (input/output to JSON)
    SerializeFailed(String),
    /// Hash computation failed
    HashFailed(String),
    /// Valtron execution failed (scheduling error)
    ExecuteFailed(String),
}

impl<E> From<StorageError> for StoreStateError<E> {
    fn from(err: StorageError) -> Self {
        Self::State(err)
    }
}

/// Configuration for StoreStateTask.
#[derive(Debug, Clone)]
pub struct StoreStateConfig {
    /// Timeout for inline processing of spawned tasks
    pub inline_processing_timeout: Duration,
}

impl Default for StoreStateConfig {
    fn default() -> Self {
        Self {
            inline_processing_timeout: Duration::from_millis(100),
        }
    }
}

/// State machine for StoreStateTask (pre-computed resource info variant).
enum StoreStatePrecomputedState<Inner, S, I, O>
where
    Inner: TaskIterator,
    S: crate::state::traits::StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static,
{
    /// Initial state - spawn the inner task
    Init {
        inner: Inner,
        state_store: Arc<S>,
        resource_id: String,
        resource_kind: String,
        provider: String,
        input: I,
        environment: Option<String>,
    },
    /// Inner task spawned - receiving results
    Receiving {
        state_store: Arc<S>,
        resource_id: String,
        resource_kind: String,
        provider: String,
        input: I,
        environment: Option<String>,
        recv: DrivenRecvIterator<Inner>,
    },
    /// Inner task succeeded - storing result in state store
    StoringResult {
        output: O,
        store_stream: StateStoreStream<()>,
    },
    /// Done - no more states
    Done,
}

/// State machine for StoreStateIdentifierTask (ResourceIdentifier trait variant).
enum StoreStateIdentifierState<Inner, S, I, O>
where
    Inner: TaskIterator,
    S: crate::state::traits::StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static,
{
    /// Initial state - spawn the inner task
    Init {
        inner: Inner,
        state_store: Arc<S>,
        input: I,
        environment: Option<String>,
    },
    /// Inner task spawned - receiving results
    Receiving {
        state_store: Arc<S>,
        input: I,
        environment: Option<String>,
        recv: DrivenRecvIterator<Inner>,
    },
    /// Inner task succeeded - storing result in state store
    StoringResult {
        output: O,
        store_stream: StateStoreStream<()>,
    },
    /// Done - no more states
    Done,
}

/// Pending states for StoreStateTask.
#[derive(Debug, Clone, Copy)]
pub enum StoreStatePending {
    /// Waiting for inner task to produce a result
    WaitingInner,
    /// Waiting for state store to finish storing
    WaitingStore,
}

/// Task wrapper that stores state changes on completion (pre-computed resource info).
///
/// WHY: Operations need to automatically persist their results
///      to the state store without boilerplate in every method.
///
/// WHAT: Wraps any TaskIterator with pre-computed resource info,
///       spawns it, monitors for successful completion,
///       calls state_store.set() and processes the returned stream, then yields
///       the original output.
///
/// HOW: State machine with four states:
///   1. Init - spawn inner task via inlined_task
///   2. Receiving - poll the receiver stream
///   3. StoringResult - poll the state store stream
///   4. Done - complete
#[derive(Debug)]
pub struct StoreStateTask<Inner, S, I, O>
where
    Inner: TaskIterator,
    S: crate::state::traits::StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static,
{
    state: Option<StoreStatePrecomputedState<Inner, S, I, O>>,
    config: StoreStateConfig,
}

/// Task wrapper that stores state changes on completion (uses ResourceIdentifier trait).
///
/// WHY: Operations need to automatically persist their results
///      to the state store without boilerplate in every method.
///
/// WHAT: Wraps any TaskIterator and defers resource ID computation until
///       the output is available via ResourceIdentifier trait.
///
/// HOW: State machine with four states:
///   1. Init - spawn inner task via inlined_task
///   2. Receiving - poll the receiver stream
///   3. StoringResult - poll the state store stream (compute resource info from output)
///   4. Done - complete
#[derive(Debug)]
pub struct StoreStateIdentifierTask<Inner, S, I, O>
where
    Inner: TaskIterator,
    S: crate::state::traits::StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static + super::resource_identifier::ResourceIdentifier<I>,
{
    state: Option<StoreStateIdentifierState<Inner, S, I, O>>,
    config: StoreStateConfig,
    _output: std::marker::PhantomData<O>,
}

impl<Inner, S, I, O> StoreStateTask<Inner, S, I, O>
where
    Inner: TaskIterator,
    S: crate::state::traits::StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static,
{
    /// Create new store state task with pre-computed resource info.
    ///
    /// # Arguments
    ///
    /// * `inner` - The inner task to wrap
    /// * `state_store` - State store for persistence
    /// * `resource_id` - Resource identifier (pre-computed)
    /// * `resource_kind` - Type of resource (e.g., "gcp::cloudkms::AutoKeyConfig")
    /// * `provider` - Provider name (e.g., "gcp")
    /// * `input` - Input configuration (stored as snapshot)
    /// * `environment` - Optional environment name
    /// * `config` - Configuration for task behavior
    pub fn new(
        inner: Inner,
        state_store: Arc<S>,
        resource_id: String,
        resource_kind: String,
        provider: String,
        input: I,
        environment: Option<String>,
        config: StoreStateConfig,
    ) -> Self {
        Self {
            state: Some(StoreStatePrecomputedState::Init {
                inner,
                state_store,
                resource_id,
                resource_kind,
                provider,
                input,
                environment,
            }),
            config,
        }
    }
}

impl<Inner, S, I, O> StoreStateIdentifierTask<Inner, S, I, O>
where
    Inner: TaskIterator,
    S: crate::state::traits::StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Send + 'static + super::resource_identifier::ResourceIdentifier<I>,
{
    /// Create new store state task using ResourceIdentifier trait.
    ///
    /// This constructor defers resource ID computation until the output is available.
    /// The output type must implement ResourceIdentifier<Input>.
    ///
    /// # Arguments
    ///
    /// * `inner` - The inner task to wrap
    /// * `state_store` - State store for persistence
    /// * `input` - Input configuration (stored as snapshot)
    /// * `environment` - Optional environment name
    /// * `config` - Configuration for task behavior
    pub fn with_resource_identifier(
        inner: Inner,
        state_store: Arc<S>,
        input: I,
        environment: Option<String>,
        config: StoreStateConfig,
    ) -> Self {
        Self {
            state: Some(StoreStateIdentifierState::Init {
                inner,
                state_store,
                input,
                environment,
            }),
            config,
            _output: std::marker::PhantomData,
        }
    }
}

impl<Inner, S, I, O, E, P> TaskIterator for StoreStateTask<Inner, S, I, O>
where
    Inner: TaskIterator<Ready = Result<O, E>, Pending = P>,
    S: crate::state::traits::StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Clone + Send + 'static,
    P: Clone + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    type Ready = Result<O, StoreStateError<E>>;
    type Pending = StoreStatePending;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state.take()? {
            StoreStatePrecomputedState::Init {
                inner,
                state_store,
                resource_id,
                resource_kind,
                provider,
                input,
                environment,
            } => {
                // Spawn the inner task
                let (action, recv) = inlined_task(
                    InlineSendActionBehaviour::LiftWithParent,
                    Vec::new(),
                    inner,
                    self.config.inline_processing_timeout,
                );

                self.state = Some(StoreStatePrecomputedState::Receiving {
                    state_store,
                    resource_id,
                    resource_kind,
                    provider,
                    input,
                    environment,
                    recv,
                });

                Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
            }
            StoreStatePrecomputedState::Receiving {
                state_store,
                resource_id,
                resource_kind,
                provider,
                input,
                environment,
                mut recv,
            } => {
                // Poll the inner task receiver (pre-computed resource info variant)
                match recv.next() {
                    Some(TaskStatus::Ready(Ok(output))) => {
                        // Inner task succeeded - build state with pre-computed resource info
                        let config_snapshot = match serde_json::to_value(&input) {
                            Ok(v) => v,
                            Err(e) => {
                                self.state = Some(StoreStatePrecomputedState::Done);
                                return Some(TaskStatus::Ready(Err(StoreStateError::SerializeFailed(e.to_string()))));
                            }
                        };

                        let output_str = match serde_json::to_string(&output) {
                            Ok(s) => s,
                            Err(e) => {
                                self.state = Some(StoreStatePrecomputedState::Done);
                                return Some(TaskStatus::Ready(Err(StoreStateError::SerializeFailed(e.to_string()))));
                            }
                        };

                        let output_value: serde_json::Value = match serde_json::from_str(&output_str) {
                            Ok(v) => v,
                            Err(e) => {
                                self.state = Some(StoreStatePrecomputedState::Done);
                                return Some(TaskStatus::Ready(Err(StoreStateError::SerializeFailed(e.to_string()))));
                            }
                        };

                        let config_hash_value = match config_hash(&config_snapshot) {
                            Ok(h) => h,
                            Err(e) => {
                                self.state = Some(StoreStatePrecomputedState::Done);
                                return Some(TaskStatus::Ready(Err(StoreStateError::HashFailed(e.to_string()))));
                            }
                        };

                        let now = chrono::Utc::now();

                        let state = ResourceState {
                            id: resource_id,
                            kind: resource_kind,
                            provider,
                            status: StateStatus::Created,
                            environment: environment.clone(),
                            config_hash: config_hash_value,
                            output: output_value,
                            config_snapshot,
                            created_at: now,
                            updated_at: now,
                        };

                        match state_store.set(&state.id, &state) {
                            Ok(store_stream) => {
                                self.state = Some(StoreStatePrecomputedState::StoringResult {
                                    output,
                                    store_stream,
                                });
                                Some(TaskStatus::Pending(StoreStatePending::WaitingStore))
                            }
                            Err(e) => {
                                self.state = Some(StoreStatePrecomputedState::Done);
                                Some(TaskStatus::Ready(Err(StoreStateError::State(e))))
                            }
                        }
                    }
                    Some(TaskStatus::Ready(Err(e))) => {
                        self.state = Some(StoreStatePrecomputedState::Done);
                        Some(TaskStatus::Ready(Err(StoreStateError::TaskFailed(e))))
                    }
                    Some(TaskStatus::Pending(_)) => {
                        self.state = Some(StoreStatePrecomputedState::Receiving {
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Pending(StoreStatePending::WaitingInner))
                    }
                    Some(TaskStatus::Init) => {
                        self.state = Some(StoreStatePrecomputedState::Receiving {
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Init)
                    }
                    Some(TaskStatus::Delayed(d)) => {
                        self.state = Some(StoreStatePrecomputedState::Receiving {
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Delayed(d))
                    }
                    Some(TaskStatus::Spawn(action)) => {
                        self.state = Some(StoreStatePrecomputedState::Receiving {
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Spawn(action))
                    }
                    Some(TaskStatus::Ignore) => {
                        self.state = Some(StoreStatePrecomputedState::Receiving {
                            state_store,
                            resource_id,
                            resource_kind,
                            provider,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Ignore)
                    }
                    None => {
                        self.state = Some(StoreStatePrecomputedState::Done);
                        Some(TaskStatus::Ready(Err(StoreStateError::ExecuteFailed(
                            "Inner task receiver exhausted without result".to_string()
                        ))))
                    }
                }
            }
            StoreStatePrecomputedState::StoringResult {
                output,
                mut store_stream,
            } => {
                // Poll the state store stream
                // StateStoreStream only yields ThreadedValue::Value variants
                match store_stream.next() {
                    Some(ThreadedValue::Value(Ok(()))) => {
                        self.state = Some(StoreStatePrecomputedState::StoringResult {
                            output,
                            store_stream,
                        });
                        Some(TaskStatus::Pending(StoreStatePending::WaitingStore))
                    }
                    Some(ThreadedValue::Value(Err(e))) => {
                        self.state = Some(StoreStatePrecomputedState::Done);
                        Some(TaskStatus::Ready(Err(StoreStateError::State(e))))
                    }
                    None => {
                        // State store stream exhausted - storage complete
                        self.state = Some(StoreStatePrecomputedState::Done);
                        Some(TaskStatus::Ready(Ok(output)))
                    }
                }
            }
            StoreStatePrecomputedState::Done => None,
        }
    }
}

impl<Inner, S, I, O, E, P> TaskIterator for StoreStateIdentifierTask<Inner, S, I, O>
where
    Inner: TaskIterator<Ready = Result<O, E>, Pending = P>,
    S: crate::state::traits::StateStore + Send + Sync + 'static,
    I: Serialize + Clone + Send + 'static,
    O: Serialize + Clone + Send + 'static + super::resource_identifier::ResourceIdentifier<I>,
    P: Clone + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    type Ready = Result<O, StoreStateError<E>>;
    type Pending = StoreStatePending;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.state.take()? {
            StoreStateIdentifierState::Init {
                inner,
                state_store,
                input,
                environment,
            } => {
                // Spawn the inner task
                let (action, recv) = inlined_task(
                    InlineSendActionBehaviour::LiftWithParent,
                    Vec::new(),
                    inner,
                    self.config.inline_processing_timeout,
                );

                self.state = Some(StoreStateIdentifierState::Receiving {
                    state_store,
                    input,
                    environment,
                    recv,
                });

                Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
            }
            StoreStateIdentifierState::Receiving {
                state_store,
                input,
                environment,
                mut recv,
            } => {
                // Poll the inner task receiver
                match recv.next() {
                    Some(TaskStatus::Ready(Ok(output))) => {
                        // Inner task succeeded - compute resource info using trait
                        use super::resource_identifier::ResourceIdentifier;
                        
                        let resource_id = output.generate_resource_id(&input);
                        let resource_kind = output.resource_kind();
                        let provider = output.provider();
                        
                        let config_snapshot = match serde_json::to_value(&input) {
                            Ok(v) => v,
                            Err(e) => {
                                self.state = Some(StoreStateIdentifierState::Done);
                                return Some(TaskStatus::Ready(Err(StoreStateError::SerializeFailed(e.to_string()))));
                            }
                        };

                        let output_str = match serde_json::to_string(&output) {
                            Ok(s) => s,
                            Err(e) => {
                                self.state = Some(StoreStateIdentifierState::Done);
                                return Some(TaskStatus::Ready(Err(StoreStateError::SerializeFailed(e.to_string()))));
                            }
                        };

                        let output_value: serde_json::Value = match serde_json::from_str(&output_str) {
                            Ok(v) => v,
                            Err(e) => {
                                self.state = Some(StoreStateIdentifierState::Done);
                                return Some(TaskStatus::Ready(Err(StoreStateError::SerializeFailed(e.to_string()))));
                            }
                        };

                        let config_hash_value = match config_hash(&config_snapshot) {
                            Ok(h) => h,
                            Err(e) => {
                                self.state = Some(StoreStateIdentifierState::Done);
                                return Some(TaskStatus::Ready(Err(StoreStateError::HashFailed(e.to_string()))));
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
                            Ok(store_stream) => {
                                self.state = Some(StoreStateIdentifierState::StoringResult {
                                    output,
                                    store_stream,
                                });
                                Some(TaskStatus::Pending(StoreStatePending::WaitingStore))
                            }
                            Err(e) => {
                                self.state = Some(StoreStateIdentifierState::Done);
                                Some(TaskStatus::Ready(Err(StoreStateError::State(e))))
                            }
                        }
                    }
                    Some(TaskStatus::Ready(Err(e))) => {
                        self.state = Some(StoreStateIdentifierState::Done);
                        Some(TaskStatus::Ready(Err(StoreStateError::TaskFailed(e))))
                    }
                    Some(TaskStatus::Pending(_)) => {
                        self.state = Some(StoreStateIdentifierState::Receiving {
                            state_store,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Pending(StoreStatePending::WaitingInner))
                    }
                    Some(TaskStatus::Init) => {
                        self.state = Some(StoreStateIdentifierState::Receiving {
                            state_store,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Init)
                    }
                    Some(TaskStatus::Delayed(d)) => {
                        self.state = Some(StoreStateIdentifierState::Receiving {
                            state_store,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Delayed(d))
                    }
                    Some(TaskStatus::Spawn(action)) => {
                        self.state = Some(StoreStateIdentifierState::Receiving {
                            state_store,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Spawn(action))
                    }
                    Some(TaskStatus::Ignore) => {
                        self.state = Some(StoreStateIdentifierState::Receiving {
                            state_store,
                            input,
                            environment,
                            recv,
                        });
                        Some(TaskStatus::Ignore)
                    }
                    None => {
                        self.state = Some(StoreStateIdentifierState::Done);
                        Some(TaskStatus::Ready(Err(StoreStateError::ExecuteFailed(
                            "Inner task receiver exhausted without result".to_string()
                        ))))
                    }
                }
            }
            StoreStateIdentifierState::StoringResult {
                output,
                mut store_stream,
            } => {
                // Poll the state store stream
                match store_stream.next() {
                    Some(ThreadedValue::Value(Ok(()))) => {
                        self.state = Some(StoreStateIdentifierState::StoringResult {
                            output,
                            store_stream,
                        });
                        Some(TaskStatus::Pending(StoreStatePending::WaitingStore))
                    }
                    Some(ThreadedValue::Value(Err(e))) => {
                        self.state = Some(StoreStateIdentifierState::Done);
                        Some(TaskStatus::Ready(Err(StoreStateError::State(e))))
                    }
                    None => {
                        // State store stream exhausted - storage complete
                        self.state = Some(StoreStateIdentifierState::Done);
                        Some(TaskStatus::Ready(Ok(output)))
                    }
                }
            }
            StoreStateIdentifierState::Done => None,
        }
    }
}
```

### Per-API Provider Structure

```rust
// providers/gcp/api/cloudkms.rs

use super::ProviderClient;
use super::error::ProviderError;
use crate::providers::gcp::clients::cloudkms::*;
use foundation_db::state::store_state_task::{StoreStateTask, StoreStateError};
use foundation_core::valtron::{execute, StreamIterator};
use std::sync::Arc;

/// Cloud KMS API provider.
///
/// WHY: Users need a state-aware API for Cloud KMS operations.
///
/// WHAT: Wrapper around ProviderClient providing methods for each Cloud KMS endpoint.
///
/// HOW: Each method builds request, creates task, wraps with StoreStateTask, executes.
#[derive(Debug, Clone)]
pub struct CloudKmsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
}

impl<S> CloudKmsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new Cloud KMS provider.
    pub fn new(client: ProviderClient<S>) -> Self {
        Self { client }
    }

    /// Update autokey config for a folder.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments with folder name and config
    ///
    /// # Returns
    ///
    /// StreamIterator yielding Result<AutoKeyConfig, ProviderError>
    ///
    /// # State Tracking
    ///
    /// On success, automatically stores:
    /// - Input: CloudkmsFoldersUpdateAutokeyConfigArgs
    /// - Output: AutoKeyConfig response
    /// - Resource ID: Generated from input.name (based on OpenAPI spec)
    /// - Resource Kind: "gcp::cloudkms::AutoKeyConfig"
    /// - Environment: stage name
    pub fn folders_update_autokey_config(
        &self,
        args: CloudkmsFoldersUpdateAutokeyConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoKeyConfig, ProviderError>,
            P = ApiPending,
        > + Send
        + 'static,
        ProviderError,
    > {
        use gcp::resource_info::CloudkmsFoldersUpdateAutokeyConfigResourceInfo as ResourceInfo;
        
        let input = args.clone();
        let state_store = Arc::clone(&self.client.state_store);
        let stage = self.client.stage.clone();
        
        // Step 1: Build request
        let builder = cloudkms_folders_update_autokey_config_builder(
            self.client.state_store(),
            &args.name,
            args.updateMask.as_deref(),
            &args.body,
        )?;
        
        // Step 2: Create task
        let task = cloudkms_folders_update_autokey_config_task(builder)?;
        
        // Step 3: Compute resource info from input using generated helper
        let resource_id = ResourceInfo::compute_resource_id(&args);
        let resource_kind = ResourceInfo::resource_kind();
        let provider = ResourceInfo::provider();
        
        // Step 4: Wrap with StoreStateTask (from foundation_db)
        let store_task = StoreStateTask::new(
            task,
            state_store,
            resource_id,
            resource_kind.to_string(),
            provider.to_string(),
            input,
            Some(stage),
            StoreStateConfig::default(),
        );
        
        // Step 5: Execute and map errors
        execute(store_task, None)
            .map_err(|e| match e {
                StoreStateError::TaskFailed(api_err) => ProviderError::Api(api_err),
                StoreStateError::State(storage_err) => ProviderError::State(storage_err),
                StoreStateError::SerializeFailed(msg) => ProviderError::SerializeFailed(msg),
                StoreStateError::HashFailed(msg) => ProviderError::HashFailed(msg),
                StoreStateError::ExecuteFailed(msg) => ProviderError::ExecuteFailed(msg),
            })
    }
}
```

## Code Generation

### ResourceIdentifier - Automated Generation Approach

**Primary approach: Automated code generation with deterministic rules.**

The ResourceIdentifier implementations CAN be generated automatically by analyzing the OpenAPI spec. The generator should follow a deterministic priority order based on what fields exist in the response schema.

**Algorithm for determining resource ID:**

1. **Extract response schema** for each operation from `paths.{path}.{method}.responses.200`
2. **Analyze response schema fields** in priority order:
   - **Priority 1:** `id` field exists → use `self.id`
   - **Priority 2:** `name` field exists → use `self.name`
   - **Priority 3:** `self_link`, `resource_name`, or `href` exists → use that field
   - **Priority 4:** No identifier in response → fall back to input path parameters

3. **Generate resource kind** from provider + schema name:
   - Format: `{provider}::{schema_name}` (e.g., `cloudflare::Zone`)
   - For nested APIs: `{provider}::{api}::{schema_name}` (e.g., `gcp::cloudkms::AutoKeyConfig`)

4. **Determine input type** from operation request body or path parameters

**Example Analysis for Cloudflare Zone:**

```json
// Path: /zones/{zone_id}
// Operation: GET zones-0-get
// Response schema: zones_zone (has `id` field)
// Path params: zone_id

// Generated ResourceIdentifier:
impl ResourceIdentifier<ZoneGetArgs> for Zone {
    fn generate_resource_id(&self, _input: &ZoneGetArgs) -> String {
        format!("cloudflare::zone/{}", self.id)  // self.id takes priority
    }
    
    fn resource_kind(&self) -> &'static str {
        "cloudflare::Zone"
    }
    
    fn provider(&self) -> &'static str {
        "cloudflare"
    }
}
```

**Example where input path is needed (rare):**

```json
// Path: /zones/{zone_id}/access/apps/{app_id}/policies/{policy_id}
// Response schema: AccessPolicy (has `id` field)
// Path params: zone_id, app_id, policy_id

// Generated ResourceIdentifier:
impl ResourceIdentifier<AccessPolicyGetArgs> for AccessPolicy {
    fn generate_resource_id(&self, input: &AccessPolicyGetArgs) -> String {
        // Nested resource: needs parent path + own id
        format!("cloudflare::access::policy/{}/{}/{}", input.zone_id, input.app_id, self.id)
    }
    
    fn resource_kind(&self) -> &'static str {
        "cloudflare::access::Policy"
    }
    
    fn provider(&self) -> &'static str {
        "cloudflare"
    }
}
```

### Implementation in gen_resources/types.rs

Add the following methods to `ResourceGenerator`:

**1. `extract_response_schema_for_operation()`** - Get response type for a path+method:
```rust
fn extract_response_schema_for_operation(
    &self,
    paths: &BTreeMap<String, PathItem>,
    path: &str,
    method: &str,
) -> Option<String>
```

**2. `analyze_schema_for_identifier_field()`** - Find identifier field in schema:
```rust
fn analyze_schema_for_identifier_field(
    &self,
    schema_name: &str,
    spec: &Value,
) -> IdentifierField {
    // Returns: Id, Name, SelfLink, or None
}

enum IdentifierField {
    Id,      // use self.id
    Name,    // use self.name  
    Other(&'static str),  // use self.{field}
    None,    // fall back to input
}
```

**3. `extract_path_parameters()`** - Get path params from URL pattern:
```rust
fn extract_path_parameters(&self, path: &str) -> Vec<String>
// e.g., "/zones/{zone_id}" -> ["zone_id"]
```

**4. `generate_resource_identifier_impl()`** - Generate the trait impl:
```rust
fn generate_resource_identifier_impl(
    &self,
    output_type: &str,
    input_type: &str,
    identifier_field: IdentifierField,
    path_params: &[String],
    provider: &str,
    api_name: Option<&str>,
) -> String
```

### Manual Review Cases (Edge Cases Only)

Manual review is only needed for edge cases where automated generation cannot determine the correct pattern:

1. **Schema has multiple potential identifiers** - e.g., both `id` and `name` but API docs say to use `name`
2. **Composite identifiers** - where resource ID is combination of multiple fields
3. **Non-standard patterns** - APIs that don't follow REST conventions
4. **Ambiguous nested resources** - where parent path params may or may not be needed

For these cases, create an override file:
```
backends/foundation_deployment/src/providers/{provider}/resources/resource_identifier_overrides.rs
```

The override file takes precedence over generated impls.

### ResourceInfo Generator (separate from types.rs)

A separate generator (to be added in `gen_resources/providers.rs`) creates `ResourceInfo` helper structs for each endpoint:

```rust
// Generated in providers/gcp/resource_info/cloudkms.rs

/// ResourceInfo helper for CloudKms.Folders.UpdateAutokeyConfig endpoint.
///
/// WHY: Provides static methods to compute resource ID/kind/provider
///      without needing an output instance.
///
/// WHAT: Generated helper struct with static methods.
///
/// HOW: Generator analyzes OpenAPI spec to determine resource ID pattern.
pub struct CloudkmsFoldersUpdateAutokeyConfigResourceInfo;

impl ResourceInfo<CloudkmsFoldersUpdateAutokeyConfigArgs> 
    for CloudkmsFoldersUpdateAutokeyConfigResourceInfo
{
    fn compute_resource_id(input: &CloudkmsFoldersUpdateAutokeyConfigArgs) -> String {
        format!("gcp::cloudkms::AutoKeyConfig/folders/{}", input.name)
    }
    
    fn resource_kind() -> &'static str {
        "gcp::cloudkms::AutoKeyConfig"
    }
    
    fn provider() -> &'static str {
        "gcp"
    }
}
```

### New `gen_resources providers` Subcommand

Add a new generator that creates per-API provider wrappers and `ResourceInfo` trait implementations:

```rust
// bin/platform/src/gen_resources/providers.rs

/// Generate per-API provider wrappers.
///
/// For each API (e.g., cloudkms, compute, run):
/// 1. Generate ResourceInfo helper structs for each endpoint
/// 2. Create {Api}Provider struct with ProviderClient<S> field
/// 3. Generate methods for each create/modify/delete endpoint
/// 4. Each method uses ResourceInfo to compute resource_id/kind/provider
/// 5. Each method wraps the _task() with StoreStateTask (pre-computed variant)
/// 6. For output types with ResourceIdentifier impl, use StoreStateIdentifierTask
/// 7. Return StreamIterator with automatic state tracking
```

### Generated File Structure

```
backends/foundation_deployment/src/providers/gcp/
├── api/
│   ├── mod.rs              # ProviderClient<S>
│   ├── error.rs            # ProviderError (unified Api + State errors)
│   ├── cloudkms.rs         # CloudKmsProvider<S> (generated)
│   ├── compute.rs          # ComputeProvider<S> (generated)
│   └── run.rs              # RunProvider<S> (generated)
├── resource_info/          # Generated ResourceInfo helpers
│   ├── mod.rs
│   ├── cloudkms.rs         # helpers for CloudKms endpoints
│   ├── compute.rs          # helpers for Compute endpoints
│   └── run.rs              # helpers for Run endpoints
├── resources/              # Generated types with ResourceIdentifier impls
│   ├── mod.rs
│   ├── cloudkms.rs         # types + ResourceIdentifier impls for CloudKms
│   ├── compute.rs          # types + ResourceIdentifier impls for Compute
│   └── run.rs              # types + ResourceIdentifier impls for Run
│   └── resource_identifier_overrides.rs  # Manual overrides (if needed)
├── clients/                # Existing generated clients
│   ├── cloudkms.rs
│   ├── compute.rs
│   └── run.rs
└── tasks/
    └── mod.rs              # Re-exports StoreStateTask, StoreStateIdentifierTask from foundation_db
```

```
backends/foundation_deployment/src/providers/cloudflare/
├── api/
│   ├── mod.rs              # ProviderClient<S>
│   ├── error.rs            # ProviderError
│   ├── worker.rs           # WorkerProvider<S> (generated)
│   ├── zone.rs             # ZoneProvider<S> (generated)
│   └── dns.rs              # DnsProvider<S> (generated)
├── resource_info/          # Generated ResourceInfo helpers
│   ├── mod.rs
│   ├── worker.rs           # helpers for Worker endpoints
│   ├── zone.rs             # helpers for Zone endpoints
│   └── dns.rs              # helpers for Dns endpoints
├── resources/              # Generated types + ResourceIdentifier impls
│   ├── mod.rs
│   ├── worker.rs           # Worker, WorkerScript, etc. + impls
│   ├── zone.rs             # Zone, ZoneSettings, etc. + impls
│   └── dns.rs              # DnsRecord, etc. + impls
│   └── resource_identifier_overrides.rs  # Manual overrides (if needed)
├── clients/                # Existing generated clients
│   ├── worker.rs
│   ├── zone.rs
│   └── dns.rs
└── tasks/
    └── mod.rs              # Re-exports StoreStateTask, StoreStateIdentifierTask from foundation_db
```

**Note:** 
- ResourceIdentifier trait implementations are generated alongside the types in `resources/{api}.rs` files.
- Manual override file (`resource_identifier_overrides.rs`) is only created when automated generation produces incorrect results for edge cases.

## Tasks

1. **Implement ResourceIdentifier trait in foundation_db** (DONE)
   - [x] Create `foundation_db/src/state/resource_identifier.rs`
   - [x] Define `ResourceIdentifier<Input>` trait with instance methods:
     - `generate_resource_id(&self, input: &Input) -> String`
     - `resource_kind(&self) -> &'static str`
     - `provider(&self) -> &'static str`
   - [x] Export from `foundation_db::state::resource_identifier`

2. **Implement ProviderClient** (DONE)
   - [x] Create `foundation_deployment/src/provider_client.rs`
   - [x] Generic over `S: StateStore`
   - [x] Store project + stage metadata

3. **Create gen_resources providers command** (DONE)
   - [x] Add `gen_resources providers` subcommand to CLI
   - [x] Scans generated client files for endpoint functions using regex
   - [x] Groups endpoints by API (for multi-spec providers like GCP)
   - [x] Generates provider wrappers with automatic state tracking

4. **Implement provider wrapper generator** (DONE)
   - [x] Create `bin/platform/src/gen_resources/provider_wrappers.rs`
   - [x] Discover endpoints by scanning client files
   - [x] Generate Provider struct per API with methods for mutating endpoints
   - [x] Each method wraps task with StoreStateIdentifierTask
   - [x] Handle APIs with no mutating endpoints gracefully
   - [x] Fix duplicate "Provider" suffix in struct names (e.g., CloudKmsProviderProvider)

5. **Fix client generator bugs** (DONE)
   - [x] Remove erroneous `pub mod types;` from individual client files
   - [x] Keep `pub mod types;` only in clients/mod.rs (shared types module)

6. **Generate provider wrappers for all providers** (IN PROGRESS)
   - [x] GCP - generator implemented, fixes applied
   - [ ] Cloudflare
   - [ ] Fly.io
   - [ ] Neon
   - [ ] PlanetScale
   - [ ] Prisma Postgres
   - [ ] Stripe
   - [ ] Supabase

7. **Documentation**
   - [ ] Document ProviderClient usage
   - [ ] Document per-API provider pattern
   - [ ] Document StoreStateIdentifierTask usage
   - [ ] Add examples for each provider

8. **Verification**
   - [ ] `cargo check -p foundation_deployment` passes
   - [ ] `cargo check -p ewe_platform` passes
   - [ ] Generated provider wrappers compile without errors
    - [ ] Zero clippy warnings
    - [ ] Zero rustdoc warnings
    - [ ] State store automatically updated on operations

## Example Usage

```rust
use foundation_deployment::providers::gcp::api::{ProviderClient, CloudKmsProvider};
use foundation_deployment::providers::gcp::error::ProviderError;
use foundation_db::state::FileStateStore;

// Create state store
let state_store = FileStateStore::new("/path/to/state", "my-project", "dev");
state_store.init()?;

// Create provider client
let client = ProviderClient::new("my-project", "dev", state_store);

// Create per-API provider
let cloud_kms = CloudKmsProvider::new(client);

// Call API method - state automatically tracked
let result_stream = cloud_kms.folders_update_autokey_config(
    CloudkmsFoldersUpdateAutokeyConfigArgs {
        name: "folders/123".to_string(),
        updateMask: Some("name,description".to_string()),
        body: AutoKeyConfig { /* ... */ },
    }
)?;

// Execute and get results
for item in result_stream {
    match item {
        Stream::Pending(p) => println!("Pending: {:?}", p),
        Stream::Next(Ok(config)) => {
            println!("Success: {:?}", config);
            // State automatically stored in state store:
            // - Resource ID: "cloudkms/folders/123"
            // - Input: AutoKeyConfig args
            // - Output: Updated AutoKeyConfig response
            // - Status: Created
        }
        Stream::Next(Err(ProviderError::Api(e))) => {
            eprintln!("API error: {:?}", e);
        }
        Stream::Next(Err(ProviderError::State(e))) => {
            eprintln!("State store error: {:?}", e);
        }
        Stream::Next(Err(e)) => {
            eprintln!("Other error: {:?}", e);
        }
    }
}
```

## Success Criteria

- [ ] All 8 tasks completed
- [ ] StoreStateTask correctly wraps any TaskIterator
- [ ] ProviderClient generic over StateStore
- [ ] Per-API providers generated for all endpoints
- [ ] State automatically updated on create/modify/delete
- [ ] Errors properly propagated
- [ ] Code compiles with zero warnings
- [ ] All tests pass

## Verification

```bash
cd /home/darkvoid/Boxxed/@dev/ewe_platform

# Check compilation
cargo check -p foundation_deployment

# Run tests
cargo test -p foundation_deployment

# Verify state tracking
cargo test provider_state_tracking -- --nocapture

# Verify code generation
cargo run --bin ewe_platform gen_resources providers --provider gcp
```

---

_Created: 2026-04-08_
