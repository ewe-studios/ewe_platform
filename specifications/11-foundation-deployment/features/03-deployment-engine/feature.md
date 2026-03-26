---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/03-deployment-engine"
this_file: "specifications/11-foundation-deployment/features/03-deployment-engine/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["01-foundation-deployment-core", "02-state-stores"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Deployment Engine

## Overview

Build the deployment orchestration engine using valtron's `StateMachine` trait. The engine drives the deployment lifecycle (detect -> validate -> build -> deploy -> verify) for any provider, manages state transitions, handles rollback on failure, and reports progress events.

## Dependencies

Depends on:
- `01-foundation-deployment-core` - `DeploymentProvider` trait, types
- `02-state-stores` - `StateStore` for persisting deployment state

Required by:
- `04-cloudflare-provider`, `05-gcp-cloud-run-provider`, `06-aws-lambda-provider` - Providers plug into the engine
- `08-mise-integration` - CLI commands drive the engine

## Requirements

### Deployment State Machine

```rust
// engine/planner.rs

use foundation_core::valtron::{StateMachine, StateTransition, NoAction};

/// States the deployment progresses through.
#[derive(Debug, Clone)]
pub enum DeployState {
    /// Initial state: detect provider from project directory.
    Detecting,

    /// Provider detected, validate config.
    Validating {
        target: DeploymentTarget,
    },

    /// Config valid, check state store for changes.
    CheckingState {
        config_hash: String,
    },

    /// State says config changed (or first deploy), run build.
    Building,

    /// Build complete, package artifacts.
    Packaging {
        build_output: BuildOutput,
    },

    /// Artifacts ready, deploy to provider.
    Deploying {
        dry_run: bool,
    },

    /// Deployed, verify the deployment is healthy.
    Verifying {
        result: DeploymentResult,
    },

    /// Everything succeeded.
    Complete {
        result: DeploymentResult,
    },

    /// Config unchanged, skip deployment.
    Skipped {
        reason: String,
    },

    /// Something failed, attempt rollback.
    RollingBack {
        error: String,
        previous_state: Option<ResourceState>,
    },

    /// Terminal failure state.
    Failed {
        error: String,
    },
}

/// The deployment planner drives a provider through its lifecycle.
pub struct DeploymentPlanner<P: DeploymentProvider> {
    provider: P,
    config: P::Config,
    state_store: Box<dyn StateStore>,
    environment: Option<String>,
    dry_run: bool,
    progress_callback: Option<Box<dyn FnMut(DeployProgress)>>,
}

impl<P: DeploymentProvider> DeploymentPlanner<P> {
    pub fn new(
        provider: P,
        config: P::Config,
        state_store: Box<dyn StateStore>,
    ) -> Self;

    pub fn environment(self, env: &str) -> Self;
    pub fn dry_run(self, dry_run: bool) -> Self;
    pub fn on_progress<F: FnMut(DeployProgress) + 'static>(self, callback: F) -> Self;
}

impl<P: DeploymentProvider> StateMachine for DeploymentPlanner<P> {
    type State = DeployState;
    type Output = DeploymentResult;
    type Error = DeploymentError;
    type Action = NoAction;

    fn initial_state(&self) -> DeployState {
        DeployState::Detecting
    }

    fn transition(
        &mut self,
        state: DeployState,
    ) -> StateTransition<DeployState, DeploymentResult, DeploymentError, NoAction> {
        match state {
            DeployState::Detecting => {
                self.emit_progress(DeployProgress::Detecting);
                StateTransition::Continue(DeployState::Validating {
                    target: DeploymentTarget::from_provider(self.provider.name()),
                })
            }

            DeployState::Validating { target } => {
                self.emit_progress(DeployProgress::Validating);
                match self.provider.validate(&self.config) {
                    Ok(_) => {
                        let hash = self.hash_config(&self.config);
                        StateTransition::Continue(DeployState::CheckingState {
                            config_hash: hash,
                        })
                    }
                    Err(e) => StateTransition::Error(e),
                }
            }

            DeployState::CheckingState { config_hash } => {
                let resource_id = self.resource_id();
                match self.state_store.get(&resource_id) {
                    Ok(Some(existing)) if !existing.needs_deploy(&config_hash) => {
                        StateTransition::Complete(DeploymentResult {
                            // Return cached result
                            ..existing.to_deployment_result()
                        })
                    }
                    _ => {
                        // Needs deployment
                        self.update_state(StateStatus::Creating);
                        StateTransition::Continue(DeployState::Building)
                    }
                }
            }

            DeployState::Building => {
                self.emit_progress(DeployProgress::Building {
                    step: "compiling".to_string(),
                });
                match self.provider.build(&self.config, self.environment.as_deref()) {
                    Ok(output) => StateTransition::Continue(DeployState::Packaging {
                        build_output: output,
                    }),
                    Err(e) => {
                        self.update_state(StateStatus::Failed {
                            error: e.to_string(),
                        });
                        StateTransition::Error(e)
                    }
                }
            }

            DeployState::Packaging { build_output } => {
                self.emit_progress(DeployProgress::Packaging);
                // Packaging is provider-specific but abstracted via build output
                StateTransition::Continue(DeployState::Deploying {
                    dry_run: self.dry_run,
                })
            }

            DeployState::Deploying { dry_run } => {
                if dry_run {
                    return StateTransition::Complete(DeploymentResult::dry_run(
                        self.provider.name(),
                        &self.resource_id(),
                    ));
                }
                self.emit_progress(DeployProgress::Deploying);
                self.update_state(StateStatus::Updating);
                match self.provider.deploy(
                    &self.config,
                    self.environment.as_deref(),
                    false,
                ) {
                    Ok(result) => StateTransition::Continue(DeployState::Verifying { result }),
                    Err(e) => {
                        let previous = self.state_store.get(&self.resource_id()).ok().flatten();
                        StateTransition::Continue(DeployState::RollingBack {
                            error: e.to_string(),
                            previous_state: previous,
                        })
                    }
                }
            }

            DeployState::Verifying { result } => {
                self.emit_progress(DeployProgress::Verifying);
                // Verification: check URL responds, check status endpoint, etc.
                // For now, trust the provider's deploy result.
                self.update_state(StateStatus::Created);
                self.persist_result(&result);
                self.emit_progress(DeployProgress::Complete(result.clone()));
                StateTransition::Complete(result)
            }

            DeployState::RollingBack { error, previous_state } => {
                self.emit_progress(DeployProgress::RollingBack);
                // Attempt provider rollback if supported
                let _ = self.provider.destroy(
                    &self.config,
                    self.environment.as_deref(),
                );
                self.update_state(StateStatus::Failed { error: error.clone() });
                StateTransition::Continue(DeployState::Failed { error })
            }

            DeployState::Failed { error } => {
                self.emit_progress(DeployProgress::Failed(error.clone()));
                StateTransition::Error(DeploymentError::DeployRejected { reason: error })
            }

            DeployState::Complete { result } => {
                StateTransition::Complete(result)
            }

            DeployState::Skipped { reason } => {
                // No-op, config unchanged
                StateTransition::Complete(DeploymentResult::skipped(&reason))
            }
        }
    }
}
```

