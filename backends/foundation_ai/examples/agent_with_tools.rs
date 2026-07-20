//! Example: Agent with custom tools (read, search, shell).
//!
//! Demonstrates:
//!   - Building a ToolShed with custom tools
//!   - Registering tools with JSON Schema argument definitions
//!   - Wiring the toolshed into an agent session
//!
//! Run with:
//! ```bash
//! ANTHROPIC_API_KEY=sk-... cargo run -p foundation_ai \
//!   --example agent_with_tools --features agentic
//! ```

use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::harness;
use foundation_ai::types::{Args, MessageRole, Messages, SessionId, TextContent, Tool, ToolShed, UserModelContent};
use foundation_compact::ids::new_scru128;
use foundation_core::valtron::valtron;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_jsonschema::scheme;

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

#[valtron]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    // Build a ToolShed with read, search, and shell tools.
    // In a real application, you'd wire ToolImpl implementations that actually
    // execute these operations. Here we register the tool definitions so the
    // agent can see and request them.
    let toolshed = ToolShed::default()
        .with_read(Some(Tool {
            name: "read_file".into(),
            description: "Read a file from the filesystem. Returns the file content.".into(),
            arguments: Some(Args::new(
                scheme::object()
                    .required("path", scheme::string().description("Absolute path to the file"))
                    .build(),
            )),
            returns: Some(Args::new(
                scheme::object()
                    .required("content", scheme::string())
                    .build(),
            )),
        }))
        .with_search(Some(Tool {
            name: "search".into(),
            description: "Search for information in the knowledge base.".into(),
            arguments: Some(Args::new(
                scheme::object()
                    .required("query", scheme::string().min_len(1).description("Search query"))
                    .build(),
            )),
            returns: Some(Args::new(
                scheme::object()
                    .required("results", scheme::array())
                    .build(),
            )),
        }))
        .with_shell(Some(Tool {
            name: "shell".into(),
            description: "Execute a shell command and return stdout/stderr.".into(),
            arguments: Some(Args::new(
                scheme::object()
                    .required("command", scheme::string().description("Shell command to execute"))
                    .build(),
            )),
            returns: Some(Args::new(
                scheme::object()
                    .required("stdout", scheme::string())
                    .required("exit_code", scheme::integer())
                    .build(),
            )),
        }));

    // Print the registered tool names.
    let tools = toolshed.all_tools();
    println!("ToolShed has {} tools:", tools.len());
    for tool in &tools {
        println!("  - {} ({})", tool.name, tool.description);
    }

    // Build the agent with the toolshed.
    let builder = harness::claude_session::<Doc, Mem>(SessionId::new(), &api_key)?;

    let agent = builder
        .with_toolshed(toolshed)
        .with_system_prompt("You are a helpful assistant with file access, search, and shell capabilities.")
        .build()?;

    // Ask the agent to use a tool.
    let prompt = Messages::User {
        id: new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "Hello! What tools do you have available?".into(),
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
