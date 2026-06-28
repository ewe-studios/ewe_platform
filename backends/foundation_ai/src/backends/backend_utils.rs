use crate::types::base_types::{CostStatus, ModelId, ToolShed, UsageCosting, UsageReport};
use crate::types::Tool;

// ============================================================================
// Helper Functions
// ============================================================================

/// Flatten a `ToolShed` into a flat Vec<Tool> for provider APIs.
#[must_use]
pub fn flatten_tools(shed: &ToolShed) -> Vec<Tool> {
    shed.all_tools()
}

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

pub fn json_value_to_arg_type(v: &serde_json::Value) -> crate::types::base_types::ArgType {
    match v {
        serde_json::Value::String(s) => crate::types::base_types::ArgType::Text(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                crate::types::base_types::ArgType::I64(i)
            } else if let Some(f) = n.as_f64() {
                crate::types::base_types::ArgType::Float64(f)
            } else {
                crate::types::base_types::ArgType::Text(n.to_string())
            }
        }
        other => crate::types::base_types::ArgType::JSON(other.to_string()),
    }
}

pub fn model_id_to_string(id: &ModelId) -> String {
    match id {
        ModelId::Name(name, _) => name.clone(),
        ModelId::Alias(alias, _) => alias.clone(),
        ModelId::Group(group, _) => group.clone(),
        ModelId::Architecture(arch, _) => arch.clone(),
    }
}
