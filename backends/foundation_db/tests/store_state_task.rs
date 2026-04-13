//! Integration tests for `StoreStateTask` and `StoreStateIdentifierTask`.

use foundation_core::valtron::TaskIterator;
use foundation_db::state::file::FileStateStore;
use foundation_db::state::resource_identifier::ResourceIdentifier;
use foundation_db::state::store_state_task::{StoreStateIdentifierTask, StoreStateTask};
use foundation_db::state::traits::StateStore;
use foundation_db::StorageError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================================
// Test helpers
// ============================================================================

/// Simple mock task that succeeds immediately.
#[derive(Debug, Clone)]
struct MockSuccessTask<T> {
    value: T,
    called: bool,
}

impl<T: Clone + Send + 'static> MockSuccessTask<T> {
    fn new(value: T) -> Self {
        Self { value, called: false }
    }
}

impl<T: Clone + Send + 'static> TaskIterator for MockSuccessTask<T> {
    type Ready = Result<T, String>;
    type Pending = ();
    type Spawner = foundation_core::valtron::BoxedSendExecutionAction;

    fn next_status(
        &mut self,
    ) -> Option<foundation_core::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>
    {
        if self.called {
            None
        } else {
            self.called = true;
            Some(foundation_core::valtron::TaskStatus::Ready(Ok(self.value.clone())))
        }
    }
}

/// Simple mock task that fails immediately with `TestResource` output.
#[derive(Debug, Clone)]
struct MockFailResourceTask {
    error: String,
    called: bool,
}

impl MockFailResourceTask {
    fn new(error: String) -> Self {
        Self { error, called: false }
    }
}

impl TaskIterator for MockFailResourceTask {
    type Ready = Result<TestResource, String>;
    type Pending = ();
    type Spawner = foundation_core::valtron::BoxedSendExecutionAction;

    fn next_status(
        &mut self,
    ) -> Option<foundation_core::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>
    {
        if self.called {
            None
        } else {
            self.called = true;
            Some(foundation_core::valtron::TaskStatus::Ready(Err(self.error.clone())))
        }
    }
}

/// Simple mock task that fails immediately.
#[derive(Debug, Clone)]
struct MockFailTask {
    error: String,
    called: bool,
}

impl MockFailTask {
    fn new(error: String) -> Self {
        Self { error, called: false }
    }
}

impl TaskIterator for MockFailTask {
    type Ready = Result<String, String>;
    type Pending = ();
    type Spawner = foundation_core::valtron::BoxedSendExecutionAction;

    fn next_status(
        &mut self,
    ) -> Option<foundation_core::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>
    {
        if self.called {
            None
        } else {
            self.called = true;
            Some(foundation_core::valtron::TaskStatus::Ready(Err(self.error.clone())))
        }
    }
}

/// Test output type with `ResourceIdentifier` implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestResource {
    id: String,
    name: String,
}

impl ResourceIdentifier<TestArgs> for TestResource {
    fn generate_resource_id(&self, _input: &TestArgs) -> String {
        format!("test::Resource/{}", self.id)
    }

    fn resource_kind(&self) -> &'static str {
        "test::Resource"
    }

    fn provider(&self) -> &'static str {
        "test"
    }
}

/// Test input type.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestArgs {
    name: String,
}

// ============================================================================
// StoreStateTask tests
// ============================================================================

