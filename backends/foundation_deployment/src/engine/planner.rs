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
        Ok(collect_first(stream)?.flatten())
    }

    // State transition helpers extracted to reduce transition() line count

    fn transition_detecting(&self) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        StateTransition::Continue(DeployState::Validating {
            target: DeploymentTarget::from_provider_name(self.provider.name())
                .unwrap_or(DeploymentTarget::Cloudflare),
        })
    }

    fn transition_validating(
        &mut self,
    ) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        match self.provider.validate(&self.config) {
            Ok(()) => {
                let hash = self.config_hash();
                StateTransition::Continue(DeployState::CheckingState { config_hash: hash })
            }
            Err(e) => StateTransition::Complete(Err(e)),
        }
    }

    fn transition_checking_state(
        &mut self,
        config_hash: &str,
    ) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        let existing = match self.get_existing_state() {
            Ok(state) => state,
            Err(e) => return StateTransition::Complete(Err(e)),
        };
        match existing {
            Some(state) if !state.needs_deploy(config_hash) => {
                StateTransition::Complete(Ok(
                    crate::core::types::DeploymentResult::skipped(
                        &format!("config unchanged: {config_hash}"),
                    )
                ))
            }
            _ => {
                if let Err(e) = self.update_state(StateStatus::Creating) {
                    return StateTransition::Complete(Err(e));
                }
                StateTransition::Continue(DeployState::Building)
            }
        }
    }

    fn transition_building(
        &mut self,
    ) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        match self.provider.build(&self.config, self.environment.as_deref()) {
            Ok(output) => StateTransition::Continue(DeployState::Packaging { build_output: output }),
            Err(e) => {
                let _ = self.update_state(StateStatus::Failed { error: e.to_string() });
                StateTransition::Complete(Err(e))
            }
        }
    }

    fn transition_packaging(
        &self,
    ) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        StateTransition::Continue(DeployState::Deploying { dry_run: self.dry_run })
    }

    fn transition_deploying(
        &mut self,
        dry_run: bool,
    ) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
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
        match self.provider.deploy(&self.config, self.environment.as_deref(), false) {
            Ok(result) => StateTransition::Continue(DeployState::Verifying {
                result,
                retries_remaining: self.verify_retries,
            }),
            Err(e) => {
                let _ = self.update_state(StateStatus::Failed { error: e.to_string() });
                StateTransition::Complete(Err(e))
            }
        }
    }

    fn transition_verifying(
        &mut self,
        result: crate::core::types::DeploymentResult,
        retries_remaining: u32,
    ) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        match self.provider.verify(&result) {
            Ok(true) => {
                let _ = self.update_state(StateStatus::Created);
                let _ = self.persist_result(&result);
                StateTransition::Complete(Ok(result))
            }
            Ok(false) if retries_remaining > 0 => {
                StateTransition::Delay(
                    self.verify_delay,
                    DeployState::Verifying {
                        result,
                        retries_remaining: retries_remaining - 1,
                    },
                )
            }
            Ok(false) => {
                let _ = self.update_state(StateStatus::Failed {
                    error: "health check failed after retries".to_string(),
                });
                StateTransition::Complete(Err(DeploymentError::DeployRejected {
                    reason: "health check failed after retries".to_string(),
                }))
            }
            Err(e) => {
                let _ = self.update_state(StateStatus::Failed { error: e.to_string() });
                StateTransition::Complete(Err(DeploymentError::DeployRejected {
                    reason: e.to_string(),
                }))
            }
        }
    }

    fn transition_rolling_back(
        &mut self,
        error: String,
    ) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        let _ = self.provider.destroy(&self.config, self.environment.as_deref());
        let _ = self.update_state(StateStatus::Failed { error: error.clone() });
        StateTransition::Continue(DeployState::Failed { error })
    }

    fn transition_failed(error: String) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        StateTransition::Complete(Err(DeploymentError::DeployRejected { reason: error }))
    }

    fn transition_skipped(reason: &str) -> StateTransition<DeployState, DeployOutcome, DeploymentError, NoAction> {
        StateTransition::Complete(Ok(crate::core::types::DeploymentResult::skipped(reason)))
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
            DeployState::Detecting => self.transition_detecting(),
            DeployState::Validating { .. } => self.transition_validating(),
            DeployState::CheckingState { ref config_hash } => self.transition_checking_state(config_hash),
            DeployState::Building => self.transition_building(),
            DeployState::Packaging { .. } => self.transition_packaging(),
            DeployState::Deploying { dry_run } => self.transition_deploying(dry_run),
            DeployState::Verifying { result, retries_remaining } => {
                self.transition_verifying(result, retries_remaining)
            }
            DeployState::RollingBack { error } => self.transition_rolling_back(error),
            DeployState::Failed { error } => Self::transition_failed(error),
            DeployState::Skipped { ref reason } => Self::transition_skipped(reason),
        }
    }
}
