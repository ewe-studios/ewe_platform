//! `RouteHandler` trait and `FnRouteHandler` closure wrapper.
//!
//! NON-GENERIC — `PlatformSession` has no `R: Runtime` parameter.
//! Three API surfaces compose together (trait impl, closure, `PatternRouter`),
//! all implementing this single trait.

use foundation_ui_traits::{NavigationIntent, RouteDecision};
use crate::session::PlatformSession;

/// Route policy is code. Handlers return `Some(decision)` to claim
/// a navigation, or `None` to fall through.
pub trait RouteHandler: Send + Sync + 'static {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision>;
}

/// A registered handler that receives the resolved `RouteDecision` and
/// `NavigationIntent` and responds with the content to serve. Each
/// `webview_app()`, `ipc_shell()`, etc. registers its own responder.
///
/// Registered on the session by `handler_id`. `execute_decision()`
/// looks up the handler and calls `respond()` instead of going through
/// a central `BackendTransport`.
pub trait RouteResponder: Send + Sync + 'static {
    fn respond(
        &self,
        intent: &NavigationIntent,
        decision: &RouteDecision,
        session: &PlatformSession,
    ) -> tauri::http::Response<Vec<u8>>;
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
