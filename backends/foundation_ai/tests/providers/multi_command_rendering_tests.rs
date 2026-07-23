//! Every provider renders a `Tool::MultiCommands` correctly (spec-60 F19).
//!
//! WHY: F19 replaced the bespoke `MemoryTool`/`DelegationTool` structs and their
//! dedicated `ToolShed` slots with one `Tool` enum, so a capability with several
//! commands is now `MultiCommands(name, Vec<ToolDefinition>)`. The whole point is
//! that providers render the enum rather than a flattened list — but the only
//! coverage was of the shared `Tool::function_spec()` helper in
//! `tests/tools/function_spec_tests.rs`. Nothing asserted that a *provider*
//! turns that into a payload the vendor will accept.
//!
//! That gap matters because each provider wraps the spec differently: OpenAI
//! nests it under `function`, Anthropic uses `input_schema` and has no
//! output-schema slot at all, and the text-based formatter (llama.cpp, candle)
//! folds the return schema into the description. A regression in any one of
//! them silently degrades a multi-command tool to an unusable one — the model
//! would see a tool it cannot select a command on.
//!
//! WHAT: for each `ToolFormatter`, render one `SingleCommand` and one
//! `MultiCommands` tool and assert the shape the vendor requires — in
//! particular that the sub-commands collapse into ONE tool entry discriminated
//! by a `command` field, not several tool entries.
//!
//! HOW: the formatters are pure (`&[Tool] -> serde_json::Value`), so no network,
//! no model and no credentials are involved.

use foundation_ai::backends::anthropic_messages_provider::AnthropicFormatter;
use foundation_ai::backends::openai_provider::OpenAIFormatter;
use foundation_ai::types::{Args, TextBasedFormatter, Tool, ToolDefinition, ToolFormatter};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn args(properties: serde_json::Value, required: &[&str]) -> Args {
    Args::new(foundation_jsonschema::ValidationOptions::with_schema(
        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
        }),
    ))
}

fn single() -> Tool {
    Tool::SingleCommand(ToolDefinition {
        name: "read".to_string(),
        category: "read".to_string(),
        description: "Read a file".to_string(),
        arguments: args(serde_json::json!({ "path": { "type": "string" } }), &["path"]),
        returns: None,
    })
}

/// A three-command `memory` tool — the shape F14/F19 actually ship.
fn multi() -> Tool {
    Tool::MultiCommands(
        "memory".to_string(),
        vec![
            ToolDefinition {
                name: "memory_add".to_string(),
                category: "memory".to_string(),
                description: "Add a memory".to_string(),
                arguments: args(
                    serde_json::json!({ "content": { "type": "string" } }),
                    &["content"],
                ),
                returns: None,
            },
            ToolDefinition {
                name: "memory_remove".to_string(),
                category: "memory".to_string(),
                description: "Remove a memory".to_string(),
                arguments: args(serde_json::json!({ "id": { "type": "string" } }), &["id"]),
                returns: None,
            },
            ToolDefinition {
                name: "memory_replace".to_string(),
                category: "memory".to_string(),
                description: "Replace a memory".to_string(),
                arguments: args(
                    serde_json::json!({
                        "id": { "type": "string" },
                        "content": { "type": "string" }
                    }),
                    &["id", "content"],
                ),
                returns: None,
            },
        ],
    )
}

fn render(formatter: &dyn ToolFormatter, tools: &[Tool]) -> Vec<serde_json::Value> {
    formatter
        .format_tools(tools)
        .expect("formatting tools must succeed")
        .as_array()
        .expect("a tool payload is always an array")
        .clone()
}

