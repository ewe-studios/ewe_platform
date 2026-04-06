---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/03-deployment-engine"
this_file: "specifications/11-foundation-deployment/features/03-deployment-engine/feature.md"

status: complete
priority: high
created: 2026-03-26
completed: 2026-04-06

depends_on: ["01-foundation-deployment-core", "02-state-stores"]

tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---


# Deployment Engine

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Build the deployment orchestration engine using valtron's `StateMachine` trait. The engine drives the deployment lifecycle (detect -> validate -> build -> deploy -> verify) for any provider, manages state transitions, handles rollback on failure, and reports progress events.

## Dependencies

Depends on:
- `01-foundation-deployment-core` - `DeploymentProvider` trait, types
- `02-state-stores` - `StateStore` for persisting deployment state

Required by:
- `04-cloudflare-provider`, `05-gcp-cloud-run-provider`, `06-aws-lambda-provider` - Providers plug into the engine
- `08-mise-integration` - CLI commands drive the engine

## Valtron Integration Notes

> **Critical**: Valtron's `StateMachineTask` silently swallows `StateTransition::Error` —
> it maps to `None` (task stops) with only a `tracing::warn!`. To propagate errors to
> callers, the `Output` type must be `Result<T, E>` and errors must be emitted via
> `StateTransition::Complete(Err(e))`, not `StateTransition::Error`.
>
> The `Yield(output, next_state)` variant emits a value AND continues — use it for
> progress events instead of a side-channel callback. This makes progress observable
> through `StreamIterator`, the standard Valtron consumption pattern.
>
> The `Delay(duration, next_state)` variant provides native backoff support in the
> executor — use it for health-check retries in the Verifying state.

## Requirements

### Deployment State Machine

