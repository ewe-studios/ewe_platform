---
name: "Provider Prompt Assembly"
status: "completed"
priority: "high"
---

# Feature: Provider Prompt Assembly

## Description

Every provider assembles `ModelInteraction` into its request format by combining `{system_prompt, soul, tools_shed}` into the system-level portion of the prompt, then appending messages.

The strategy depends on what the provider supports:

### Path A: Provider has native system prompt AND native tools

Combine `system_prompt` + `soul` into the system prompt. Flatten `tools_shed` into a `Vec<Tool>` and pass it through the provider's existing `ToolFormatter.format_tools()` (which all providers already implement).

### Path B: Provider only has a single text prompt

Combine `{system_prompt, soul, tools_shed, messages}` into a single string with markdown section markers. Tools are formatted using the provider's `ToolFormatter` — `format_tools()` for tool definitions and `tool_calling_instructions()` for usage guidance.

## ModelInteraction Structure

The actual types in `types/mod.rs`:

```rust
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MemoryTool {
    pub add: Tool,
    pub replace: Tool,
    pub remove: Tool,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DelegationTool {
    pub start: Tool,
    pub check: Tool,
    pub get: Tool,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToolShed {
    pub shed: Tool,
    pub memory: Option<MemoryTool>,
    delegate: Option<DelegationTool>,
    pub read: Tool,
    pub edit: Tool,
    pub write: Tool,
    pub search: Tool,
    pub bash: Option<Tool>,
    pub others: Option<Vec<Tool>>,
}

#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelInteraction {
    pub system_prompt: Option<String>,
    pub soul: Option<String>,
    pub tools_shed: Option<ToolShed>,
    pub tool_choice: Option<ToolChoice>,
    pub chat_template: Option<String>,
    pub messages: Vec<Messages>,
}
```

Note: the old `tools: Vec<Tool>` field is gone. Tools now come from `ToolShed`.

## Tool Shed Flattening

All providers that handle tools flatten `ToolShed` into a `Vec<Tool>`:

```rust
fn flatten_tools(shed: &ToolShed) -> Vec<Tool> {
    let mut tools = vec![
        shed.shed.clone(),
        shed.read.clone(),
        shed.edit.clone(),
        shed.write.clone(),
        shed.search.clone(),
    ];
    if let Some(mem) = &shed.memory {
        tools.push(mem.add.clone());
        tools.push(mem.replace.clone());
        tools.push(mem.remove.clone());
    }
    if let Some(delegate) = &shed.delegate {
        tools.push(delegate.start.clone());
        tools.push(delegate.check.clone());
        tools.push(delegate.get.clone());
    }
    if let Some(bash) = &shed.bash {
        tools.push(bash.clone());
    }
    if let Some(others) = &shed.others {
        tools.extend(others.iter().cloned());
    }
    tools
}
```

## Provider Classification

### Path A (native system + native tools)

- `anthropic_messages_provider.rs` — has `system` field + native tools array
- `openai_provider.rs` — has system-role messages + native tools array
- `openai_responses_provider.rs` — has `instructions` field + native tools array

### Path B (single text prompt)

- `llamacpp.rs` — uses `apply_chat_template()` for messages, system goes in as a system-role chat message
- `candle.rs` — builds a plain text prompt with `System:`, `User:`, `Assistant:` markers

### Delegates (no changes needed)

- `huggingface_gguf_provider.rs` — delegates to `llamacpp.rs`
- `huggingface_candle_provider.rs` — delegates to `candle.rs`

## Path A Design

### Anthropic

```rust
// System: combine system_prompt + soul
let system_content = match (&interaction.system_prompt, &interaction.soul) {
    (Some(sys), Some(soul)) => Some(format!("{sys}\n\n{soul}")),
    (Some(sys), None) => Some(sys.clone()),
    (None, Some(soul)) => Some(soul.clone()),
    (None, None) => None,
};
let system = system_content.map(|s| AnthropicSystemContent::Text(s));

// Tools: flatten ToolShed, format through AnthropicFormatter
let tools = interaction.tools_shed.as_ref().map(|s| {
    let all_tools = flatten_tools(s);
    AnthropicFormatter.format_tools(&all_tools)
});

// Messages: unchanged (already maps Vec<Messages>)
```

### OpenAI (Completions)

```rust
// System: combine system_prompt + soul into system message(s)
let mut messages = Vec::new();
match (&interaction.system_prompt, &interaction.soul) {
    (Some(sys), Some(soul)) => {
        messages.push(OpenAIMessage {
            role: "system".into(),
            content: Some(OpenAIMessageContent::Text(format!("{sys}\n\n{soul}"))),
            ..
        });
    }
    (Some(sys), None) => {
        messages.push(OpenAIMessage {
            role: "system".into(),
            content: Some(OpenAIMessageContent::Text(sys.clone())),
            ..
        });
    }
    (None, Some(soul)) => {
        messages.push(OpenAIMessage {
            role: "system".into(),
            content: Some(OpenAIMessageContent::Text(soul.clone())),
            ..
        });
    }
    (None, None) => {}
}

// Tools: flatten ToolShed, format through OpenAI formatter
let tools = interaction.tools_shed.as_ref().map(|s| {
    let all_tools = flatten_tools(s);
    OpenAIFunctionFormatter.format_tools(&all_tools)
});

// Messages: unchanged (already maps Vec<Messages>)
```

