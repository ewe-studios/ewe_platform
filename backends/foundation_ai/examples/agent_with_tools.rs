//! Example: Agent with custom tools via ToolPreset and ToolShed.
//!
//! Demonstrates:
//!   - Building tools with ToolImpl (F19 unified tool model)
//!   - Giving them to a session through a ToolShed
//!   - What the model sees: the `shed` meta-tool, then the tools it activates
//!
//! Run with:
//! ```bash
//! ANTHROPIC_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example agent_with_tools --features agentic
//! ```

use std::collections::HashMap;

use async_trait::async_trait;
use foundation_ai::agentic::tool_impl::{ToolCallResult, ToolDefinition, ToolError, ToolImpl};
use foundation_ai::agentic::ToolShed;
use foundation_ai::harness;
use foundation_ai::types::{
    ArgType, Args, MessageRole, Messages, SessionId, TextContent, Tool, UserModelContent,
};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;
use foundation_jsonschema::scheme;

// ---------------------------------------------------------------------------
// Custom tool: greet
// ---------------------------------------------------------------------------

struct GreetTool;

#[async_trait]
impl ToolImpl for GreetTool {
    fn definition(&self) -> Tool {
        Tool::SingleCommand(ToolDefinition {
            name: "greet".into(),
            category: "custom".into(),
            description: "Greet someone by name.".into(),
            arguments: Args::new(
                scheme::object()
                    .required(
                        "name",
                        scheme::string().min_len(1).description("Name to greet"),
                    )
                    .build(),
            ),
            returns: Some(Args::new(
                scheme::object()
                    .required("greeting", scheme::string())
                    .build(),
            )),
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let name = match arguments.get("name") {
            Some(ArgType::Text(s)) => s.clone(),
            _ => {
                return Err(ToolError::InvalidArguments {
                    tool: "greet".into(),
                    reason: "missing 'name'".into(),
                })
            }
        };
        Ok(ToolCallResult {
            content: UserModelContent::Text(TextContent {
                content: format!("Hello, {name}!"),
                signature: None,
            }),
            error_detail: None,
        })
    }
}

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");

    // 1. Build the model preset (Claude Opus main + Sonnet memory)
    let builder = harness::claude_session(SessionId::new(), &api_key)?;

    // 2. Build the agent session with its tools. The ToolShed is the one list
    //    of what the agent can call (add ToolPreset::files(fs) etc. the same way).
    let agent = builder
        .with_system_prompt("You are a helpful assistant with a custom greeting tool.")
        .with_toolshed(ToolShed::new().tool(GreetTool))
        .build()?;

    // 3. What the model can reach through `shed`, and what it is offered now
    //    (just `shed` until `shed` returns a tool).
    let all = agent.tool_manager().all_declarations();
    println!("The session has {} tool(s):", all.tools.len());
    for tool in &all.tools {
        println!("  - {} ({})", tool.name(), tool.arg_summary());
    }
    let offered = agent.tool_manager().offered_tools();
    println!(
        "Offered on the first request: {:?}",
        offered
            .all_tools()
            .iter()
            .map(Tool::name)
            .collect::<Vec<_>>()
    );

    // 4. Ask the agent to use the tool
    let prompt = Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Hello! Please use the greet tool to greet Alice.".into(),
            signature: None,
        }),
        signature: None,
    };

    println!("\nAsking the agent about its tools...");
    let records = agent.run_turn(prompt)?;

    for record in &records {
        println!("{record:?}");
    }

    println!("\nAgent responded! Got {} records.", records.len());
    agent.end()?;
    Ok(())
}