```rust
// engine/planner.rs

use foundation_core::valtron::{StateMachine, StateTransition, NoAction};
use std::time::Duration;

/// Successful deployment outcome, returned as the final Yield/Complete value.
pub type DeployOutcome = Result<DeploymentResult, DeploymentError>;

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
        retries_remaining: u32,
    },

    /// Everything succeeded.
    Complete {
        result: DeploymentResult,
    },

    /// Config unchanged, skip deployment.
    Skipped {
        reason: String,
    },

    /// Terminal failure state.
    Failed {
        error: String,
    },
}

/// The deployment planner drives a provider through its lifecycle.
///
/// Progress is reported via `StateTransition::Yield(DeployProgress, next_state)`
/// rather than a callback, making progress observable through the standard
/// `StreamIterator` API after `execute()`.
pub struct DeploymentPlanner<P: DeploymentProvider> {
    provider: P,
    config: P::Config,
    state_store: Box<dyn StateStore>,
    environment: Option<String>,
    dry_run: bool,
    verify_retries: u32,
    verify_delay: Duration,
}

/// Default number of health-check retries during verification.
const DEFAULT_VERIFY_RETRIES: u32 = 5;
/// Default delay between verification retries (executor handles the sleep).
const DEFAULT_VERIFY_DELAY: Duration = Duration::from_secs(5);

impl<P: DeploymentProvider> DeploymentPlanner<P> {
    pub fn new(
        provider: P,
        config: P::Config,
        state_store: Box<dyn StateStore>,
    ) -> Self;

    pub fn environment(self, env: &str) -> Self;
    pub fn dry_run(self, dry_run: bool) -> Self;
    pub fn verify_retries(self, retries: u32, delay: Duration) -> Self;
}

/// Output type is `DeployOutcome` (= `Result<DeploymentResult, DeploymentError>`)
/// so that errors propagate through `Complete(Err(e))` instead of being swallowed.
///
/// Progress events are emitted via `Yield(DeployProgress, next_state)` — the caller
/// receives them as `Stream::Next(DeployProgress)` from the `StreamIterator`.
impl<P: DeploymentProvider> StateMachine for DeploymentPlanner<P> {
    type State = DeployState;
    type Output = DeployOutcome;
    type Error = DeploymentError;   // unused by StateMachineTask, but required by trait
    type Action = NoAction;

    fn initial_state(&self) -> DeployState {
        DeployState::Detecting
    }

    fn transition(
        &mut self,
        state: DeployState,
    ) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        match state {
            DeployState::Detecting => {
                // Yield progress, then continue to Validating
                StateTransition::Continue(DeployState::Validating {
                    target: DeploymentTarget::from_provider(self.provider.name()),
                })
            }

            DeployState::Validating { target } => {
                match self.provider.validate(&self.config) {
                    Ok(_) => {
                        let hash = self.hash_config(&self.config);
                        StateTransition::Continue(DeployState::CheckingState {
                            config_hash: hash,
                        })
                    }
                    // Propagate error via Complete(Err(...)), NOT StateTransition::Error
                    Err(e) => StateTransition::Complete(Err(e)),
                }
            }

            DeployState::CheckingState { config_hash } => {
                let resource_id = self.resource_id();
                // StateStore::get returns StateStoreStream — consume at boundary
                let existing = match self.state_store.get(&resource_id) {
                    Ok(stream) => collect_first(stream).unwrap_or(None),
                    Err(e) => return StateTransition::Complete(Err(e)),
                };
                match existing {
                    Some(Some(state)) if !state.needs_deploy(&config_hash) => {
                        StateTransition::Complete(Ok(
                            state.to_deployment_result()
                        ))
                    }
                    _ => {
                        // Needs deployment
                        self.update_state(StateStatus::Creating);
                        StateTransition::Continue(DeployState::Building)
                    }
                }
            }

            DeployState::Building => {
                match self.provider.build(&self.config, self.environment.as_deref()) {
                    Ok(output) => StateTransition::Continue(DeployState::Packaging {
                        build_output: output,
                    }),
                    Err(e) => {
                        self.update_state(StateStatus::Failed {
                            error: e.to_string(),
                        });
                        StateTransition::Complete(Err(e))
                    }
                }
            }

            DeployState::Packaging { build_output } => {
                // Packaging is provider-specific but abstracted via build output
                StateTransition::Continue(DeployState::Deploying {
                    dry_run: self.dry_run,
                })
            }

            DeployState::Deploying { dry_run } => {
                if dry_run {
                    return StateTransition::Complete(Ok(DeploymentResult::dry_run(
                        self.provider.name(),
                        &self.resource_id(),
                    )));
                }
                self.update_state(StateStatus::Updating);
                match self.provider.deploy(
                    &self.config,
                    self.environment.as_deref(),
                    false,
                ) {
                    Ok(result) => StateTransition::Continue(DeployState::Verifying {
                        result,
                        retries_remaining: self.verify_retries,
                    }),
                    Err(e) => {
                        // Attempt rollback via provider — errors from rollback are logged but
                        // the original deploy error is what gets propagated to the caller.
                        let previous = self.state_store.get(&self.resource_id())
                            .ok()
                            .and_then(|stream| collect_first(stream).ok())
                            .flatten();
                        if let Err(rollback_err) = self.provider.rollback(
                            &self.config,
                            self.environment.as_deref(),
                            previous.as_ref(),
                        ) {
                            tracing::warn!("Rollback failed: {rollback_err}");
                        }
                        self.update_state(StateStatus::Failed {
                            error: e.to_string(),
                        });
                        StateTransition::Complete(Err(e))
                    }
                }
            }

            DeployState::Verifying { result, retries_remaining } => {
                // Health-check: verify deployment is responsive
                match self.provider.verify(&result) {
                    Ok(true) => {
                        // Healthy — persist and complete
                        self.update_state(StateStatus::Created);
                        self.persist_result(&result);
                        StateTransition::Complete(Ok(result))
                    }
                    Ok(false) if retries_remaining > 0 => {
                        // Not healthy yet — use Delay for native executor backoff
                        StateTransition::Delay(
                            self.verify_delay,
                            DeployState::Verifying {
                                result,
                                retries_remaining: retries_remaining - 1,
                            },
                        )
                    }
                    Ok(false) => {
                        // Retries exhausted — rollback then propagate error
                        let previous = self.state_store.get(&self.resource_id())
                            .ok()
                            .and_then(|stream| collect_first(stream).ok())
                            .flatten();
                        if let Err(rollback_err) = self.provider.rollback(
                            &self.config,
                            self.environment.as_deref(),
                            previous.as_ref(),
                        ) {
                            tracing::warn!("Rollback failed: {rollback_err}");
                        }
                        self.update_state(StateStatus::Failed {
                            error: "health check failed after retries".to_string(),
                        });
                        StateTransition::Complete(Err(DeploymentError::DeployRejected {
                            reason: "health check failed after retries".to_string(),
                        }))
                    }
                    Err(e) => {
                        // Verify itself failed — rollback then propagate error
                        let previous = self.state_store.get(&self.resource_id())
                            .ok()
                            .and_then(|stream| collect_first(stream).ok())
                            .flatten();
                        if let Err(rollback_err) = self.provider.rollback(
                            &self.config,
                            self.environment.as_deref(),
                            previous.as_ref(),
                        ) {
                            tracing::warn!("Rollback failed: {rollback_err}");
                        }
                        self.update_state(StateStatus::Failed {
                            error: e.to_string(),
                        });
                        StateTransition::Complete(Err(DeploymentError::DeployRejected {
                            reason: e.to_string(),
                        }))
                    }
                }
            }

            DeployState::RollingBack { error, previous_state } => {
                // Attempt provider rollback if supported
                let _ = self.provider.destroy(
                    &self.config,
                    self.environment.as_deref(),
                );
                self.update_state(StateStatus::Failed { error: error.clone() });
                StateTransition::Continue(DeployState::Failed { error })
            }

            DeployState::Failed { error } => {
                // Terminal — propagate error via Complete(Err(...))
                StateTransition::Complete(Err(
                    DeploymentError::DeployRejected { reason: error }
                ))
            }

            DeployState::Complete { result } => {
                StateTransition::Complete(Ok(result))
            }

            DeployState::Skipped { reason } => {
                StateTransition::Complete(Ok(DeploymentResult::skipped(&reason)))
            }
        }
    }
}
```

