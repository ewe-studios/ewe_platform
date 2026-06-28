# How-To: Using the AgentSession API

Zero-to-expert guide for creating, running, and managing an AI agent session.

---

## Quick Start

The shortest path to a working agent:

```rust
use foundation_ai::agentic::session::AgentSession;
use foundation_ai::types::{ModelId, Messages, SessionId, UserModelContent, MessageRole, TextContent};
use foundation_ai::types::{ProviderRouter, ToolShed};
use foundation_ai::costing::calculate_cost;

// 1. Create a provider router with your model provider
let router = ProviderRouter::single(Box::new(my_openai_provider));

// 2. Create a session
let session = AgentSession::builder(SessionId::new(), router).build()?;

// 3. Run a turn
let prompt = Messages::User {
    id: foundation_compact::ids::new_scru128(),
    role: MessageRole::User,
    content: UserModelContent::Text(TextContent {
        content: "Explain Rust lifetimes".into(),
        signature: None,
    }),
    signature: None,
};

let records = session.run_turn(prompt)?;
for record in &records {
    match record {
        SessionRecord::Conversation { message: Messages::Assistant { content, .. } } => {
            if let ModelOutput::Text(t) = content {
                println!("{}", t.content);
            }
        }
        _ => {}
    }
}

// 4. Clean up
session.end()?;
```

---

## 1. Creating a ProviderRouter

The `ProviderRouter` decides which model provider handles each request.

### Single provider (most common)

```rust
use foundation_ai::types::ProviderRouter;

// If you have one provider (e.g., OpenAI):
let router = ProviderRouter::single(Box::new(openai_provider));

// openai_provider implements RoutableProvider:
// - supports() returns true for models this provider can handle
// - generate() / generate_stream() produce the response
```

### Multiple providers with routing rules

```rust
let router = ProviderRouter::builder()
    .add_provider(Box::new(openai_provider))
    .add_provider(Box::new(anthropic_provider))
    .add_provider(Box::new(llamacpp_provider))
    .rule(RoutingRule {
        model: ModelId::Name("gpt-4".into(), None),
        provider_name: "openai".into(),  // match by provider name()
    })
    .rule(RoutingRule {
        model: ModelId::Name("claude-sonnet-4-6".into(), None),
        provider_name: "anthropic".into(),
    })
    .rule(RoutingRule {
        model: ModelId::Name("local-llama".into(), None),
        provider_name: "llama-cpp".into(),
    })
    .build();
```

### Adding providers with custom names

Providers get their name from `provider.describe()`. If a provider returns
`None` from `describe()`, construction **panics** — every provider must have
a name and identity. Use `with_identity()` to set them explicitly:

```rust
use foundation_ai::types::{RoutableProviderBox, ModelProviders};

// If provider.describe() works (most built-in providers):
let routed = RoutableProviderBox::new(openai_provider);
// → name: "openai", provider_id: ModelProviders::OpenAI

// If provider has no descriptor, set explicitly:
let routed = RoutableProviderBox::with_identity(
    my_custom_provider,
    "my-provider",                  // custom name for routing
    ModelProviders::Custom("my-provider".into()),
);
```

### Model identification

```rust
// By exact name
ModelId::Name("gpt-4".into(), None)
ModelId::Name("claude-sonnet-4-6".into(), None)

// By alias (user-friendly names that map to concrete models)
ModelId::Alias("default".into(), None)
ModelId::Alias("fast".into(), None)

// By group (semantic categories)
ModelId::Group("coding".into(), None)
ModelId::Group("reasoning".into(), None)
```

---

## 2. Building an AgentSession

The builder requires `SessionId` and `ProviderRouter`, everything else is optional.

### Minimal session

```rust
let session = AgentSession::builder(SessionId::new(), router).build()?;
```

### Full configuration

```rust
use foundation_ai::agentic::session::AgentSession;
use foundation_ai::agentic::access::AllowAllAccess;
use foundation_ai::agentic::errors::{AgenticError, ErrorPolicy, AgentAction, GenKind};
use foundation_ai::types::UserId;

let session = AgentSession::builder(SessionId::new(), router)
    // Required: ToolShed (what tools the agent can use)
    .with_toolshed(my_toolshed)
    
    // Access control (who can do what)
    .with_access(Arc::new(my_auth_manager))
    .with_user(UserId("alice".into()))
    
    // Model selection
    .with_model(ModelId::Name("claude-sonnet-4-6".into(), None))
    .with_fallback_models(vec![
        ModelId::Name("gpt-4".into(), None),
        ModelId::Name("local-llama".into(), None),
    ])
    .with_memory_model(ModelId::Name("embedding-model".into(), None))
    
    // Storage (defaults to in-memory)
    .with_doc_store(my_document_store)
    .with_memory_store(my_memory_store)
    
    // Prompt and behavior
    .with_system_prompt("You are a helpful coding assistant.")
    .with_error_policy(ErrorPolicy::customize(|error| {
        match error {
            AgenticError::Generation(f) if f.kind == GenKind::RateLimit => {
                AgentAction::RetryWithReducedContext
            }
            _ => ErrorPolicy::new().classify(error),
        }
    }))
    .with_config(AgentConfig {
        primary_model: ModelId::Name("claude-sonnet-4-6".into(), None),
        max_inner_iterations: 10,    // max tool call rounds per turn
        max_outer_iterations: 5,     // max conversation rounds
        circuit_breaker_threshold: 3, // failures before fallback
        ..AgentConfig::default()
    })
    .build()?;
```