### OpenAI Responses

```rust
// Instructions: combine system_prompt + soul
let instructions = match (&interaction.system_prompt, &interaction.soul) {
    (Some(sys), Some(soul)) => Some(format!("{sys}\n\n{soul}")),
    (Some(sys), None) => Some(sys.clone()),
    (None, Some(soul)) => Some(soul.clone()),
    (None, None) => None,
};

// Tools: flatten ToolShed, format through Responses formatter
let tools = interaction.tools_shed.as_ref().map(|s| {
    let all_tools = flatten_tools(s);
    ResponsesFormatter.format_tools(&all_tools)
});

// Input: unchanged (already maps Vec<Messages>)
```

## Path B Design

### llama.cpp

llama.cpp uses `apply_chat_template()` which takes a list of `{role, content}` messages. The system portion is injected as a system-role message, and tool instructions are injected via `ToolFormatter.tool_calling_instructions()` as part of the system message:

```rust
let mut chat_messages: Vec<LlamaChatMessage> = Vec::new();

// Build system message from system_prompt + soul + tool instructions
let system_parts: Vec<String> = interaction.system_prompt.iter()
    .cloned()
    .chain(interaction.soul.iter().cloned())
    .chain(Self::Formatter::default().tool_calling_instructions().into_iter())
    .collect();

if !system_parts.is_empty() {
    chat_messages.push(LlamaChatMessage::new(
        "system".into(),
        system_parts.join("\n\n"),
    ));
}

// Tool definitions: formatted as system message via format_tools
if let Some(shed) = &interaction.tools_shed {
    let all_tools = flatten_tools(shed);
    let tool_defs = Self::Formatter::default().format_tools(&all_tools)?;
    chat_messages.push(LlamaChatMessage::new(
        "system".into(),
        format!("Available tools:\n{tool_defs}"),
    ));
}

// Messages: existing apply_chat_template flow unchanged
for msg in &interaction.messages {
    // ... existing message mapping
}

// Apply template
let prompt = model.apply_chat_template(&template, &chat_messages, true)?;
```

### Candle

Candle builds a flat text prompt. All sections use `System:`, `User:`, `Assistant:` markers:

```rust
let mut parts = Vec::new();

// System prompt
if let Some(sys) = &interaction.system_prompt {
    parts.push(format!("System: {sys}"));
}

// Soul
if let Some(soul) = &interaction.soul {
    parts.push(format!("Soul: {soul}"));
}

// Tool instructions
if let Some(instructions) = Self::Formatter::default().tool_calling_instructions() {
    parts.push(format!("System: {instructions}"));
}

// Tool definitions
if let Some(shed) = &interaction.tools_shed {
    let all_tools = flatten_tools(shed);
    let tool_defs = Self::Formatter::default().format_tools(&all_tools)?;
    parts.push(format!("Tools:\n{tool_defs}"));
}

// Messages
for msg in &interaction.messages {
    match msg {
        Messages::User { content, .. } => { /* "User: {text}" */ }
        Messages::Assistant { content, .. } => { /* "Assistant: {text}" */ }
        Messages::ToolResult { content, .. } => { /* "Tool: {text}" */ }
    }
}

// Trailing prompt to trigger generation
parts.push("Assistant:".to_string());
parts.join("\n")
```

## Current State by Provider

### `anthropic_messages_provider.rs`
- **system_prompt**: Mapped to Anthropic's `system` field
- **soul**: Not handled
- **tools_shed**: Not handled
- **messages**: Mapped to `AnthropicMessage[]`
- **broken**: References `interaction.tools` which no longer exists

### `openai_provider.rs`
- **system_prompt**: Inserted as system-role message
- **soul**: Not handled
- **tools_shed**: Not handled
- **messages**: Mapped to OpenAI message format
- **broken**: References `interaction.tools` which no longer exists

### `openai_responses_provider.rs`
- **system_prompt**: Mapped to `instructions` field
- **soul**: Not handled
- **tools_shed**: Not handled
- **messages**: Mapped to `ResponseInputItem[]`
- **broken**: No tool handling (never had `interaction.tools`)

### `llamacpp.rs`
- **system_prompt**: Raw prompt or in chat template
- **soul**: Not handled
- **tools_shed**: Not handled
- **messages**: Through `apply_chat_template()`
- **broken**: No tool handling

### `candle.rs`
- **system_prompt**: `System: {text}` prefix
- **soul**: Not handled
- **tools_shed**: Not handled
- **messages**: `User:/Assistant:` format
- **broken**: No tool handling

## File

Each provider file in `backends/foundation_ai/src/backends/`:
1. `anthropic_messages_provider.rs`
2. `openai_provider.rs`
3. `openai_responses_provider.rs`
4. `llamacpp.rs`
5. `candle.rs`

(The huggingface providers delegate to llama.cpp and Candle respectively.)