#[test]
fn test_store_state_task_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_store = FileStateStore::new(temp_dir.path(), "test-project", "dev");
    state_store.init().expect("Failed to init state store");

    let output = TestResource {
        id: "res-123".to_string(),
        name: "Test Resource".to_string(),
    };

    let input = TestArgs {
        name: "test".to_string(),
    };

    let inner_task = MockSuccessTask::new(output.clone());

    let store_task = StoreStateTask::new(
        inner_task,
        Arc::new(state_store),
        "test::Resource/res-123".to_string(),
        "test::Resource".to_string(),
        "test".to_string(),
        input,
        Some("dev".to_string()),
    );

    // Drive the task to completion
    let mut task = store_task;
    let mut found_success = false;

    while let Some(status) = task.next_status() {
        match status {
            foundation_core::valtron::TaskStatus::Ready(Ok(result)) => {
                assert_eq!(result.id, "res-123");
                found_success = true;
            }
            foundation_core::valtron::TaskStatus::Ready(Err(e)) => {
                panic!("Task failed with error: {e}");
            }
            _ => {}
        }
    }

    assert!(found_success, "Task should have yielded a successful result");

    // Verify state was stored
    let state_store = FileStateStore::new(temp_dir.path(), "test-project", "dev");
    let mut get_stream = state_store
        .get("test::Resource/res-123")
        .expect("Failed to get state");
    let stored_state = get_stream.next().expect("Stream should yield value");

    match stored_state {
        foundation_core::valtron::ThreadedValue::Value(Ok(Some(state))) => {
            assert_eq!(state.id, "test::Resource/res-123");
            assert_eq!(state.kind, "test::Resource");
            assert_eq!(state.provider, "test");
        }
        foundation_core::valtron::ThreadedValue::Value(_) => panic!("Expected stored state"),
    }
}

#[test]
fn test_store_state_task_inner_failure() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_store = FileStateStore::new(temp_dir.path(), "test-project", "dev");
    state_store.init().expect("Failed to init state store");

    let input = TestArgs {
        name: "test".to_string(),
    };

    let inner_task = MockFailTask::new("inner error".to_string());

    let store_task = StoreStateTask::new(
        inner_task,
        Arc::new(state_store),
        "test::Resource/res-123".to_string(),
        "test::Resource".to_string(),
        "test".to_string(),
        input,
        Some("dev".to_string()),
    );

    // Drive the task to completion
    let mut task = store_task;
    let mut found_error = false;

    while let Some(status) = task.next_status() {
        match status {
            foundation_core::valtron::TaskStatus::Ready(Err(_)) => {
                found_error = true;
            }
            foundation_core::valtron::TaskStatus::Ready(Ok(_)) => {
                panic!("Task should have failed");
            }
            _ => {}
        }
    }

    assert!(found_error, "Task should have yielded an error");

    // Verify state was NOT stored
    let state_store = FileStateStore::new(temp_dir.path(), "test-project", "dev");
    let mut get_stream = state_store
        .get("test::Resource/res-123")
        .expect("Failed to get state");
    let stored_state = get_stream.next().expect("Stream should yield value");

    match stored_state {
        foundation_core::valtron::ThreadedValue::Value(Ok(None)) => {
            // Expected - state should not exist
        }
        foundation_core::valtron::ThreadedValue::Value(_) => {
            panic!("Expected no state to be stored")
        }
    }
}

// ============================================================================
// StoreStateIdentifierTask tests
// ============================================================================

#[test]
fn test_store_state_identifier_task_success() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_store = FileStateStore::new(temp_dir.path(), "test-project", "dev");
    state_store.init().expect("Failed to init state store");

    let output = TestResource {
        id: "res-456".to_string(),
        name: "Test Resource 2".to_string(),
    };

    let input = TestArgs {
        name: "test".to_string(),
    };

    let inner_task = MockSuccessTask::new(output.clone());

    let store_task = StoreStateIdentifierTask::new(
        inner_task,
        Arc::new(state_store),
        input,
        Some("dev".to_string()),
    );

    // Drive the task to completion
    let mut task = store_task;
    let mut found_success = false;

    while let Some(status) = task.next_status() {
        match status {
            foundation_core::valtron::TaskStatus::Ready(Ok(result)) => {
                assert_eq!(result.id, "res-456");
                found_success = true;
            }
            foundation_core::valtron::TaskStatus::Ready(Err(e)) => {
                panic!("Task failed with error: {e}");
            }
            _ => {}
        }
    }

    assert!(found_success, "Task should have yielded a successful result");

    // Verify state was stored with correct resource ID from trait
    let state_store = FileStateStore::new(temp_dir.path(), "test-project", "dev");
    let mut get_stream = state_store
        .get("test::Resource/res-456")
        .expect("Failed to get state");
    let stored_state = get_stream.next().expect("Stream should yield value");

    match stored_state {
        foundation_core::valtron::ThreadedValue::Value(Ok(Some(state))) => {
            assert_eq!(state.id, "test::Resource/res-456");
            assert_eq!(state.kind, "test::Resource");
            assert_eq!(state.provider, "test");
        }
        foundation_core::valtron::ThreadedValue::Value(_) => panic!("Expected stored state"),
    }
}

