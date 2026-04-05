//! Deployment engine using Valtron `StateMachine` pattern.
//!
//! WHY: Orchestrate deployment lifecycle (detect -> validate -> build -> deploy -> verify)
//! for any provider, with state persistence and rollback support.
//!
//! WHAT: `DeploymentPlanner` implements `StateMachine` trait; `DeploymentExecutor` runs
//! the machine to completion via Valtron's `execute()`.
//!
//! HOW: State transitions emit progress via `Yield()` and final result via `Complete()`.
//! Errors propagate through `Complete(Err(e))` — never `StateTransition::Error` (swallowed).

mod executor;
mod planner;

pub use executor::DeploymentExecutor;
pub use planner::{DeployOutcome, DeployState, DeploymentPlanner};

/// Default number of health-check retries during verification.
pub const DEFAULT_VERIFY_RETRIES: u32 = 5;
/// Default delay between verification retries (executor handles the sleep).
pub const DEFAULT_VERIFY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);
