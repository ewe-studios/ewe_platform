# 13 - Tool Calling, Function Calling, and Structured Output in llama.cpp

## Overview

llama.cpp provides a comprehensive tool calling and structured output system that is OpenAI-compatible. This document covers every layer of the implementation: chat template-based tool formatting, grammar-constrained generation, JSON schema to GBNF conversion, and the ReAct agent pattern.

---

## 1. Architecture: How Tool Calling Works

Tool calling in llama.cpp is implemented through three cooperating systems:

```
Tool Definitions (JSON schema)
    ↓
Chat Template (Jinja2) — formats tools into model-specific prompt format
    ↓
Grammar Constraint (GBNF) — ensures model output is valid JSON matching tool schema
    ↓
Output Parser — extracts tool_call structs from generated text
```

### Data Flow

1. **Client** sends tool definitions + messages to `/v1/chat/completions`
2. **Server** applies Jinja2 chat template to format tools into the prompt
3. **Server** generates a GBNF grammar from tool schemas to constrain output
4. **Model** generates text constrained by grammar → valid JSON guaranteed
5. **Server** parses output to extract `tool_calls` array
6. **Client** receives OpenAI-compatible response with `finish_reason: "tool"`

---

## 2. Chat Template Tool Integration

### Core Data Structures (`common/chat.h`)

```cpp
// Tool definition (matches OpenAI format)
struct common_chat_tool {
    std::string name;
    std::string description;
    std::string parameters;  // JSON schema string
};

// Tool call result from generation
struct common_chat_tool_call {
    std::string name;
    std::string arguments;  // JSON string
    std::string id;         // Optional unique identifier
};

// Chat message with tool support
struct common_chat_msg {
    std::string role;                              // "system", "user", "assistant", "tool"
    std::string content;
    std::vector<common_chat_tool_call> tool_calls; // For assistant messages
    std::string tool_call_id;                      // For tool result messages
    std::string reasoning_content;                 // For chain-of-thought models
    std::vector<common_chat_msg_content_part> content_parts; // Multipart content
};

// Tool choice controls
enum common_chat_tool_choice {
    COMMON_CHAT_TOOL_CHOICE_AUTO,     // Model decides
    COMMON_CHAT_TOOL_CHOICE_REQUIRED, // Must call a tool
    COMMON_CHAT_TOOL_CHOICE_NONE,     // No tool calling
};
```

### Template Application with Tools

```cpp
struct common_chat_templates_inputs {
    std::vector<common_chat_msg> messages;
    std::vector<common_chat_tool> tools;           // Tool definitions
    common_chat_tool_choice tool_choice;            // auto/required/none
    bool parallel_tool_calls;                       // Allow multiple simultaneous calls
    std::string json_schema;                        // For structured output (non-tool)
    std::string grammar;                            // User-provided GBNF grammar
    bool add_generation_prompt;
};

// Apply template to produce formatted prompt + grammar constraint
common_chat_params result = common_chat_templates_apply(
    templates,  // Loaded from model or override
    inputs      // Messages + tools + settings
);

// Result contains:
// result.prompt       — Formatted prompt string
// result.grammar      — GBNF grammar constraining output
// result.grammar_lazy — Whether to delay grammar activation
// result.format       — Which format handler to use
```

### Grammar Types

```cpp
enum common_grammar_type {
    COMMON_GRAMMAR_TYPE_NONE,          // No grammar constraint
    COMMON_GRAMMAR_TYPE_USER,          // User-provided GBNF string
    COMMON_GRAMMAR_TYPE_OUTPUT_FORMAT, // Auto-generated from JSON schema
    COMMON_GRAMMAR_TYPE_TOOL_CALLS,    // Auto-generated from tool definitions
};
```

---

## 3. Native Format Handlers

llama.cpp recognizes specific model chat templates and uses optimized format handlers:

### Supported Native Formats

| Format | Models | Tool Call Format |
|--------|--------|-----------------|
| **Llama 3.x** | Llama 3.1, 3.2, 3.3 | `{"name": "...", "parameters": {...}}` with `<\|python_tag\|>` |
| **Hermes 2 Pro** | Hermes 2/3, Qwen 2.5, many fine-tunes | `<tool_call>{"name": "...", "arguments": {...}}</tool_call>` |
| **Functionary v3.1/v3.2** | MeetKai Functionary models | Function name as special token + JSON args |
| **Mistral Nemo** | Mistral Nemo, Mistral 7B v0.3+ | `[TOOL_CALLS]` prefix + JSON array |
| **FireFunction v2** | Fireworks AI models | Custom function format |
| **Command R7B** | Cohere Command R models | With reasoning extraction |
| **DeepSeek R1** | DeepSeek R1 and distills | With `<think>` reasoning extraction |
| **Generic** | All other models | JSON-constrained output |

