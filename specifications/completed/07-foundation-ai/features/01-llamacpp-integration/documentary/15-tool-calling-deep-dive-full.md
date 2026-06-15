# 15 - Tool Calling Deep Dive: From Examples to Production Implementation

## Overview

This document provides a comprehensive analysis of tool calling patterns extracted from llama.cpp examples, server implementation, and test cases. We trace the complete data flow from tool definition through grammar-constrained generation to structured output parsing.

**Source Files Analyzed**:
- `examples/simple-chat/simple-chat.cpp` - Basic chat template usage
- `tools/server/server.cpp` - Production OpenAI-compatible API
- `tools/server/server-context.cpp` - Request handling with tools
- `tools/server/server-tools.h/cpp` - Built-in tool implementations
- `common/chat.h/cpp` - Chat template abstraction layer
- `common/json-schema-to-grammar.h/cpp` - Schema to GBNF conversion
- `tests/test-chat-template.cpp` - Tool calling test cases
- `tests/test-json-schema-to-grammar.cpp` - Grammar generation tests
- `examples/pydantic_models_to_grammar.py` - Python reference implementation

---

## 1. Architecture Overview

### 1.1 Three-Layer System

Tool calling in llama.cpp operates through three cooperating layers:

1. **Application Layer** - Tool definitions, tool choice, parallel call configuration
2. **Template Layer** - Jinja2 template application, model-specific formatting
3. **Grammar Layer** - JSON Schema to GBNF conversion, grammar-constrained sampling
4. **Generation Layer** - Token sampling with constraints, PEG parsing output extraction

### 1.2 Data Flow

```
Client Request (OpenAI format)
    ↓
Parse tools[] and messages[]
    ↓
Apply chat template → formatted prompt + grammar triggers
    ↓
Generate GBNF grammar from JSON schemas
    ↓
Sample tokens constrained by grammar
    ↓
Parse generated text → tool_calls array
    ↓
Return OpenAI-compatible response
```

---

## 2. Core Data Structures

### 2.1 Tool Definition

```cpp
// From common/chat.h
struct common_chat_tool {
    std::string name;        // Function name (e.g., "get_weather")
    std::string description; // What the tool does
    std::string parameters;  // JSON schema as string
};
```

### 2.2 Tool Call Output

```cpp
struct common_chat_tool_call {
    std::string name;         // Which tool was called
    std::string arguments;    // JSON string of arguments
    std::string id;           // Unique identifier (call_abc123)
};
```

### 2.3 Chat Message with Tool Support

```cpp
struct common_chat_msg {
    std::string role;         // "system", "user", "assistant", "tool"
    std::string content;      // Text content
    std::vector<common_chat_tool_call> tool_calls;  // For assistant messages
    std::string tool_call_id; // For tool result messages
    std::string reasoning_content; // Chain-of-thought
};
```

### 2.4 Tool Choice

```cpp
enum common_chat_tool_choice {
    COMMON_CHAT_TOOL_CHOICE_AUTO,     // Model decides
    COMMON_CHAT_TOOL_CHOICE_REQUIRED, // Must call a tool
    COMMON_CHAT_TOOL_CHOICE_NONE,     // No tool calling
};
```

---

## 3. Model-Specific Tool Formats

Different model families use different tool call formats:

| Model Family | Format | Special Tokens |
|--------------|--------|----------------|
| **Llama 3.1/3.2** | `{"name": "...", "parameters": {...}}` | `<|python_tag|>` prefix |
| **Hermes 2/3 Pro** | `## 4. Grammar-Constrained Generation

### 4.1 GBNF Grammar System

GBNF (Generalized Backus-Naur Form) is llama.cpp's grammar specification language that constrains token-by-token generation.

```cpp
// Create grammar-constrained sampler
llama_sampler * grammar_sampler = llama_sampler_init_grammar(
    vocab,         // Vocabulary for token mapping
    grammar_str,   // GBNF grammar string
    grammar_root   // Root rule name (typically "root")
);