#[test]
fn test_store_state_identifier_task_inner_failure() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let state_store = FileStateStore::new(temp_dir.path(), "test-project", "dev");
    state_store.init().expect("Failed to init state store");

    let input = TestArgs {
        name: "test".to_string(),
    };

    let inner_task = MockFailResourceTask::new("identifier task error".to_string());

    let store_task = StoreStateIdentifierTask::new(
        inner_task,
        Arc::new(state_store),
        input,
        Some("dev".to_string()),
    );

    // Drive the task to completion
    let mut task = store_task;
    let mut found_error = false;

    while let Some(status) = task.next_status() {
        match status {
            foundation_core::valtron::TaskStatus::Ready(Err(_)) => {
                found_error = true;
            }
            foundation_core::valtron::TaskStatus::Ready(Ok(_)) => {
                panic!("Task should have failed");
            }
            _ => {}
        }
    }

    assert!(found_error, "Task should have yielded an error");

    // Verify state was NOT stored
    let state_store = FileStateStore::new(temp_dir.path(), "test-project", "dev");
    let mut get_stream = state_store
        .get("test::Resource/res-456")
        .expect("Failed to get state");
    let stored_state = get_stream.next().expect("Stream should yield value");

    match stored_state {
        foundation_core::valtron::ThreadedValue::Value(Ok(None)) => {
            // Expected - state should not exist
        }
        foundation_core::valtron::ThreadedValue::Value(_) => {
            panic!("Expected no state to be stored")
        }
    }
}

// ============================================================================
// ProviderError tests
// ============================================================================

#[test]
fn test_provider_error_display() {
    use foundation_db::state::store_state_task::ProviderError;

    let api_err: ProviderError<String> = ProviderError::Api("api error".to_string());
    assert!(api_err.to_string().contains("API error"));

    let state_err: ProviderError<String> =
        ProviderError::State(StorageError::Serialization(
            "serde error".to_string(),
        ));
    assert!(state_err.to_string().contains("state store error"));

    let serialize_err: ProviderError<String> =
        ProviderError::SerializeFailed("serde error".to_string());
    assert!(serialize_err.to_string().contains("serialization failed"));

    let hash_err: ProviderError<String> = ProviderError::HashFailed("hash error".to_string());
    assert!(hash_err.to_string().contains("hash failed"));

    let exec_err: ProviderError<String> =
        ProviderError::ExecuteFailed("exec error".to_string());
    assert!(exec_err.to_string().contains("execution failed"));
}

#[test]
fn test_provider_error_from_storage_error() {
    use foundation_db::state::store_state_task::ProviderError;

    let storage_err = StorageError::Serialization(
        "test error".to_string(),
    );
    let provider_err: ProviderError<String> = storage_err.into();

    match provider_err {
        ProviderError::State(_) => (),
        _ => panic!("Expected State variant"),
    }
}

#[test]
fn test_provider_error_map() {
    use foundation_db::state::store_state_task::ProviderError;

    let api_err: ProviderError<&str> = ProviderError::Api("original");
    let mapped: ProviderError<String> = api_err.map(std::string::ToString::to_string);

    match mapped {
        ProviderError::Api(s) => assert_eq!(s, "original"),
        _ => panic!("Expected Api variant"),
    }
}