### Llama 3.1 Built-in Tools

Llama 3.1+ models have special support for built-in tools:
- `wolfram_alpha` — Math computation
- `web_search` / `brave_search` — Web search
- `code_interpreter` — Code execution

These use the `<|python_tag|>` special token and have dedicated prompt formatting.

### Generic Format Fallback

When no native handler is recognized, the generic format:
1. Injects tool definitions into the system prompt
2. Generates a GBNF grammar that constrains output to valid tool call JSON
3. Parses the constrained output to extract tool calls

---

## 4. GBNF Grammar System

### What is GBNF?

GBNF (Generalized Backus-Naur Form) is llama.cpp's grammar specification language. It constrains token-by-token generation to match a formal grammar — guaranteeing structurally valid output.

### Grammar Sampler Integration

```cpp
// Create a grammar-constrained sampler
llama_sampler * grammar_sampler = llama_sampler_init_grammar(
    vocab,         // Vocabulary for token mapping
    grammar_str,   // GBNF grammar string
    grammar_root   // Root rule name (typically "root")
);

// Add to sampler chain BEFORE temperature/distribution
llama_sampler_chain_add(chain, grammar_sampler);
llama_sampler_chain_add(chain, llama_sampler_init_temp(0.8));
llama_sampler_chain_add(chain, llama_sampler_init_dist(seed));
```

The grammar sampler masks out tokens that would violate the grammar at each position, ensuring only valid continuations are sampled.

### Lazy Grammar Activation

```cpp
// Grammar can be activated lazily — only start constraining after a trigger pattern
llama_sampler * lazy_grammar = llama_sampler_init_grammar_lazy_patterns(
    vocab,
    grammar_str,
    grammar_root,
    trigger_patterns,   // Regex patterns that activate the grammar
    n_trigger_patterns,
    trigger_tokens,     // Token IDs that activate the grammar
    n_trigger_tokens
);
```

This is used for models that produce free-form text before the tool call (e.g., reasoning before `<tool_call>`).

### GBNF Syntax Examples

```gbnf
# Simple JSON object
root ::= "{" ws "\"name\"" ws ":" ws string "," ws "\"arguments\"" ws ":" ws object "}" ws

# String with escaping
string ::= "\"" ([^"\\] | "\\" ["\\/bfnrt] | "\\u" [0-9a-fA-F]{4})* "\""

# Number
number ::= "-"? ("0" | [1-9] [0-9]*) ("." [0-9]+)? ([eE] [+-]? [0-9]+)?

# Whitespace
ws ::= [ \t\n]*

# Array
array ::= "[" ws (value ("," ws value)*)? "]" ws

# Object
object ::= "{" ws (string ":" ws value ("," ws string ":" ws value)*)? "}" ws
```

---

## 5. JSON Schema to GBNF Conversion

### C++ Implementation (`common/json-schema-to-grammar.h/cpp`)

```cpp
// Convert JSON schema to GBNF grammar string
std::string json_schema_to_grammar(const nlohmann::json & schema);

// Key classes:
// common_schema_converter — Main conversion logic
// common_schema_info     — Probes schema type/constraints
// common_grammar_builder — Registers rules and composes grammar
// gbnf_format_literal()  — Escapes special chars for grammar literals
```

### Supported JSON Schema Features

| Feature | Example | GBNF Output |
|---------|---------|-------------|
| String | `{"type": "string"}` | `string` rule with escaping |
| Integer | `{"type": "integer"}` | `"-"? [0-9]+` |
| Number | `{"type": "number"}` | Full float grammar |
| Boolean | `{"type": "boolean"}` | `"true" \| "false"` |
| Null | `{"type": "null"}` | `"null"` |
| Enum | `{"enum": ["a", "b"]}` | `"\"a\"" \| "\"b\""` |
| Array | `{"type": "array", "items": ...}` | Repetition with separator |
| Object | `{"type": "object", "properties": ...}` | Property rules with required/optional |
| min/max | `{"minimum": 0, "maximum": 100}` | Digit-level range constraints |
| pattern | `{"pattern": "^[a-z]+$"}` | Regex to grammar conversion |
| format | `{"format": "date-time"}` | ISO 8601 grammar |
| oneOf/anyOf | `{"oneOf": [...]}` | Alternation rules |
| $ref | `{"$ref": "#/defs/Foo"}` | Recursive rule references |

