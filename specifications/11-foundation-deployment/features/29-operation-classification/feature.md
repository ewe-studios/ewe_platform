---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/29-operation-classification"
this_file: "specifications/11-foundation-deployment/features/29-operation-classification/feature.md"

status: complete
priority: high
created: 2026-04-10
completed: 2026-04-10

depends_on: ["28-provider-wrapper"]

tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# Operation Type Classification for Provider Wrappers

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_openapi -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_openapi --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_openapi` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Add semantic operation type classification to API endpoints to determine which operations should wrap tasks with `StoreStateIdentifierTask`. Only operations that modify resource state (Create, Update, Delete, and some Actions) should track state. Read-only operations should execute without state tracking.

**Key Benefits:**
1. **Accurate state tracking** - Only mutating operations wrap with state store
2. **Cleaner provider wrappers** - Read operations have simple execute methods
3. **Semantic understanding** - Classification based on operation semantics, not just HTTP method
4. **Consistent API** - All operations available through provider wrappers with appropriate handling

## Operation Type Classification

### OperationType Enum

```rust
/// Classification of endpoint operation type.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    /// Creates a new resource (e.g., createInstance, insertRow)
    Create,
    /// Reads/fetches resource state without modification (e.g., get, list, search)
    Read,
    /// Updates existing resource (e.g., update, patch, modify)
    Update,
    /// Removes resource (e.g., delete, remove, destroy)
    Delete,
    /// Action that may or may not modify state (e.g., cancel, trigger, invoke)
    /// Requires individual classification based on action semantics
    Action(OperationEffect),
}

/// Effect classification for Action operations.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationEffect {
    /// Action modifies state (e.g., cancelOperation, startInstance)
    Mutating,
    /// Action is informational only (e.g., testIamPermissions, export)
    ReadOnly,
}
```

### Classification Heuristics

| Operation Type | HTTP Method | Operation ID Keywords | Path Pattern | State Tracking |
|---------------|-------------|----------------------|--------------|----------------|
| Create | POST/PUT | create, insert, add, register, new | collection | Yes |
| Read | GET/POST | get, list, fetch, retrieve, search, query, describe | resource or collection | No |
| Update | PUT/PATCH/POST | update, patch, modify, replace, set | resource/{id} | Yes |
| Delete | DELETE | delete, remove, destroy, unpublish | resource/{id} | Yes |
| Action (Mutating) | POST | cancel, start, stop, restart, activate, deactivate | resource/{id}/action | Yes |
| Action (ReadOnly) | POST | test, validate, export, analyze, inspect | resource/{id}/action | No |

### Default Classification Rules

1. **If operation_id contains explicit keyword** → use keyword classification
2. **If HTTP method is GET** → default to Read
3. **If HTTP method is DELETE** → default to Delete
4. **If HTTP method is PUT/PATCH** → default to Update
5. **If HTTP method is POST**:
   - Path ends with `:actionName` → Action (classify by action name)
   - Operation_id contains action keyword → Action
   - Otherwise → default to Create if no resource ID in path, Update if resource ID present

## Architecture

### Classification Flow

```
OpenAPI Spec
      │
      ▼
┌─────────────────────────────┐
│  EndpointExtractor          │
│  - Extracts raw endpoints   │
│  - operation_id, method,    │
│    path, request_type       │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│  OperationTypeClassifier    │
│  - Analyzes operation_id    │
│  - Analyzes path pattern    │
│  - Considers HTTP method    │
│  - Applies heuristics       │
│  - Returns OperationType    │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│  EndpointInfo.operation_type│
│  - Stored with endpoint     │
│  - Available for codegen    │
└─────────────────────────────┘
```

### Provider Wrapper Generation Flow

```
ApiCatalog
      │
      ▼
┌─────────────────────────────────────┐
│  For each endpoint:                 │
│  - Check operation_type             │
│  - If Create/Update/Delete/Action:  │
│    → Generate method with           │
│      StoreStateIdentifierTask       │
│  - If Read:                         │
│    → Generate method with direct    │
│      execute() call                 │
└─────────────────────────────────────┘
```

## Implementation Details

### 1. Add OperationType to foundation_openapi

**File:** `backends/foundation_openapi/src/endpoint.rs`

```rust
/// Classification of endpoint operation type.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationType {
    Create,
    Read,
    Update,
    Delete,
    Action(OperationEffect),
}

