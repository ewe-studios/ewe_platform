//! Example: Agent with custom tools via ToolPreset and ToolShed.
//!
//! Demonstrates:
//!   - Building tools with ToolImpl (F19 unified tool model)
//!   - Using ToolPreset for quick tool registration
//!   - Wiring tools into an agent session
//!
//! Run with:
//! ```bash
//! ANTHROPIC_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example agent_with_tools --features agentic
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use foundation_ai::agentic::tool_impl::{
    ToolCallManager, ToolCallResult, ToolDefinition, ToolError, ToolImpl,
};
use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::harness;
use foundation_ai::types::{
    ArgType, Args, MessageRole, Messages, SessionId, TextContent, Tool, ToolShed, UserModelContent,
};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_jsonschema::scheme;

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

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
    let builder = harness::claude_session::<Doc, Mem>(SessionId::new(), &api_key)?;

    // 2. Build the agent session — tools are registered on the session's
    //    ToolCallManager after build().
    let agent = builder
        .with_system_prompt(
            "You are a helpful assistant with a custom greeting tool.",
        )
        .build()?;

    // 3. Register tools (programmatically — or use ToolPreset for built-in tools)
    agent.tool_manager().register(Arc::new(GreetTool));

    // 4. Build the ToolShed (what the model sees)
    let toolshed = agent.tool_manager().build_toolshed();
    println!("ToolShed has {} tool(s):", toolshed.tools.len());
    for tool in &toolshed.tools {
        println!("  - {} ({})", tool.name(), tool.arg_summary());
    }

    // 5. Ask the agent to use the tool
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