### Python Implementation (`examples/json_schema_to_grammar.py`)

The Python version (10,000+ lines) is more feature-complete and serves as the reference:

```python
# Key functions:
def json_schema_to_grammar(schema, prop_order=None, allow_fetch=False, 
                            dotall=False, raw_pattern=None):
    """Convert JSON schema to GBNF grammar string."""

# Handles:
# - Complex min/max integer ranges with digit-level constraints
# - String format validators (date, time, uuid, regex patterns)
# - Recursive schema references with cycle detection
# - Configurable whitespace handling
# - Array repetition with min/max items
```

---

## 6. Pydantic-Based Tool Definition (`examples/pydantic_models_to_grammar.py`)

### Overview

This 16,000+ line Python module converts Pydantic `BaseModel` classes directly into GBNF grammars for constrained generation. It supports:

### Basic Tool Definition

```python
from pydantic import BaseModel
from pydantic_models_to_grammar import generate_gbnf_grammar_and_documentation

class GetWeather(BaseModel):
    """Get the current weather in a given location."""
    location: str
    unit: Literal["celsius", "fahrenheit"] = "celsius"

# Generate grammar that constrains output to valid GetWeather JSON
grammar, docs = generate_gbnf_grammar_and_documentation(
    pydantic_model_list=[GetWeather],
    outer_object_name="function",
    outer_object_content="function_parameters",
    model_prefix="Function",
    list_of_outputs=False
)
```

### Concurrent Tool Calling

```python
# Multiple tools — model picks which to call
class SendMessage(BaseModel):
    message: str

class SearchWeb(BaseModel):
    query: str

grammar, docs = generate_gbnf_grammar_and_documentation(
    pydantic_model_list=[SendMessage, SearchWeb],
    list_of_outputs=True  # Allow array of tool calls
)
# Output grammar accepts: [{"function": "SendMessage", ...}, {"function": "SearchWeb", ...}]
```

### Supported Python Types → GBNF

| Python Type | GBNF Rule |
|-------------|-----------|
| `str` | String with escaping |
| `int` | Integer |
| `float` | Number |
| `bool` | Boolean |
| `Optional[T]` | `T \| "null"` |
| `List[T]` | Array of T |
| `Dict[str, T]` | Object with T values |
| `Literal["a", "b"]` | Enum alternation |
| `Enum` | Enum values |
| `Union[A, B]` | `A \| B` |
| Nested `BaseModel` | Recursive object rule |

### Dynamic Model from OpenAI Format

```python
from pydantic_models_to_grammar import convert_dictionary_to_pydantic_model

# Convert OpenAI tool definition to Pydantic model
openai_tool = {
    "type": "function",
    "function": {
        "name": "get_weather",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {"type": "string", "description": "City name"}
            },
            "required": ["location"]
        }
    }
}

PydanticModel = convert_dictionary_to_pydantic_model(openai_tool)
```

---

## 7. Server Tool Calling Implementation

### Endpoint: `POST /v1/chat/completions`

```json
{
    "model": "gpt-3.5-turbo",
    "messages": [
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "What is the weather in Istanbul?"}
    ],
    "tools": [
        {
            "type": "function",
            "function": {
                "name": "get_current_weather",
                "description": "Get the current weather in a given location",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city and country, e.g. San Francisco, CA"
                        }
                    },
                    "required": ["location"]
                }
            }
        }
    ],
    "tool_choice": "auto"
}
```

### Response Format

```json
{
    "choices": [{
        "finish_reason": "tool",
        "index": 0,
        "message": {
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_abc123",
                "type": "function",
                "function": {
                    "name": "get_current_weather",
                    "arguments": "{\"location\":\"Istanbul, Turkey\"}"
                }
            }]
        }
    }]
}
```

### Tool Result Messages

After executing the tool, send the result back:

```json
{
    "messages": [
        {"role": "user", "content": "What is the weather in Istanbul?"},
        {"role": "assistant", "content": null, "tool_calls": [...]},
        {"role": "tool", "tool_call_id": "call_abc123", "content": "{\"temperature\": 22, \"unit\": \"celsius\"}"},
    ]
}
```

### Server Configuration

```bash
# Must use --jinja flag for tool calling support
llama-server --jinja -fa -hf bartowski/Qwen2.5-7B-Instruct-GGUF:Q4_K_M

# With custom template override:
llama-server --jinja -fa -hf model.gguf \
    --chat-template-file models/templates/custom.jinja
```

