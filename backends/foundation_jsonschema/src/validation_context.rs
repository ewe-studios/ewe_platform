//! Stub: ValidationContext — will be implemented in Feature 4.
//!
//! WHY: Recursive schemas can cause infinite validation loops. The context
//! tracks which (schema_node, instance_location) pairs are in-progress and
//! caches results to break cycles and avoid redundant work.

/// Mutable state for a single validation run.
pub struct ValidationContext;