### Deployment Executor

```rust
// engine/executor.rs

/// High-level API that creates a planner and runs it to completion.
pub struct DeploymentExecutor;

impl DeploymentExecutor {
    /// Deploy a project. Auto-detects provider, loads config, runs state machine.
    pub fn deploy(
        project_dir: &Path,
        env: Option<&str>,
        dry_run: bool,
        state_store: Box<dyn StateStore>,
    ) -> Result<DeploymentResult, DeploymentError> {
        let target = DeploymentTarget::detect(project_dir)
            .ok_or_else(|| DeploymentError::NoProviderDetected {
                project_dir: project_dir.display().to_string(),
            })?;

        match target {
            DeploymentTarget::Cloudflare => {
                let config = CloudflareProvider::detect(project_dir)
                    .ok_or_else(|| DeploymentError::ConfigInvalid {
                        file: "wrangler.toml".into(),
                        reason: "failed to parse".into(),
                    })?;
                let provider = CloudflareProvider::new();
                let planner = DeploymentPlanner::new(provider, config, state_store)
                    .environment_opt(env)
                    .dry_run(dry_run);
                run_state_machine(planner)
            }
            DeploymentTarget::GcpCloudRun => {
                // Same pattern for GCP
            }
            DeploymentTarget::AwsLambda => {
                // Same pattern for AWS
            }
        }
    }

    /// Destroy resources for a project.
    pub fn destroy(
        project_dir: &Path,
        env: Option<&str>,
        state_store: Box<dyn StateStore>,
    ) -> Result<(), DeploymentError>;

    /// Show deployment status.
    pub fn status(
        project_dir: &Path,
        env: Option<&str>,
        state_store: Box<dyn StateStore>,
    ) -> Result<serde_json::Value, DeploymentError>;
}

/// Run a valtron StateMachine to completion.
fn run_state_machine<M: StateMachine<Output = DeploymentResult, Error = DeploymentError>>(
    machine: M,
) -> Result<DeploymentResult, DeploymentError> {
    let task = StateMachineTask::new(machine);
    // Drive task via valtron executor
    // ...
}
```