### Parallel Tool Calls

```json
{
    "parallel_tool_calls": true,
    "messages": [...],
    "tools": [...]
}
```

When enabled, the model can generate multiple tool calls in a single response. The grammar is adjusted to accept an array of tool call objects.

---

## 8. Built-in Server Tools (`tools/server/server-tools.h`)

The server can also expose built-in tools:

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents |
| `file_glob_search` | Search for files by pattern |
| `grep_search` | Search file contents |

Each tool implements:
```cpp
struct server_tool {
    virtual nlohmann::json get_definition() = 0;
    virtual nlohmann::json invoke(const nlohmann::json & params) = 0;
};
```

---

## 9. ReAct Agent Pattern (`examples/reason-act.sh`)

### Implementation

The simplest form of tool-augmented generation uses reverse prompts:

```bash
./llama-cli $MODEL --color \
    -f ./prompts/reason-act.txt \       # ReAct prompt template
    -i --interactive-first \             # Interactive mode
    --top_k 10000 --temp 0.2 \          # Low temp for reasoning
    -r "Question:" -r "Observation:" \   # Reverse prompts
    --in-prefix " " -n -1
```

**How it works**:
1. Model generates "Thought: I should search for..." + "Action: search[query]"
2. Model outputs "Observation:" → generation stops (reverse prompt triggered)
3. External system executes the action, provides observation text
4. User inputs observation, model continues reasoning
5. Repeat until "Final Answer:" is generated

### ReAct Prompt Structure

```
Answer the following questions as best you can. You have access to the following tools:

Search[entity] - searches for an entity on Wikipedia
Lookup[string] - looks up a string in the last Wikipedia article

Use the following format:
Question: the input question
Thought: you should think about what to do
Action: the action to take
Observation: the result of the action
... (repeat as needed)
Thought: I now know the final answer
Final Answer: the final answer
```

---

## 10. Approach Comparison for Tool Calling

### Approach 1: Grammar-Constrained Generation (Recommended)

**How**: Define tool schemas → convert to GBNF grammar → constrain sampling

**Pros**:
- Guarantees valid JSON output (no parsing failures)
- Works with any model (including non-chat models)
- Composable with other sampling strategies
- No special model training required

**Cons**:
- Grammar constraint can slow generation slightly
- Complex schemas produce large grammars
- Model must still understand the semantic intent

### Approach 2: Native Chat Template Tool Calling

**How**: Use model's built-in Jinja2 template with tool definitions

**Pros**:
- Model trained on this exact format → highest quality
- Template handles formatting perfectly
- Native support for parallel tool calls
- Reasoning extraction (DeepSeek R1, Command R7B)

**Cons**:
- Only works with supported model families
- Template must match model's training format exactly
- Requires `--jinja` flag and template library

### Approach 3: ReAct Agent Loop

**How**: Prompt model with ReAct format, use reverse prompts to pause for tool execution

**Pros**:
- Simplest implementation
- Works with any model
- Explicit reasoning chain visible

**Cons**:
- Requires interactive mode
- No structured output guarantee
- Parsing action/observation requires regex
- Single tool call per step

### Recommended Strategy for foundation_ai

1. **Primary**: Grammar-constrained tool calling for OpenAI-compatible API
2. **Native templates**: Detect model family, use native format when available
3. **Fallback**: Generic format with grammar constraint
4. **Agent pattern**: ReAct as an application-layer pattern built on top of the API

---

## 11. Implementation Roadmap for foundation_ai

### Phase 1: Grammar Sampler Integration
- Expose `llama_sampler_init_grammar()` in Rust bindings
- Support GBNF grammar strings in `ModelParams`
- Implement JSON schema → GBNF conversion in Rust (port from C++)

### Phase 2: Tool Call API
- Add `tools: Vec<ToolDefinition>` to generation request
- Auto-generate grammar from tool schemas
- Parse tool calls from constrained output
- Return `ToolCall { name, arguments, id }` in response

### Phase 3: Template-Aware Tool Calling
- Detect model family from chat template metadata
- Use native format handler when available
- Fall back to generic format with grammar constraint

### Phase 4: Structured Output
- Support `response_format: { type: "json_schema", json_schema: {...} }`
- Convert arbitrary JSON schemas to GBNF
- Validate output against schema

---

_Created: 2026-04-07_
_Source: llama.cpp docs/function-calling.md, common/chat.h, examples/*.py, tools/server/_
