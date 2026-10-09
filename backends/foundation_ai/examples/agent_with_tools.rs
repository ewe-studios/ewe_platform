//! Example: Agent with custom tools via `ToolShed`.
//!
//! Demonstrates:
//!   - A struct tool (`impl ToolImpl`) that reads its arguments with `ToolArgs`
//!   - A closure tool (`FnTool`)
//!   - Giving both to a session through a `ToolShed`
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
use foundation_ai::agentic::{FnTool, ToolArgs, ToolShed, TurnEvent};
use foundation_ai::harness;
use foundation_ai::types::{ArgType, Args, SessionId, Tool};
use foundation_core::valtron::valtron;
use foundation_jsonschema::scheme;

// ---------------------------------------------------------------------------
// A struct tool: greet
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
                    .optional("shout", scheme::boolean())
                    .build(),
            ),
            returns: None,
        })
    }

    async fn execute(
        &self,
        arguments: HashMap<String, ArgType>,
    ) -> Result<ToolCallResult, ToolError> {
        let args = ToolArgs::new("greet", &arguments);
        let name = args.str("name")?;
        let greeting = format!("Hello, {name}!");
        // `true` and `"true"` both read as true, whichever backend sent it.
        if args.opt_bool("shout")?.unwrap_or(false) {
            return Ok(ToolCallResult::text(greeting.to_uppercase()));
        }
        Ok(ToolCallResult::text(greeting))
    }
}

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key =
        std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY must be set");

    // A closure tool: no struct, no `impl ToolImpl`.
    let count_letters = FnTool::new(
        "count_letters",
        "Count the letters in a word.",
        Args::new(scheme::object().required("word", scheme::string()).build()),
        |args: ToolArgs<'_>| {
            let word = args.str("word").map(str::to_owned);
            async move {
                let count = word?.chars().filter(|c| c.is_alphabetic()).count();
                Ok(ToolCallResult::text(count.to_string()))
            }
        },
    );

    // 1. Build the model preset (Claude Opus main + Sonnet memory) and the
    //    session with its tools. The ToolShed is the one list of what the
    //    agent can call (add ToolPreset::files(fs) etc. the same way).
    let agent = harness::claude_session(SessionId::new(), &api_key)?
        .with_system_prompt("You are a helpful assistant with a few custom tools.")
        .with_toolshed(ToolShed::new().tool(GreetTool).tool(count_letters))
        .build()?;

    // 2. What the model can reach through `shed`, and what it is offered now
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

    // 3. Ask the agent to use the tools, watching the turn as it happens.
    println!("\nAsking the agent to greet Alice...");
    for event in agent
        .run_turn_stream("Please greet Alice, then tell me how many letters her name has.")?
        .events()
    {
        match event {
            TurnEvent::Text(delta) => print!("{delta}"),
            TurnEvent::ToolCall { name, .. } => println!("\n→ {name}"),
            TurnEvent::ToolResult { name, .. } => println!("← {name}"),
            TurnEvent::Failed(error) => return Err(format!("\nturn failed: {error}").into()),
            TurnEvent::Done(summary) => println!("\n[{} tokens]", summary.usage.total),
            _ => {}
        }
    }

    agent.end()?;
    Ok(())
}
