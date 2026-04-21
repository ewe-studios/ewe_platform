//! Authentication state machine — tracks auth lifecycle and handles concurrent requests.
//!
//! WHY: Multiple requests may trigger token refresh simultaneously. This ensures
//! only one refresh happens and others queue.
//!
//! WHAT: `AuthStateMachine` with state transitions and request queuing.
//! HOW: Synchronous state transitions. No Valtron needed (enum logic only).

use std::collections::VecDeque;

/// Authentication states in the lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    /// Not authenticated — no credentials available.
    Unauthenticated,
    /// Authentication in progress (e.g. OAuth redirect pending).
    Authenticating,
    /// Authenticated with valid credentials.
    Authenticated,
    /// Token has expired — refresh required.
    TokenExpired,
    /// Token refresh in progress.
    Refreshing,
    /// Authentication failed (terminal until reset).
    Failed,
}

impl AuthState {
    /// Whether requests can proceed in this state.
    #[must_use]
    pub fn can_make_request(self) -> bool {
        matches!(self, AuthState::Authenticated)
    }

    /// Whether this state is terminal (requires explicit reset to leave).
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, AuthState::Failed)
    }

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Unauthenticated => "Unauthenticated",
            Self::Authenticating => "Authenticating",
            Self::Authenticated => "Authenticated",
            Self::TokenExpired => "TokenExpired",
            Self::Refreshing => "Refreshing",
            Self::Failed => "Failed",
        }
    }
}

/// Events that trigger state transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthEvent {
    /// User initiated login/OAuth flow.
    AuthenticateStarted,
    /// User completed login (code/token received).
    AuthenticateCompleted,
    /// Token expired or near-expiry detected.
    TokenExpired,
    /// Token refresh started.
    RefreshStarted,
    /// Token refresh succeeded.
    RefreshCompleted,
    /// Token refresh failed.
    RefreshFailed,
    /// Explicit logout / credential clear.
    Logout,
}

/// Pending request queued during a refresh.
#[derive(Debug)]
pub struct QueuedRequest {
    /// Unique ID for tracking.
    pub id: u64,
}

/// State machine for authentication lifecycle.
pub struct AuthStateMachine {
    current: AuthState,
    queue: VecDeque<QueuedRequest>,
    next_request_id: u64,
}

impl Default for AuthStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthStateMachine {
    /// Create a new state machine in `Unauthenticated` state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: AuthState::Unauthenticated,
            queue: VecDeque::new(),
            next_request_id: 0,
        }
    }

    /// Current authentication state.
    #[must_use]
    pub fn current(&self) -> AuthState {
        self.current
    }

    /// Attempt to transition to a new state based on an event.
    ///
    /// Returns `Ok(())` if the transition is valid, or `Err` if the
    /// event is not allowed in the current state.
    ///
    /// # Errors
    ///
    /// Returns `AuthStateError` if the transition is invalid.
    pub fn transition_to(&mut self, event: AuthEvent) -> Result<(), AuthStateError> {
        let current = self.current;
        let next = match (current, event) {
            // AuthenticateStarted: Unauthenticated → Authenticating, Failed → Authenticating
            (AuthState::Unauthenticated | AuthState::Failed, AuthEvent::AuthenticateStarted) => {
                AuthState::Authenticating
            }

            // RefreshStarted: Unauthenticated → Refreshing, TokenExpired → Refreshing
            (AuthState::Unauthenticated | AuthState::TokenExpired, AuthEvent::RefreshStarted) => {
                AuthState::Refreshing
            }

            // Completed → Authenticated
            (AuthState::Authenticating, AuthEvent::AuthenticateCompleted)
            | (AuthState::Refreshing, AuthEvent::RefreshCompleted) => AuthState::Authenticated,

            // Logout from any active state → Unauthenticated
            (
                AuthState::Authenticating
                | AuthState::Authenticated
                | AuthState::TokenExpired
                | AuthState::Refreshing
                | AuthState::Failed,
                AuthEvent::Logout,
            ) => AuthState::Unauthenticated,

            // RefreshFailed → Failed
            (AuthState::Authenticating | AuthState::Refreshing, AuthEvent::RefreshFailed) => {
                AuthState::Failed
            }

            // Token near-expiry → TokenExpired
            (AuthState::Authenticated, AuthEvent::TokenExpired) => AuthState::TokenExpired,

            // Invalid transitions
            (from, event) => {
                return Err(AuthStateError::InvalidTransition {
                    from,
                    event: format!("{event:?}"),
                });
            }
        };

        self.current = next;
        Ok(())
    }

    /// Handle an event, transitioning state and returning any queued requests
    /// to process if refresh completed.
    ///
    /// # Errors
    ///
    /// Returns `AuthStateError` if the transition is invalid.
    pub fn handle_event(&mut self, event: AuthEvent) -> Result<Vec<QueuedRequest>, AuthStateError> {
        let was_refreshing = self.current == AuthState::Refreshing;
        self.transition_to(event)?;

        // If refresh completed, return all queued requests to proceed
        if was_refreshing && self.current == AuthState::Authenticated {
            let drained = self.queue.drain(..).collect::<Vec<_>>();
            return Ok(drained);
        }

        Ok(Vec::new())
    }

    /// Enqueue a request that arrived during a refresh.
    pub fn enqueue_request(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        self.queue.push_back(QueuedRequest { id });
        id
    }

    /// Number of requests currently queued.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Whether a refresh is in progress (requests should queue).
    #[must_use]
    pub fn is_refreshing(&self) -> bool {
        self.current == AuthState::Refreshing
    }

    /// Reset the state machine to `Unauthenticated`.
    pub fn reset(&mut self) {
        self.current = AuthState::Unauthenticated;
        self.queue.clear();
    }
}