### Deployment Executor

```rust
// engine/executor.rs

use foundation_core::valtron::executors::state_machine::StateMachineTask;
use foundation_core::valtron::executors::unified::execute;
use foundation_core::synca::mpp::{Stream, StreamIterator};

/// High-level API that creates a planner and runs it to completion.
pub struct DeploymentExecutor;

impl DeploymentExecutor {
    /// Deploy a project. Auto-detects provider, loads config, runs state machine.
    ///
    /// **Caller requirement**: A valtron `PoolGuard` must be alive in the calling
    /// scope. Binary entry points initialize it; library code must not.
    /// ```rust
    /// // In main() or test setup:
    /// let _guard = foundation_core::valtron::executors::unified::initialize_pool(42, None);
    /// let result = DeploymentExecutor::deploy(...);
    /// ```
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

/// Run a valtron StateMachine to completion, collecting the final result.
///
/// The `Output` type is `DeployOutcome` (`Result<DeploymentResult, DeploymentError>`),
/// so errors are delivered as `Stream::Next(Err(e))` — never swallowed.
fn run_state_machine<M>(machine: M) -> Result<DeploymentResult, DeploymentError>
where
    M: StateMachine<Output = DeployOutcome, Error = DeploymentError, Action = NoAction>
        + Send + 'static,
    M::State: Send,
{
    let task = StateMachineTask::new(machine);
    let mut stream = execute(task, None)
        .map_err(|e| DeploymentError::ExecutorError { reason: e.to_string() })?;

    // Drive the stream to completion. The last Stream::Next value is the outcome.
    let mut last_outcome: Option<DeployOutcome> = None;
    loop {
        match stream.next_stream() {
            Some(Stream::Next(outcome)) => {
                last_outcome = Some(outcome);
            }
            Some(Stream::Pending(_)) | Some(Stream::Delayed(_)) | Some(Stream::Ignore) => {
                // Intermediate states — executor handles delays internally
                continue;
            }
            Some(Stream::Init) => continue,
            None => break,
        }
    }

    last_outcome.unwrap_or_else(|| Err(DeploymentError::ExecutorError {
        reason: "state machine produced no output".to_string(),
    }))
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

4. **Implement rollback support**
   - [ ] Each provider implements `rollback()` method
   - [ ] Engine calls `provider.rollback()` on deploy/verify failure
   - [ ] Rollback errors are logged but don't mask original error

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

- **State store streams** — `StateStore` methods return `StateStoreStream<T>` (lazy iterators). The engine consumes these at the sync boundary using `collect_first()` for single values and `drive_to_completion()` for writes. Import helpers from `state/helpers.rs`. The `update_state()` helper method should call `drive_to_completion(self.state_store.set(...)?)?` internally.
- Use `foundation_core::valtron::executors::state_machine::StateMachine` as the base trait
- `StateTransition::Continue` for intermediate states, `::Complete(Ok(result))` for terminal success, `::Complete(Err(e))` for terminal failure
- **Never use `StateTransition::Error`** — `StateMachineTask` maps it to `None` (silently stops the task). Always propagate errors through the `Output` type as `Complete(Err(e))`
- Use `StateTransition::Delay(duration, next_state)` for retry/backoff (e.g., health-check retries in Verifying state) — the executor handles the sleep natively
- Use `StateTransition::Yield(progress, next_state)` if progress events need to be observable through `StreamIterator` (alternative to callbacks)
- **PoolGuard requirement**: Binary entry points must call `valtron::executors::unified::initialize_pool()` before using the deployment executor. Library code (this crate) must NOT initialize the pool
- Config hashing uses `sha2::Sha256` on canonical JSON serialization
- The `execute()` function auto-selects single-threaded (WASM) or multi-threaded (native) executor

## Success Criteria

- [ ] All 6 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings, zero suppression
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere in the code
- [ ] State machine drives correctly through happy path
- [ ] Changed config triggers deployment, unchanged config skips
- [ ] Failed deployment triggers rollback
- [ ] Dry-run mode stops before actual deployment
- [ ] Errors propagate as `Complete(Err(e))` — never swallowed by `StateMachineTask`
- [ ] Health-check retries use `StateTransition::Delay` for native executor backoff
- [ ] `run_state_machine()` drives `StateMachineTask` via `valtron::execute()` to completion

## Verification

```bash
cd backends/foundation_deployment
cargo test engine -- --nocapture
cargo test planner -- --nocapture
cargo test rollback -- --nocapture
```

---

_Created: 2026-03-26_
_Updated: 2026-04-06 - Status changed to complete, all 6/6 tasks implemented_

## Verification Notes (2026-04-06)

**Implementation Status: COMPLETE**

All 6 tasks completed:
- [x] `DeployState` enum with all states (Detecting, Validating, CheckingState, Building, Packaging, Deploying, Verifying, RollingBack, Failed, Skipped)
- [x] `DeploymentPlanner` implementing `StateMachine` trait with all state transitions
- [x] `DeploymentExecutor` with `deploy()`, `destroy()`, `status()` methods
- [x] Rollback support in state transitions
- [x] State persistence integration via `StateStore`
- [x] Tests passing as part of full test suite

**Verification Results:**
- `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — **zero warnings**
- `cargo test -p foundation_deployment` — **33 tests passed** (includes engine/planner tests)
- No `#[allow(...)]` or `#[expect(...)]` suppressions in code
- `transition()` function (155 lines) split into 10 helper methods:
  - `transition_detecting()`, `transition_validating()`, `transition_checking_state()`
  - `transition_building()`, `transition_packaging()`, `transition_deploying()`
  - `transition_verifying()`, `transition_rolling_back()`, `transition_failed()`, `transition_skipped()`
- Error propagation via `StateTransition::Complete(Err(e))` (not `StateTransition::Error`)
- `StateTransition::Delay` used for health-check retry backoff
- Config hash parameter optimized from `String` to `&str`
- Unused self parameters converted to associated functions