### ToolShed setup

```rust
use foundation_ai::agentic::tools::shed::ToolShed;
use foundation_ai::agentic::tool_impl::ToolCallManager;
use foundation_ai::types::SessionId;

// Create a ToolShed with tools
let mut toolshed = ToolShed::default();

// Register tools
let manager = ToolCallManager::new(SessionId::new());
manager.register(Arc::new(ReadFileTool::new(vfs)));
manager.register(Arc::new(ShellTool::new(shell)));
manager.register(Arc::new(SearchContextTool::new(context)));

// Build the toolshed from the manager
let toolshed = manager.build_toolshed();
```

---

## 3. Running Turns

### Streaming (recommended)

```rust
use foundation_core::valtron::Stream;

let prompt = /* ... create Messages::User ... */;
let stream = session.run_turn_stream(prompt)?;

for item in stream {
    match item {
        Stream::Pending(progress) => {
            // Agent is working — show progress
            match progress {
                AgentProgress::Initializing { step } => println!("Initializing: {}", step),
                AgentProgress::Generating { model, tokens_so_far } => {
                    println!("Generating with {}... ({} tokens)", model.name, tokens_so_far.unwrap_or(0));
                }
                AgentProgress::ToolCallRequested { name } => println!("Calling tool: {}", name),
                AgentProgress::ExecutingTools { total, completed } => {
                    println!("Executing tools: {}/{}", completed, total);
                }
                AgentProgress::ProcessingMemory { kind } => println!("Processing memory: {:?}", kind),
                AgentProgress::SessionEnding => println!("Session ending"),
            }
        }
        Stream::Next(record) => {
            // Agent produced something
            match record {
                SessionRecord::Conversation { message } => {
                    if let Messages::Assistant { content, .. } = message {
                        match content {
                            ModelOutput::Text(t) => print!("{}", t.content),
                            ModelOutput::ToolCall { name, arguments, .. } => {
                                println!("\nTool call: {} {:?}", name, arguments);
                            }
                            _ => {}
                        }
                    }
                }
                SessionRecord::FailedAction { error, .. } => {
                    eprintln!("Error: {:?}", error);
                    break;
                }
                _ => {}
            }
        }
        Stream::Spread(items) => {
            // Multiple items at once (batched output)
            for item in items {
                // handle each item
            }
        }
        _ => {} // Init, Ignore, Wait, Delayed — control signals
    }
}
```

### Non-streaming (collect all results)

```rust
let records = session.run_turn(prompt)?;
// records: Vec<SessionRecord>

for record in &records {
    if let SessionRecord::Conversation { message: Messages::Assistant { content, .. } } = record {
        if let ModelOutput::Text(t) = content {
            println!("{}", t.content);
        }
    }
}
```

### Multi-turn conversation

```rust
// First turn
let response1 = session.run_turn(user_message("What is Rust?"))?;

// Second turn (session remembers context)
let response2 = session.run_turn(user_message("How does borrowing work?"))?;

// Third turn
let response3 = session.run_turn(user_message("Show me an example"))?;
```

---

## 4. Steering and Interruption

### High-priority steering (interrupts current work)

```rust
// Agent is generating a response — interrupt it
session.steer(Messages::User {
    // ... new urgent message
});
```

### Follow-up (processed after current work)

```rust
// Queue a follow-up for after the current generation completes
session.follow_up(Messages::User {
    // ... follow-up question
});
```

---

## 5. Session Lifecycle

### End session (teardown)

```rust
// Flushes message buffer, drains queues, persists remaining state
session.end()?;
```

What `end()` does:
1. Flushes the Message API buffer (writes pending messages to storage)
2. Drains priority and follow-up queues → persists as conversation records
3. Flushes again (ensure everything is persisted)
4. Resets the cancel signal (clean state for potential resume)

### Resume session (deterministic replay)

```rust
use foundation_ai::agentic::session::AgentSession;

// Resume a previous session by its SessionId
let session = AgentSession::resume(
    saved_session_id,    // The ID from the original session
    router,              // ProviderRouter (can be different from original)
    AgentConfig::default(),
)?;

// Resume protocol (Decision 01 order):
// 1. Load WorkingMemory
// 2. Load Observation + Reflection (observation if no reflection exists)
// 3. Load recent 10 messages from MessageApi
// 4. Semantic recall (deferred — F31/F32)
// 5. Assemble context: system → working → reflection → recent → recalled
// 6. Queues start EMPTY (were drained+persisted on prior end())
// 7. ToolCallManager fresh

// Continue the conversation
let response = session.run_turn(user_message("Continue from where we left off"))?;
```