// Add to sampler chain BEFORE temperature/distribution
llama_sampler_chain_add(chain, grammar_sampler);
llama_sampler_chain_add(chain, llama_sampler_init_temp(0.8));
```

### 4.2 Grammar Types

```cpp
enum common_grammar_type {
    COMMON_GRAMMAR_TYPE_NONE,          // No grammar constraint
    COMMON_GRAMMAR_TYPE_USER,          // User-provided GBNF string
    COMMON_GRAMMAR_TYPE_OUTPUT_FORMAT, // Auto-generated from JSON schema
    COMMON_GRAMMAR_TYPE_TOOL_CALLS,    // Auto-generated from tool definitions
};
```

### 4.3 Lazy Grammar Activation

For models that produce free-form text before tool calls:

```cpp
llama_sampler * lazy_grammar = llama_sampler_init_grammar_lazy_patterns(
    vocab,
    grammar_str,
    grammar_root,
    trigger_patterns,   // Regex patterns that activate grammar
    n_trigger_patterns,
    trigger_tokens,     // Token IDs that activate grammar
    n_trigger_tokens
);
```

---

## 5. JSON Schema to GBNF Conversion

### 5.1 C++ Implementation

```cpp
// From common/json-schema-to-grammar.h
std::string json_schema_to_grammar(const nlohmann::ordered_json & schema);

// Key classes:
// - common_schema_converter: Main conversion logic
// - common_schema_info: Probes schema type/constraints
// - common_grammar_builder: Registers rules and composes grammar
```

### 5.2 Supported JSON Schema Features

| Feature | Example | GBNF Output |
|---------|---------|-------------|
| String | `{"type": "string"}` | String rule with escaping |
| Integer | `{"type": "integer"}` | `"- "? [0-9]+` |
| Number | `{"type": "number"}` | Full float grammar |
| Boolean | `{"type": "boolean"}` | `"true" \| "false"` |
| Enum | `{"enum": ["a", "b"]}` | `"\"a\"" \| "\"b\""` |
| Array | `{"type": "array", "items": ...}` | Repetition with separator |
| Object | `{"type": "object", "properties": ...}` | Property rules |
| min/max | `{"minimum": 0, "maximum": 100}` | Digit-level constraints |
| pattern | `{"pattern": "^[a-z]+$"}` | Regex to grammar |
| oneOf/anyOf | `{"oneOf": [...]}` | Alternation rules |

### 5.3 Example: Tool Schema to Grammar

```json
{
  "type": "function",
  "function": {
    "name": "get_weather",
    "parameters": {
      "type": "object",
      "properties": {
        "location": {"type": "string"},
        "unit": {"enum": ["celsius", "fahrenheit"]}
      },
      "required": ["location"]
    }
  }
}
```

Generates GBNF:
```gbnf
root ::= "{" ws "\"function\"" ws ":" ws function_def "}"
function_def ::= "{" ws "\"name\"" ws ":" ws string "," ws "\"parameters\"" ws ":" ws params "}"
params ::= "{" ws "\"location\"" ws ":" ws string ("," ws "\"unit\"" ws ":" ws unit)? "}"
unit ::= "\"celsius\"" | "\"fahrenheit\""
string ::= "\"" ([^"\\] | "\\" .)* "\""
ws ::= [ \t\n]*
```

---

## 6. Server Implementation (Production API)

### 6.1 OpenAI-Compatible Endpoint

```cpp
// From tools/server/server.cpp
// POST /v1/chat/completions

Request:
{
    "model": "gpt-3.5-turbo",
    "messages": [
        {"role": "system", "content": "You are helpful"},
        {"role": "user", "content": "Weather in Istanbul?"}
    ],
    "tools": [{
        "type": "function",
        "function": {
            "name": "get_weather",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }
        }
    }],
    "tool_choice": "auto"
}

Response:
{
    "choices": [{
        "finish_reason": "tool_calls",
        "message": {
            "role": "assistant",
            "tool_calls": [{
                "id": "call_abc123",
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "arguments": "{\"location\":\"Istanbul\"}"
                }
            }]
        }
    }]
}
```

### 6.2 Server Configuration

```bash
# Must use --jinja flag for tool calling support
./llama-server --jinja -fa \
    -hf bartowski/Qwen2.5-7B-Instruct-GGUF:Q4_K_M
```

---

## 7. Built-in Server Tools

The server exposes built-in tools for file operations:

```cpp
// From tools/server/server-tools.h
struct server_tool {
    virtual nlohmann::json get_definition() = 0;
    virtual nlohmann::json invoke(const nlohmann::json & params) = 0;
};

// Built-in tools:
// - read_file: Read file contents
// - file_glob_search: Search for files by pattern
// - grep_search: Search file contents
```

---

## 8. ReAct Agent Pattern

### 8.1 Reverse Prompt Implementation

The simplest tool-augmented generation uses reverse prompts:

```bash
./llama-cli $MODEL \
    -f ./prompts/reason-act.txt \
    -i --interactive-first \
    --top_k 10000 --temp 0.2 \
    -r "Question:" -r "Observation:" \
    --in-prefix " " -n -1
```

### 8.2 ReAct Prompt Structure

```
Answer the following questions as best you can. You have access to:

Search[entity] - searches Wikipedia
Lookup[string] - looks up in last article

Use format:
Question: input question
Thought: what to do
Action: action to take
Observation: result
... (repeat)
Thought: I know final answer
Final Answer: answer
```

---

## 9. Approach Comparison

### 9.1 Grammar-Constrained Generation (Recommended)

**How**: Define tool schemas → convert to GBNF → constrain sampling

**Pros**:
- Guarantees valid JSON (no parse failures)
- Works with any model
- Composable with other strategies
- No special training required

**Cons**:
- Slight slowdown from constraint
- Complex schemas → large grammars

### 9.2 Native Chat Template Tool Calling

**How**: Use model's built-in Jinja2 template

**Pros**:
- Highest quality (model trained on format)
- Native parallel tool call support
- Reasoning extraction (DeepSeek R1)

**Cons**:
- Only works with supported families
- Requires `--jinja` flag
- Template must match exactly

### 9.3 ReAct Agent Loop

**How**: Prompt + reverse prompts for tool execution

**Pros**:
- Simplest implementation
- Works with any model
- Explicit reasoning chain

**Cons**:
- Requires interactive mode
- No structured output guarantee
- Single tool per step

---

## 10. Implementation Roadmap for foundation_ai

### Phase 1: Grammar Sampler Integration
- [ ] Expose `llama_sampler_init_grammar()` in Rust bindings
- [ ] Support GBNF strings in `ModelParams`
- [ ] Implement JSON schema → GBNF conversion

### Phase 2: Tool Call API
- [ ] Add `tools: Vec<ToolDefinition>` to generation request
- [ ] Auto-generate grammar from tool schemas
- [ ] Parse tool calls from constrained output
- [ ] Return `ToolCall { name, arguments, id }`

### Phase 3: Template-Aware Tool Calling
- [ ] Detect model family from chat template
- [ ] Use native format handler when available
- [ ] Fall back to generic format + grammar

### Phase 4: Structured Output
- [ ] Support `response_format: {type: "json_schema"}`
- [ ] Convert arbitrary JSON schemas to GBNF
- [ ] Validate output against schema

---

## 11. Key Files for Reference

| File | Purpose |
|------|---------|
| `common/chat.h` | Core data structures |
| `common/chat.cpp` | Template application logic |
| `common/json-schema-to-grammar.h/cpp` | Schema conversion |
| `common/peg-parser.h/cpp` | Output parsing |
| `tools/server/server-context.cpp` | Request handling |
| `tests/test-chat-template.cpp` | Test cases |

---

## 12. Lessons from Examples Analysis

### From simple-chat/simple-chat.cpp:
- Basic chat template application pattern
- Incremental prompt formatting (don't re-tokenize history)
- Stateful KV cache across turns

### From parallel/parallel.cpp:
- System prompt sharing via KV cache copy
- Continuous batching for concurrent requests
- Per-client sampler state

### From speculative/speculative.cpp:
- Tree-based drafting concepts apply to parallel tool calls
- Stochastic acceptance preserves distribution

### From retrieval/retrieval.cpp:
- RAG pipeline: chunk → embed → search
- Cosine similarity for relevance

---

## 13. Foundation.ai Integration Notes

### Rust Binding Considerations:

1. **Sampler Chain**: Must expose grammar sampler before temperature/distribution
2. **Thread Safety**: Grammar samplers are not thread-safe - clone per request
3. **Memory Management**: Grammar strings must outlive sampler

### API Design:

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub enum ToolChoice {
    Auto,
    Required,
    None,
    Function { name: String },
}

pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub tool_choice: ToolChoice,
    pub parallel_tool_calls: bool,
}

pub struct ChatResponse {
    pub message: Message,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String, // "stop", "tool_calls", "length"
}
```

---

## 14. Common Pitfalls

1. **Forgetting `--jinja` flag**: Tool calling won't work without Jinja2 support
2. **Wrong sampler order**: Grammar must come BEFORE temperature in chain
3. **Schema validation**: JSON schema must be valid - test with jsonschema Python package
4. **Model compatibility**: Not all models support tool calling natively
5. **Grammar too large**: Complex schemas can produce unwieldy GBNF

---

## 15. Testing Strategy

1. **Unit tests**: JSON schema → GBNF conversion
2. **Integration tests**: End-to-end tool call with mock server
3. **Model tests**: Test with Llama 3.x, Hermes, Qwen families
4. **Edge cases**: Empty tools, invalid schema, parallel calls

---

_Created: 2026-04-07_
_Source: Direct analysis of llama.cpp source code, examples, tests, and server implementation_