/// Auth state machine errors.
#[derive(derive_more::From, Debug)]
pub enum AuthStateError {
    /// Invalid state transition.
    #[from(ignore)]
    InvalidTransition { from: AuthState, event: String },
}

impl core::fmt::Display for AuthStateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AuthStateError::InvalidTransition { from, event } => {
                write!(f, "Invalid transition from {event:?} in state {from:?}")
            }
        }
    }
}

impl std::error::Error for AuthStateError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let sm = AuthStateMachine::new();
        assert_eq!(sm.current(), AuthState::Unauthenticated);
        assert!(!sm.is_refreshing());
    }

    #[test]
    fn test_successful_auth_flow() {
        let mut sm = AuthStateMachine::new();

        sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
        assert_eq!(sm.current(), AuthState::Authenticating);

        sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
        assert_eq!(sm.current(), AuthState::Authenticated);
        assert!(sm.current().can_make_request());
    }

    #[test]
    fn test_token_expiry_and_refresh() {
        let mut sm = AuthStateMachine::new();

        // Auth
        sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
        sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();

        // Token expires
        sm.transition_to(AuthEvent::TokenExpired).unwrap();
        assert_eq!(sm.current(), AuthState::TokenExpired);

        // Refresh
        sm.transition_to(AuthEvent::RefreshStarted).unwrap();
        assert!(sm.is_refreshing());

        // Enqueue requests during refresh
        let id1 = sm.enqueue_request();
        let id2 = sm.enqueue_request();
        assert_eq!(sm.queue_len(), 2);

        // Refresh completes — queue drained
        let drained = sm.handle_event(AuthEvent::RefreshCompleted).unwrap();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].id, id1);
        assert_eq!(drained[1].id, id2);
        assert_eq!(sm.queue_len(), 0);
        assert_eq!(sm.current(), AuthState::Authenticated);
    }

    #[test]
    fn test_refresh_failure() {
        let mut sm = AuthStateMachine::new();

        sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
        sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
        sm.transition_to(AuthEvent::TokenExpired).unwrap();
        sm.transition_to(AuthEvent::RefreshStarted).unwrap();

        // Refresh fails
        sm.transition_to(AuthEvent::RefreshFailed).unwrap();
        assert_eq!(sm.current(), AuthState::Failed);
        assert!(sm.current().is_terminal());
    }

    #[test]
    fn test_recovery_from_failure() {
        let mut sm = AuthStateMachine::new();

        sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
        sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
        sm.transition_to(AuthEvent::TokenExpired).unwrap();
        sm.transition_to(AuthEvent::RefreshStarted).unwrap();
        sm.transition_to(AuthEvent::RefreshFailed).unwrap();

        // Can retry auth
        sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
        assert_eq!(sm.current(), AuthState::Authenticating);
    }

    #[test]
    fn test_logout_from_any_state() {
        let mut sm = AuthStateMachine::new();

        sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
        sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
        sm.transition_to(AuthEvent::Logout).unwrap();
        assert_eq!(sm.current(), AuthState::Unauthenticated);
    }

    #[test]
    fn test_invalid_transition() {
        let mut sm = AuthStateMachine::new();
        // Cannot refresh from unauthenticated (must authenticate first)
        let result = sm.transition_to(AuthEvent::RefreshCompleted);
        assert!(result.is_err());
    }

    #[test]
    fn test_reset_clears_queue() {
        let mut sm = AuthStateMachine::new();
        sm.transition_to(AuthEvent::AuthenticateStarted).unwrap();
        sm.transition_to(AuthEvent::AuthenticateCompleted).unwrap();
        sm.transition_to(AuthEvent::TokenExpired).unwrap();
        sm.transition_to(AuthEvent::RefreshStarted).unwrap();

        sm.enqueue_request();
        sm.enqueue_request();
        assert_eq!(sm.queue_len(), 2);

        sm.reset();
        assert_eq!(sm.current(), AuthState::Unauthenticated);
        assert_eq!(sm.queue_len(), 0);
    }
}
