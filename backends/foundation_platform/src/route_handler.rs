//! RouteHandler trait and FnRouteHandler closure wrapper.
//!
//! NON-GENERIC — `PlatformSession` has no `R: Runtime` parameter.
//! Three API surfaces compose together (trait impl, closure, PatternRouter),
//! all implementing this single trait.

use foundation_ui_traits::*;
use crate::session::PlatformSession;

/// Route policy is code, not a config file. The platform intercepts every
/// navigation intent and delegates decisions to user-provided handlers.
///
/// Patterned after `foundation_http`'s `Serve` trait: trait object, user
/// implements, registers with the platform. Handlers return `Some(decision)`
/// to claim a navigation, or `None` to fall through.
pub trait RouteHandler: Send + Sync + 'static {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision>;
}

/// Wraps a closure as a `RouteHandler`. Use when the policy is simple.
pub struct FnRouteHandler<F>(pub F)
where
    F: Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision>
        + Send + Sync + 'static;

impl<F> FnRouteHandler<F>
where
    F: Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision>
        + Send + Sync + 'static,
{
    pub fn new(f: F) -> Self {
        FnRouteHandler(f)
    }
}

impl<F> RouteHandler for FnRouteHandler<F>
where
    F: Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision>
        + Send + Sync + 'static,
{
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision> {
        (self.0)(intent, session)
    }
}
