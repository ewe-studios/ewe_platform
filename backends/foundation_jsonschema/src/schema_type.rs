use serde_json::Value;

/// Generate a standard JSON Schema representation for a Rust type.
///
/// Implemented automatically via `#[derive(JsonSchema)]` in `foundation_macros`.
pub trait JsonSchema {
    /// Returns the JSON Schema for this type as a `serde_json::Value`.
    fn json_schema() -> Value;
}