### Rollback Handling

```rust
// engine/rollback.rs

/// Provider-specific rollback strategies.
pub enum RollbackStrategy {
    /// Cloudflare: redeploy previous version.
    RedeployPrevious { version_id: String },

    /// GCP Cloud Run: route traffic back to previous revision.
    TrafficShift { previous_revision: String },

    /// AWS Lambda: update alias to point to previous version.
    AliasSwitch { alias: String, previous_version: String },

    /// Generic: destroy the failed deployment.
    Destroy,

    /// No rollback available.
    None,
}

pub fn determine_rollback(
    provider: &str,
    previous_state: Option<&ResourceState>,
) -> RollbackStrategy {
    match (provider, previous_state) {
        ("cloudflare", Some(state)) => {
            if let Some(version_id) = state.output.get("version_id").and_then(|v| v.as_str()) {
                RollbackStrategy::RedeployPrevious { version_id: version_id.to_string() }
            } else {
                RollbackStrategy::Destroy
            }
        }
        ("gcp", Some(state)) => {
            if let Some(revision) = state.output.get("revision").and_then(|v| v.as_str()) {
                RollbackStrategy::TrafficShift { previous_revision: revision.to_string() }
            } else {
                RollbackStrategy::Destroy
            }
        }
        ("aws", Some(state)) => {
            if let Some(version) = state.output.get("version").and_then(|v| v.as_str()) {
                RollbackStrategy::AliasSwitch {
                    alias: "live".to_string(),
                    previous_version: version.to_string(),
                }
            } else {
                RollbackStrategy::Destroy
            }
        }
        _ => RollbackStrategy::None,
    }
}
```

## Tasks

1. **Define deployment states**
   - [ ] Create `src/engine/mod.rs`
   - [ ] Define `DeployState` enum with all states
   - [ ] Document state transition diagram

2. **Implement DeploymentPlanner**
   - [ ] Create `src/engine/planner.rs`
   - [ ] Implement `StateMachine` trait from valtron
   - [ ] Implement all state transitions
   - [ ] Add progress callback support
   - [ ] Implement config hashing for change detection

3. **Implement DeploymentExecutor**
   - [ ] Create `src/engine/executor.rs`
   - [ ] Implement `deploy()` with auto-detection
   - [ ] Implement `destroy()` and `status()`
   - [ ] Wire up valtron executor for running the state machine

4. **Implement rollback**
   - [ ] Create `src/engine/rollback.rs`
   - [ ] Define `RollbackStrategy` enum
   - [ ] Implement `determine_rollback()` for each provider
   - [ ] Wire into `RollingBack` state transition

5. **State persistence integration**
   - [ ] Read existing state on deploy start
   - [ ] Update state at each transition (Creating, Updating, Created, Failed)
   - [ ] Persist `DeploymentResult` output on success
   - [ ] Call `sync_remote()` after state changes

6. **Write tests**
   - [ ] Unit test state transitions with mock provider
   - [ ] Test change detection (skip unchanged, deploy changed)
   - [ ] Test rollback path
   - [ ] Test dry-run mode
   - [ ] Integration test with real valtron executor

## Implementation Notes

- Use `foundation_core::valtron::executors::state_machine::StateMachine` as the base trait
- `StateTransition::Continue` for intermediate states, `::Complete` for terminal success, `::Error` for terminal failure
- Progress callbacks are `FnMut` to allow mutable state in the reporter
- Config hashing uses `sha2::Sha256` on canonical JSON serialization

## Success Criteria

- [ ] All 6 tasks completed
- [ ] State machine drives correctly through happy path
- [ ] Changed config triggers deployment, unchanged config skips
- [ ] Failed deployment triggers rollback
- [ ] Dry-run mode stops before actual deployment
- [ ] Progress events fire at each state transition

## Verification

```bash
cd backends/foundation_deployment
cargo test engine -- --nocapture
cargo test planner -- --nocapture
cargo test rollback -- --nocapture
```

---

_Created: 2026-03-26_