---

## 6. Access Control

### Default: AllowAllAccess

By default, anyone can use any model and tool:

```rust
let session = AgentSession::builder(SessionId::new(), router).build()?;
// Uses AllowAllAccess internally
```

### Custom access provider

```rust
struct MyAccessProvider { /* ... */ }

impl SessionAccessProvider for MyAccessProvider {
    fn can_access_session(&self, user: &UserId, session: &SessionId) -> Result<bool, AuthError> {
        // Check if user owns this session
        Ok(true)
    }
    
    fn can_use_model(&self, user: &UserId, model: &str) -> Result<bool, AuthError> {
        // Check if user is allowed to use this model
        if model == "gpt-4" && !user.is_premium() {
            return Ok(false);
        }
        Ok(true)
    }
    
    fn can_use_tool(&self, user: &UserId, tool: &str) -> Result<bool, AuthError> {
        // Check if user can use this tool
        if tool == "shell" && !user.is_admin() {
            return Ok(false);
        }
        Ok(true)
    }
    
    fn can_spend(&self, user: &UserId, tokens: u64) -> Result<bool, AuthError> {
        // Check if user has enough token budget
        Ok(user.remaining_budget() >= tokens)
    }
    
    fn token_budget(&self, user: &UserId) -> Result<TokenBudget, AuthError> {
        Ok(TokenBudget {
            limit: Some(user.max_tokens()),
            used: user.tokens_used(),
        })
    }
}

let session = AgentSession::builder(SessionId::new(), router)
    .with_access(Arc::new(MyAccessProvider))
    .build()?;
```

---

## 7. Configuration Reference

### AgentConfig

| Field | Default | Description |
|---|---|---|
| `primary_model` | None | The model to use for generation |
| `fallback_models` | `[]` | Models to try if primary fails |
| `memory_model` | None | Model for memory generation |
| `max_inner_iterations` | 10 | Max tool call rounds per turn |
| `max_outer_iterations` | 5 | Max conversation rounds |
| `circuit_breaker_threshold` | 3 | Failures before fallback |
| `context_pressure_threshold` | 0.8 | Context window pressure (0.0-1.0) |
| `preflight_compression_threshold` | 0.9 | Compress before this ratio |

### ContextConfig

| Field | Default | Description |
|---|---|---|
| `recent_message_count` | 10 | Number of recent messages to include |
| `semantic_recall_count` | 5 | Number of semantic recall results |

### MemoryConfig

| Field | Default | Description |
|---|---|---|
| `observation_trigger_tokens` | 30_000 | Tokens before observation generation |
| `reflection_trigger_tokens` | 40_000 | Tokens before reflection generation |

---

## 8. Common Patterns

### Simple Q&A bot

```rust
fn run_qa(router: ProviderRouter, questions: Vec<String>) -> Result<Vec<String>, ErrorTrace<AgenticError>> {
    let session = AgentSession::builder(SessionId::new(), router)
        .with_system_prompt("Answer concisely.")
        .build()?;
    
    let mut answers = Vec::new();
    for q in questions {
        let records = session.run_turn(user_message(&q))?;
        for record in &records {
            if let Some(text) = extract_assistant_text(record) {
                answers.push(text);
            }
        }
    }
    session.end()?;
    Ok(answers)
}
```

### Streaming chat interface

```rust
fn chat_stream(session: &AgentSession, prompt: String) -> impl Iterator<Item = String> {
    let stream = session.run_turn_stream(user_message(&prompt)).unwrap();
    
    // Filter to just text tokens
    stream.flat_map(|item| {
        match item {
            Stream::Next(SessionRecord::Conversation { message }) => {
                if let Messages::Assistant { content: ModelOutput::Text(t), .. } = message {
                    Some(t.content)
                } else {
                    None
                }
            }
            _ => None,
        }
    })
}
```

### Tool-using agent

```rust
// 1. Set up tools
let manager = ToolCallManager::new(SessionId::new());
manager.register(Arc::new(ReadFileTool::new(vfs)));
manager.register(Arc::new(ShellTool::new(shell)));
let toolshed = manager.build_toolshed();

// 2. Create session with tools
let session = AgentSession::builder(SessionId::new(), router)
    .with_toolshed(toolshed)
    .with_system_prompt("You can read files and run shell commands.")
    .build()?;

// 3. Run — agent will automatically call tools when needed
let records = session.run_turn(user_message("What's in the Cargo.toml?"))?;

// The agent will:
// 1. Generate → decides to call read_file
// 2. Tool executes → returns file contents
// 3. Agent resumes with tool results → generates response
```

### Session persistence and resume

```rust
// Save session ID for later
let session_id = SessionId::new();
println!("Session ID: {}", session_id);

// Create and use session
let session = AgentSession::builder(session_id.clone(), router).build()?;
session.run_turn(user_message("Let's start a project"))?;
session.end()?;

// ... later, in a different process ...

// Resume the session
let resumed = AgentSession::resume(session_id, router, AgentConfig::default())?;
resumed.run_turn(user_message("What were we working on?"))?;
```