/// Effect classification for Action operations.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationEffect {
    Mutating,
    ReadOnly,
}

impl OperationType {
    /// Check if this operation type should wrap with StoreStateIdentifierTask.
    pub fn requires_state_tracking(&self) -> bool {
        match self {
            OperationType::Create | OperationType::Update | OperationType::Delete => true,
            OperationType::Read => false,
            OperationType::Action(effect) => matches!(effect, OperationEffect::Mutating),
        }
    }
}
```

### 2. Add Classifier Module

**File:** `backends/foundation_openapi/src/classifier.rs` (new)

```rust
//! Operation type classification for API endpoints.

use crate::endpoint::{EndpointInfo, OperationType, OperationEffect};

/// Classifies endpoint operation type based on heuristics.
pub struct OperationTypeClassifier;

impl OperationTypeClassifier {
    /// Classify an endpoint's operation type.
    pub fn classify(endpoint: &EndpointInfo) -> OperationType {
        // Check operation_id keywords first
        if let Some(op_type) = Self::classify_by_operation_id(&endpoint.operation_id) {
            return op_type;
        }
        
        // Fall back to HTTP method and path analysis
        Self::classify_by_method_and_path(endpoint)
    }
    
    fn classify_by_operation_id(operation_id: &str) -> Option<OperationType> {
        let lower = operation_id.to_lowercase();
        
        // Create keywords
        if lower.contains("create") || lower.contains("insert") 
            || lower.contains("add") || lower.contains("register")
            || lower.contains("new") || lower.contains("allocate") {
            return Some(OperationType::Create);
        }
        
        // Read keywords
        if lower.contains("get") || lower.contains("list") 
            || lower.contains("fetch") || lower.contains("retrieve")
            || lower.contains("search") || lower.contains("query")
            || lower.contains("describe") || lower.contains("export")
            || lower.contains("test") || lower.contains("validate")
            || lower.contains("inspect") || lower.contains("analyze") {
            return Some(OperationType::Read);
        }
        
        // Update keywords
        if lower.contains("update") || lower.contains("patch")
            || lower.contains("modify") || lower.contains("replace")
            || lower.contains("set") || lower.contains("configure") {
            return Some(OperationType::Update);
        }
        
        // Delete keywords
        if lower.contains("delete") || lower.contains("remove")
            || lower.contains("destroy") || lower.contains("unpublish")
            || lower.contains("deprovision") {
            return Some(OperationType::Delete);
        }
        
        // Action keywords - need further classification
        if lower.contains("cancel") || lower.contains("start")
            || lower.contains("stop") || lower.contains("restart")
            || lower.contains("activate") || lower.contains("deactivate")
            || lower.contains("enable") || lower.contains("disable")
            || lower.contains("trigger") || lower.contains("invoke")
            || lower.contains("execute") || lower.contains("run") {
            // Classify action as mutating or read-only
            let effect = Self::classify_action_effect(&lower);
            return Some(OperationType::Action(effect));
        }
        
        None
    }
    
    fn classify_action_effect(action_lower: &str) -> OperationEffect {
        // Mutating actions
        if action_lower.contains("cancel") || action_lower.contains("start")
            || action_lower.contains("stop") || action_lower.contains("restart")
            || action_lower.contains("activate") || action_lower.contains("deactivate")
            || action_lower.contains("enable") || action_lower.contains("disable") {
            return OperationEffect::Mutating;
        }
        
        // Read-only actions
        if action_lower.contains("test") || action_lower.contains("validate")
            || action_lower.contains("export") || action_lower.contains("analyze")
            || action_lower.contains("inspect") {
            return OperationEffect::ReadOnly;
        }
        
        // Default to mutating for unknown actions (conservative)
        OperationEffect::Mutating
    }
    
