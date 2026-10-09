//! Request-scoped context helpers for Workers routes.
//!
//! WHY: Workers passes `RouteContext<Data>` to non-DO handlers; the data
//! often includes D1 bindings. Wrapping the extraction pattern here avoids
//! repeating it in every route handler.
//!
//! WHAT: thin wrappers that extract typed bindings from the route context.

use worker::{RouteContext, Result as WorkerResult};

/// Run a closure with a D1 database extracted from the route data.
///
/// The route data must be a `D1Database` (set up by the Worker's `fn
/// router()` or by the `#[event(fetch)]` init path that stores the
/// env-extracted DB into the context).
///
/// # Panics
///
/// Panics if the route data is not a `D1Database`. This is a setup error —
/// the route data type is determined at compile time by the Worker's router.
pub fn with_d1<T>(
    ctx: &RouteContext<worker::D1Database>,
    f: impl FnOnce(&worker::D1Database) -> T,
) -> T {
    f(&ctx.data)
}

/// Run an async closure with a D1 database from the route context.
///
/// Async variant for handlers that need `.await`.
pub async fn with_d1_async<T, F, Fut>(ctx: &RouteContext<worker::D1Database>, f: F) -> WorkerResult<T>
where
    F: FnOnce(&worker::D1Database) -> Fut,
    Fut: std::future::Future<Output = WorkerResult<T>>,
{
    f(&ctx.data).await
}
