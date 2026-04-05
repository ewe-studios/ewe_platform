//! Deployment planner state machine.
//!
//! WHY: Drives provider through deployment lifecycle with proper state transitions.
//!
//! WHAT: `DeploymentPlanner` implements `StateMachine` trait with `DeployState` states.
//!
//! HOW: Uses `StateTransition::Continue` for intermediate states, `Yield` for progress,
//! and `Complete(Ok/Err)` for terminal states. Errors propagate via `Complete(Err(e))`.

use crate::config::DeploymentTarget;
use crate::core::traits::DeploymentProvider;
use crate::core::types::BuildOutput;
use crate::error::DeploymentError;
use chrono::Utc;
use foundation_core::valtron::{NoAction, StateMachine, StateTransition};
use foundation_db::state::{collect_first, StateStatus, StateStore};
use std::time::Duration;

/// Successful deployment outcome, returned as the final Yield/Complete value.
pub type DeployOutcome = Result<crate::core::types::DeploymentResult, DeploymentError>;

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
        result: crate::core::types::DeploymentResult,
        retries_remaining: u32,
    },

    /// Terminal failure state.
    Failed {
        error: String,
    },

    /// Config unchanged, skip deployment.
    Skipped {
        reason: String,
    },

    /// Rollback in progress.
    RollingBack {
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

impl<P: DeploymentProvider> DeploymentPlanner<P> {
    /// Create a new deployment planner.
    pub fn new(
        provider: P,
        config: P::Config,
        state_store: Box<dyn StateStore>,
    ) -> Self {
        Self {
            provider,
            config,
            state_store,
            environment: None,
            dry_run: false,
            verify_retries: super::DEFAULT_VERIFY_RETRIES,
            verify_delay: super::DEFAULT_VERIFY_DELAY,
        }
    }

    /// Set the environment name (e.g., "staging", "production").
    #[must_use]
    pub fn environment(mut self, env: &str) -> Self {
        self.environment = Some(env.to_string());
        self
    }

    /// Set environment from optional string.
    #[must_use]
    pub fn environment_opt(mut self, env: Option<&str>) -> Self {
        self.environment = env.map(str::to_string);
        self
    }

    /// Set dry-run mode (simulate deployment without applying).
    #[must_use]
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Configure verification retries and delay.
    #[must_use]
    pub fn verify_retries(mut self, retries: u32, delay: Duration) -> Self {
        self.verify_retries = retries;
        self.verify_delay = delay;
        self
    }

    /// Generate resource ID from config.
    fn resource_id(&self) -> String {
        // Use provider name + config identifier as resource key
        format!("{}:{}", self.provider.name(), self.config_hash())
    }

    /// Hash the configuration for change detection.
    fn config_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let json = serde_json::to_string(&self.config).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Update state in the state store.
    fn update_state(&self, status: StateStatus) -> Result<(), DeploymentError> {
        use foundation_db::state::drive_to_completion;

        let resource_id = self.resource_id();
        let now = Utc::now();
        let state = foundation_db::state::ResourceState {
            id: resource_id.clone(),
            kind: format!("{}::deployment", self.provider.name()),
            provider: self.provider.name().to_string(),
            status,
            environment: self.environment.clone(),
            config_hash: self.config_hash(),
            output: serde_json::Value::Null,
            config_snapshot: serde_json::to_value(&self.config).unwrap_or_default(),
            created_at: now,
            updated_at: now,
        };

        let stream = self.state_store.set(&resource_id, &state)?;
        drive_to_completion(stream)?;
        Ok(())
    }

    /// Persist deployment result to state store.
    fn persist_result(
        &self,
        result: &crate::core::types::DeploymentResult,
    ) -> Result<(), DeploymentError> {
        use foundation_db::state::drive_to_completion;

        let resource_id = self.resource_id();
        let now = Utc::now();
        let state = foundation_db::state::ResourceState {
            id: resource_id.clone(),
            kind: format!("{}::deployment", self.provider.name()),
            provider: self.provider.name().to_string(),
            status: StateStatus::Created,
            environment: self.environment.clone(),
            config_hash: self.config_hash(),
            output: serde_json::to_value(result).map_err(|e| DeploymentError::Generic(format!(
                "Failed to serialize deployment result: {e}"
            )))?,
            config_snapshot: serde_json::to_value(&self.config).unwrap_or_default(),
            created_at: now,
            updated_at: now,
        };

        let stream = self.state_store.set(&resource_id, &state)?;
        drive_to_completion(stream)?;
        Ok(())
    }

    /// Get existing state from state store.
    fn get_existing_state(
        &self,
    ) -> Result<Option<foundation_db::state::ResourceState>, DeploymentError> {
        let resource_id = self.resource_id();
        let stream = self.state_store.get(&resource_id)?;
        // collect_first returns Option<Option<ResourceState>> - flatten to Option<ResourceState>
        Ok(collect_first(stream)?.flatten())
    }
}

impl<P: DeploymentProvider> StateMachine for DeploymentPlanner<P> {
    type State = DeployState;
    type Output = DeployOutcome;
    type Error = DeploymentError;
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
                    target: DeploymentTarget::from_provider_name(self.provider.name())
                        .unwrap_or(DeploymentTarget::Cloudflare),
                })
            }

            DeployState::Validating { target: _ } => {
                match self.provider.validate(&self.config) {
                    Ok(()) => {
                        let hash = self.config_hash();
                        StateTransition::Continue(DeployState::CheckingState {
                            config_hash: hash,
                        })
                    }
                    // Propagate error via Complete(Err(...)), NOT StateTransition::Error
                    Err(e) => StateTransition::Complete(Err(e)),
                }
            }

            DeployState::CheckingState { config_hash } => {
                // StateStore::get returns StateStoreStream — consume at boundary
                let existing = match self.get_existing_state() {
                    Ok(state) => state,
                    Err(e) => return StateTransition::Complete(Err(e)),
                };
                match existing {
                    Some(state) if !state.needs_deploy(&config_hash) => {
                        // Config unchanged, skip deployment
                        StateTransition::Complete(Ok(
                            crate::core::types::DeploymentResult::skipped(
                                &format!("config unchanged: {config_hash}"),
                            )
                        ))
                    }
                    _ => {
                        // Needs deployment
                        if let Err(e) = self.update_state(StateStatus::Creating) {
                            return StateTransition::Complete(Err(e));
                        }
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
                        if let Err(update_err) = self.update_state(StateStatus::Failed {
                            error: e.to_string(),
                        }) {
                            tracing::warn!("Failed to update state to Failed: {update_err}");
                        }
                        StateTransition::Complete(Err(e))
                    }
                }
            }

            DeployState::Packaging { build_output: _ } => {
                // Packaging is provider-specific but abstracted via build output
                StateTransition::Continue(DeployState::Deploying {
                    dry_run: self.dry_run,
                })
            }

            DeployState::Deploying { dry_run } => {
                if dry_run {
                    return StateTransition::Complete(Ok(
                        crate::core::types::DeploymentResult::dry_run(
                            self.provider.name(),
                            &self.resource_id(),
                        )
                    ));
                }
                if let Err(e) = self.update_state(StateStatus::Updating) {
                    return StateTransition::Complete(Err(e));
                }
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
                        // Attempt rollback via provider
                        // Note: For now we skip rollback since it requires provider-specific Resources type
                        // TODO: Implement proper rollback with type mapping
                        if let Err(update_err) = self.update_state(StateStatus::Failed {
                            error: e.to_string(),
                        }) {
                            tracing::warn!("Failed to update state to Failed: {update_err}");
                        }
                        StateTransition::Complete(Err(e))
                    }
                }
            }

            DeployState::Verifying { result, retries_remaining } => {
                // Health-check: verify deployment is responsive
                match self.provider.verify(&result) {
                    Ok(true) => {
                        // Healthy — persist and complete
                        if let Err(e) = self.update_state(StateStatus::Created) {
                            tracing::warn!("Failed to update state to Created: {e}");
                        }
                        if let Err(e) = self.persist_result(&result) {
                            tracing::warn!("Failed to persist result: {e}");
                        }
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
                        // Note: For now we skip rollback since it requires provider-specific Resources type
                        // TODO: Implement proper rollback with type mapping
                        if let Err(update_err) = self.update_state(StateStatus::Failed {
                            error: "health check failed after retries".to_string(),
                        }) {
                            tracing::warn!("Failed to update state to Failed: {update_err}");
                        }
                        StateTransition::Complete(Err(DeploymentError::DeployRejected {
                            reason: "health check failed after retries".to_string(),
                        }))
                    }
                    Err(e) => {
                        // Verify itself failed — rollback then propagate error
                        // Note: For now we skip rollback since it requires provider-specific Resources type
                        // TODO: Implement proper rollback with type mapping
                        if let Err(update_err) = self.update_state(StateStatus::Failed {
                            error: e.to_string(),
                        }) {
                            tracing::warn!("Failed to update state to Failed: {update_err}");
                        }
                        StateTransition::Complete(Err(DeploymentError::DeployRejected {
                            reason: e.to_string(),
                        }))
                    }
                }
            }

            DeployState::RollingBack { error } => {
                // Attempt provider rollback if supported
                let _ = self.provider.destroy(
                    &self.config,
                    self.environment.as_deref(),
                );
                if let Err(update_err) = self.update_state(StateStatus::Failed {
                    error: error.clone()
                }) {
                    tracing::warn!("Failed to update state to Failed: {update_err}");
                }
                StateTransition::Continue(DeployState::Failed { error })
            }

            DeployState::Failed { error } => {
                // Terminal — propagate error via Complete(Err(...))
                StateTransition::Complete(Err(
                    DeploymentError::DeployRejected { reason: error }
                ))
            }

            DeployState::Skipped { reason } => {
                StateTransition::Complete(Ok(
                    crate::core::types::DeploymentResult::skipped(&reason)
                ))
            }
        }
    }
}
