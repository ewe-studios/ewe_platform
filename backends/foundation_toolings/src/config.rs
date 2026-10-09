// Route handler types — raw HTTP handler registry (no hyper).

use std::collections::HashMap;
use std::sync::Arc;

/// Raw HTTP route handler.
pub type RouteHandler = dyn Fn(&str) -> Vec<u8> + Send + Sync + 'static;

/// Route map — path → handler.
pub type RouteMap = HashMap<String, Arc<RouteHandler>>;
