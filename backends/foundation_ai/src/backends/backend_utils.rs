use crate::types::base_types::{CostStatus, ModelId, ToolDeclarations, UsageCosting, UsageReport};
use crate::types::Tool;

// ============================================================================
// Helper Functions
// ============================================================================

/// Flatten a `ToolDeclarations` into a flat Vec<Tool> for provider APIs.
#[must_use]
pub fn flatten_tools(shed: &ToolDeclarations) -> Vec<Tool> {
    shed.all_tools()
}

#[must_use]
pub fn empty_usage_report() -> UsageReport {
    UsageReport {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCosting {
            currency: String::from("USD"),
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            status: CostStatus::Actual,
        },
    }
}

pub use crate::types::base_types::json_value_to_arg_type;

#[must_use]
pub fn model_id_to_string(id: &ModelId) -> String {
    match id {
        ModelId::Name(name, _) => name.clone(),
        ModelId::Alias(alias, _) => alias.clone(),
        ModelId::Group(group, _) => group.clone(),
        ModelId::Architecture(arch, _) => arch.clone(),
    }
}