/// Every `const` value under a schema's `oneOf`/`anyOf` branches' `command`
/// property — i.e. the sub-commands the model may select.
fn commands_offered(schema: &serde_json::Value) -> Vec<String> {
    let branches = schema
        .get("oneOf")
        .or_else(|| schema.get("anyOf"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    branches
        .iter()
        .filter_map(|b| {
            b.get("properties")?
                .get("command")?
                .get("const")?
                .as_str()
                .map(str::to_string)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The invariant that matters everywhere
// ---------------------------------------------------------------------------

#[test]
fn a_multi_command_tool_is_one_entry_for_every_provider() {
    // The regression this guards: rendering a MultiCommands tool as one entry
    // PER COMMAND. The model would then see `memory_add`/`memory_remove`/
    // `memory_replace` as three unrelated tools, which is precisely the
    // flattened world F19 removed.
    let formatters: [(&str, Box<dyn ToolFormatter>); 3] = [
        ("openai", Box::new(OpenAIFormatter)),
        ("anthropic", Box::new(AnthropicFormatter)),
        ("text-based (llama.cpp / candle)", Box::new(TextBasedFormatter)),
    ];

    for (name, formatter) in formatters {
        let rendered = render(formatter.as_ref(), &[single(), multi()]);
        assert_eq!(
            rendered.len(),
            2,
            "{name}: two Tools must render as two entries, not one-per-command \
             (got {rendered:#?})"
        );
    }
}

// ---------------------------------------------------------------------------
// OpenAI
// ---------------------------------------------------------------------------

#[test]
fn openai_renders_a_multi_command_tool_as_one_discriminated_function() {
    let rendered = render(&OpenAIFormatter, &[multi()]);
    let func = rendered[0]
        .get("function")
        .expect("OpenAI nests the spec under `function`");

    assert_eq!(
        func.get("name").and_then(serde_json::Value::as_str),
        Some("memory"),
        "the function takes the GROUP name, not a sub-command name"
    );

    let params = func.get("parameters").expect("parameters must be present");
    let mut offered = commands_offered(params);
    offered.sort();
    assert_eq!(
        offered,
        vec!["memory_add", "memory_remove", "memory_replace"],
        "every sub-command must be selectable via `command`: {params:#}"
    );
}

#[test]
fn openai_still_renders_a_single_command_tool_flat() {
    let rendered = render(&OpenAIFormatter, &[single()]);
    let func = rendered[0].get("function").expect("function object");
    assert_eq!(
        func.get("name").and_then(serde_json::Value::as_str),
        Some("read")
    );
    assert!(
        commands_offered(func.get("parameters").expect("parameters")).is_empty(),
        "a leaf tool must not grow a `command` discriminator"
    );
}

// ---------------------------------------------------------------------------
// Anthropic
// ---------------------------------------------------------------------------

#[test]
fn anthropic_renders_a_multi_command_tool_under_input_schema() {
    // Anthropic's tool_use block uses `input_schema` and has no output-schema
    // slot, so the discriminator has to live inside input_schema.
    let rendered = render(&AnthropicFormatter, &[multi()]);
    let tool = &rendered[0];

    assert_eq!(
        tool.get("name").and_then(serde_json::Value::as_str),
        Some("memory")
    );

    let schema = tool
        .get("input_schema")
        .expect("Anthropic uses `input_schema`, not `parameters`");
    let mut offered = commands_offered(schema);
    offered.sort();
    assert_eq!(
        offered,
        vec!["memory_add", "memory_remove", "memory_replace"],
        "sub-commands must be selectable inside input_schema: {schema:#}"
    );
}

// ---------------------------------------------------------------------------
// Text-based (llama.cpp, candle, and the Responses provider's formatter)
// ---------------------------------------------------------------------------

#[test]
fn the_text_formatter_names_every_sub_command() {
    // Local models get the tool list as text, so the sub-commands have to be
    // discoverable from the rendered payload — otherwise the model can call the
    // tool but never pick a command.
    let rendered = render(&TextBasedFormatter, &[multi()]);
    let blob = serde_json::to_string(&rendered[0]).expect("serialisable");

    for command in ["memory_add", "memory_remove", "memory_replace"] {
        assert!(
            blob.contains(command),
            "the text payload must name `{command}` so the model can select it: {blob}"
        );
    }
}

#[test]
fn the_text_formatter_describes_how_to_select_a_command() {
    let rendered = render(&TextBasedFormatter, &[multi()]);
    let description = rendered[0]
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    assert!(
        description.to_lowercase().contains("command"),
        "the description must tell the model that `command` selects the \
         sub-command, or a multi-command tool is unusable: {description:?}"
    );
}
