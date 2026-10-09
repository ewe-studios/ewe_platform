use crate::types::base_types::{CostStatus, ModelId, ToolDeclarations, UsageCosting, UsageReport};
use crate::types::{Tool, ToolArguments};

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

/// A tool call's arguments from the JSON value a provider sent (Anthropic's
/// `input`, a parsed `arguments` string).
///
/// An object is the arguments as is. Anything else is not a valid argument
/// object: it is logged and the call gets no arguments, so schema validation
/// reports the missing fields to the model instead of the tool running on
/// something it never declared.
#[must_use]
pub fn tool_arguments_from_value(tool: &str, value: serde_json::Value) -> Option<ToolArguments> {
    match value {
        serde_json::Value::Object(map) => Some(map),
        serde_json::Value::Null => None,
        other => {
            tracing::warn!(
                tool,
                arguments = %other,
                "tool call arguments are not a JSON object; dropping them"
            );
            None
        }
    }
}

/// A tool call's arguments from the JSON text a provider sent (`OpenAI`'s
/// `function.arguments`).
///
/// Empty text means no arguments. Text that does not parse is logged and the
/// call gets no arguments (see [`tool_arguments_from_value`]).
#[must_use]
pub fn tool_arguments_from_str(tool: &str, text: &str) -> Option<ToolArguments> {
    if text.trim().is_empty() {
        return None;
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => tool_arguments_from_value(tool, value),
        Err(error) => {
            tracing::warn!(
                tool,
                arguments = text,
                %error,
                "tool call arguments are not valid JSON; dropping them"
            );
            None
        }
    }
}

#[must_use]
pub fn model_id_to_string(id: &ModelId) -> String {
    match id {
        ModelId::Name(name, _) => name.clone(),
        ModelId::Alias(alias, _) => alias.clone(),
        ModelId::Group(group, _) => group.clone(),
        ModelId::Architecture(arch, _) => arch.clone(),
    }
}