    fn classify_by_method_and_path(endpoint: &EndpointInfo) -> OperationType {
        match endpoint.method.as_str() {
            "GET" => OperationType::Read,
            "DELETE" => OperationType::Delete,
            "PUT" | "PATCH" => OperationType::Update,
            "POST" => {
                // Check if path has resource ID (update) or is collection (create)
                if endpoint.path.contains('{') {
                    OperationType::Update
                } else {
                    OperationType::Create
                }
            }
            _ => OperationType::Action(OperationEffect::Mutating),
        }
    }
}
```

### 3. Update EndpointInfo

Add `operation_type` field and update constructor in extractor.

### 4. Update api_catalog.rs

Add helper methods for filtering by operation type.

### 5. Update Provider Wrapper Generator

**File:** `bin/platform/src/gen_resources/provider_wrappers.rs`

```rust
// Only wrap mutating operations with StoreStateIdentifierTask
for ep in &api.endpoints {
    let op_type = OperationTypeClassifier::classify(ep);
    
    if op_type.requires_state_tracking() {
        // Generate method with StoreStateIdentifierTask wrapping
        self.generate_wrapped_method(&mut out, ep, provider)?;
    } else {
        // Generate simple execute method
        self.generate_simple_method(&mut out, ep, provider)?;
    }
}
```

## Example Usage

### Provider Wrapper with Classified Operations

```rust
// Create operation - wrapped with state tracking
pub fn instances_create(
    &self,
    args: &InstancesCreateArgs,
) -> Result<impl StreamIterator<...>, ProviderError> {
    let builder = instances_create_builder(&self.http_client, &args.project, &args.zone, &args.instance)?;
    let task = instances_create_task(builder)?;
    let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);
    execute(store_task, None).map_err(...)
}

// Read operation - simple execute, no state tracking
pub fn instances_get(
    &self,
    args: &InstancesGetArgs,
) -> Result<impl StreamIterator<...>, ProviderError> {
    let builder = instances_get_builder(&self.http_client, &args.project, &args.zone, &args.instance)?;
    let task = instances_get_task(builder)?;
    // Direct execute, no state wrapping
    execute(task, None).map_err(...)
}

// Mutating action - wrapped with state tracking
pub fn instances_start(
    &self,
    args: &InstancesStartArgs,
) -> Result<impl StreamIterator<...>, ProviderError> {
    let builder = instances_start_builder(&self.http_client, &args.project, &args.zone, &args.instance)?;
    let task = instances_start_task(builder)?;
    let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);
    execute(store_task, None).map_err(...)
}

// Read-only action - simple execute
pub fn instances_test_iam_permissions(
    &self,
    args: &InstancesTestIamPermissionsArgs,
) -> Result<impl StreamIterator<...>, ProviderError> {
    let builder = instances_test_iam_permissions_builder(&self.http_client, &args.resource, &args.permissions)?;
    let task = instances_test_iam_permissions_task(builder)?;
    execute(task, None).map_err(...)
}
```

## Tasks

1. **Add OperationType and OperationEffect enums**
   - [x] Add to `backends/foundation_openapi/src/endpoint.rs`
   - [x] Add `requires_state_tracking()` method
   - [x] Add `operation_type` field to `EndpointInfo`

2. **Create classifier module**
   - [x] Create `backends/foundation_openapi/src/classifier.rs`
   - [x] Implement `OperationTypeClassifier::classify()`
   - [x] Implement keyword-based classification
   - [x] Implement method/path fallback classification
   - [x] Export from `lib.rs`

3. **Update endpoint extractor**
   - [x] Update `extractor.rs` to classify endpoints during extraction
   - [x] Set `operation_type` field for all endpoints

4. **Update api_catalog**
   - [x] Add `create_endpoints()`, `read_endpoints()`, `update_endpoints()`, `delete_endpoints()` helpers
   - [x] Update `mutating_endpoints()` to use operation type

5. **Update provider wrapper generator**
   - [x] Filter endpoints by operation type
   - [x] Generate wrapped methods for Create/Update/Delete/Mutating Action
   - [x] Generate simple methods for Read/ReadOnly Action
   - [x] Regenerate all provider wrappers

6. **Verification**
   - [x] `cargo check -p foundation_openapi` passes with zero warnings
   - [x] `cargo check -p foundation_deployment --features gcp,cloudflare` passes
   - [x] Generated wrappers compile without errors
   - [x] Read operations don't wrap with StoreStateIdentifierTask
   - [x] Mutating operations correctly wrap with StoreStateIdentifierTask

## Testing Strategy

1. **Unit tests for classifier** - Test each keyword classification
2. **Integration tests** - Verify generated code for sample endpoints
3. **Manual verification** - Check generated provider wrappers for correctness

## Related Features

- Feature 28: Provider Wrapper Pattern (dependency)
- Feature 27: Generate Task Methods
- Feature 02: State Stores
