# 17 - Anthropic Messages API Implementation: Claude Code Compatibility Deep Dive

## Overview

This document provides a comprehensive analysis of llama.cpp's Anthropic Messages API implementation, with specific focus on compatibility with Claude Code. We examine the complete request/response lifecycle, tool calling normalization, streaming event formats, and content block handling that enables seamless integration with Anthropic-compatible clients.

**Source Files Analyzed**:
- `tools/server/server.cpp` - HTTP route registration for `/v1/messages`
- `tools/server/server-context.cpp` - Request handling, `convert_anthropic_to_oai()`, streaming response
- `tools/server/server-task.h` - Task result structures, `TASK_RESPONSE_TYPE_ANTHROPIC`
- `tools/server/server-task.cpp` - `to_json_anthropic()`, `to_json_anthropic_stream()` implementations
- `tools/server/server-common.cpp` - API conversion utilities
- `common/chat.h` - Chat message structures, tool call definitions

---

## 1. Architecture Overview

### 1.1 Request Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Claude Code Client                           │
│  - Sends Anthropic Messages API format                          │
│  - Expects Anthropic streaming events                           │
│  - Tool calls in Anthropic format                               │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              │ POST /v1/messages
                              │ Content-Type: application/json
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              llama.cpp HTTP Server Layer                        │
│  ┌────────────────────────────────────────────────────────────┐│
│  │ ctx_http.post("/v1/messages", ex_wrapper(                  ││
│  │     routes.post_anthropic_messages))                       ││
│  └────────────────────────────────────────────────────────────┘│
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Anthropic Messages Handler                         │
│                                                                 │
│  1. Parse request body                                          │
│  2. convert_anthropic_to_oai() ← Key normalization step         │
│  3. oaicompat_chat_params_parse()                               │
│  4. handle_completions_impl(..., TASK_RESPONSE_TYPE_ANTHROPIC) │
│                                                                 │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Task Queue + Slot Scheduler                        │
│  - Internal processing uses OpenAI format                       │
│  - Response type flag preserves Anthropic output formatting     │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              Response Formatting                                │
│  ┌────────────────────────────────────────────────────────────┐│
│  │ switch (res_type) {                                        ││
│  │   case TASK_RESPONSE_TYPE_ANTHROPIC:                       ││
│  │     return to_json_anthropic() / to_json_anthropic_stream()││
│  │ }                                                          ││
│  └────────────────────────────────────────────────────────────┘│
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              │ SSE Stream (Anthropic format)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Claude Code Client                           │
│  - message_start                                                 │
│  - content_block_start/delta/stop (repeating)                   │
│  - message_delta                                                 │
│  - message_stop                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 Key Design Decision: Internal Normalization

The llama.cpp server uses a **normalization pattern**:

1. **Incoming Anthropic requests** → Convert to internal OpenAI format
2. **Internal processing** → Uses OpenAI structures throughout
3. **Outgoing responses** → Convert back to Anthropic format based on `res_type`

This allows:
- Single code path for inference
- Multiple API compatibility layers
- Easy addition of new API formats

---

## 2. Request Conversion: Anthropic → OpenAI

### 2.1 Full Conversion Implementation

**Source**: `server-common.cpp` lines 1436-1656

```cpp
json convert_anthropic_to_oai(const json & body) {
    json oai_body;
    
    // === 1. SYSTEM PROMPT CONVERSION ===
    json oai_messages = json::array();
    auto system_param = json_value(body, "system", json());
    
    if (!system_param.is_null()) {
        std::string system_content;
        
        // Handle string system prompt
        if (system_param.is_string()) {
            system_content = system_param;
        } 
        // Handle block array system prompt (Anthropic extended format)
        else if (system_param.is_array()) {
            for (const auto & block : system_param) {
                if (json_value(block, "type", std::string()) == "text") {
                    system_content += json_value(block, "text", std::string());
                }
                // Note: thinking blocks in system prompt could be added here
            }
        }
        
        oai_messages.push_back({
            {"role", "system"},
            {"content", system_content}
        });
    }
    
    // === 2. MESSAGES ARRAY CONVERSION ===
    if (!body.contains("messages")) {
        throw std::runtime_error("'messages' is required");
    }
    
    const json & messages = body.at("messages");
    
    for (const auto & msg : messages) {
        std::string role = json_value(msg, "role", std::string());
        const json & content = msg.at("content");
        
        json tool_calls = json::array();
        json converted_content = json::array();
        json tool_results = json::array();
        std::string reasoning_content;
        
        // Process content blocks
        for (const auto & block : content) {
            std::string type = json_value(block, "type", std::string());
            
            if (type == "text") {
                // Direct pass-through for text blocks
                converted_content.push_back(block);
            } 
            else if (type == "thinking") {
                // Extract thinking/reasoning blocks (Anthropic extended thinking)
                // Maps to OpenAI 'reasoning_content' extension
                reasoning_content += json_value(block, "thinking", std::string());
            }
            else if (type == "image") {
                // Convert Anthropic image format to OpenAI image_url format
                json source = json_value(block, "source", json::object());
                std::string source_type = json_value(source, "type", std::string(""));
                
                if (source_type == "base64") {
                    std::string media_type = json_value(
                        source, "media_type", std::string("image/jpeg"));
                    std::string data = json_value(source, "data", std::string(""));
                    
                    // Build data: URL for OpenAI format
                    std::ostringstream ss;
                    ss << "data:" << media_type << ";base64," << data;
                    
                    converted_content.push_back({
                        {"type", "image_url"},
                        {"image_url", {{"url", ss.str()}}}
                    });
                } 
                else if (source_type == "url") {
                    std::string url = json_value(source, "url", std::string(""));
                    converted_content.push_back({
                        {"type", "image_url"},
                        {"image_url", {{"url", url}}}
                    });
                }
            }
            else if (type == "tool_use") {
                // CRITICAL: Convert Anthropic tool_use to OpenAI tool_calls format
                tool_calls.push_back({
                    {"id", json_value(block, "id", std::string())},
                    {"type", "function"},
                    {"function", {
                        {"name", json_value(block, "name", std::string())},
                        // Note: Anthropic uses 'input', OpenAI uses 'arguments'
                        {"arguments", json_value(block, "input", json::object()).dump()}
                    }}
                });
            }
            else if (type == "tool_result") {
                // Convert tool_result to separate tool role message
                std::string tool_use_id = json_value(block, "tool_use_id", std::string(""));
                
                auto result_content = json_value(block, "content", json());
                std::string result_text;
                
                if (result_content.is_string()) {
                    result_text = result_content;
                } else if (result_content.is_array()) {
                    for (const auto & c : result_content) {
                        if (json_value(c, "type", std::string()) == "text") {
                            result_text += json_value(c, "text", std::string());
                        }
                    }
                }
                
                tool_results.push_back({
                    {"role", "tool"},
                    {"tool_call_id", tool_use_id},
                    {"content", result_text}
                });
            }
        }
        
        // Build converted message
        if (!converted_content.empty() || !tool_calls.empty() || !reasoning_content.empty()) {
            json new_msg = {{"role", role}};
            
            if (!converted_content.empty()) {
                new_msg["content"] = converted_content;
            } else if (!tool_calls.empty() || !reasoning_content.empty()) {
                // Empty content required for tool-only or reasoning-only messages
                new_msg["content"] = "";
            }
            
            if (!tool_calls.empty()) {
                new_msg["tool_calls"] = tool_calls;
            }
            
            if (!reasoning_content.empty()) {
                // Custom extension for reasoning/thinking content
                new_msg["reasoning_content"] = reasoning_content;
            }
            
            oai_messages.push_back(new_msg);
        }
        
        // Append tool result messages (separate from assistant message)
        for (const auto & tool_msg : tool_results) {
            oai_messages.push_back(tool_msg);
        }
    }
    
    oai_body["messages"] = oai_messages;
    
    // === 3. TOOLS ARRAY CONVERSION ===
    if (body.contains("tools")) {
        const json & tools = body.at("tools");
        if (tools.is_array()) {
            json oai_tools = json::array();
            
            for (const auto & tool : tools) {
                oai_tools.push_back({
                    {"type", "function"},
                    {"function", {
                        {"name", json_value(tool, "name", std::string())},
                        {"description", json_value(tool, "description", std::string())},
                        // Anthropic: 'input_schema' → OpenAI: 'parameters'
                        {"parameters", tool.contains("input_schema") 
                            ? tool.at("input_schema") 
                            : json::object()}
                    }}
                });
            }
            oai_body["tools"] = oai_tools;
        }
    }
    
    // === 4. TOOL_CHOICE CONVERSION ===
    if (body.contains("tool_choice")) {
        const json & tc = body.at("tool_choice");
        if (tc.is_object()) {
            std::string type = json_value(tc, "type", std::string());
            
            if (type == "auto") {
                oai_body["tool_choice"] = "auto";
            } else if (type == "any" || type == "tool") {
                // Anthropic 'any' → OpenAI 'required'
                oai_body["tool_choice"] = "required";
            } else if (type == "tool") {
                // Specific tool selection
                std::string tool_name = json_value(tc, "name", std::string());
                oai_body["tool_choice"] = {
                    {"type", "function"},
                    {"function", {{"name", tool_name}}}
                };
            }
        }
    }
    
    // === 5. PARAMETER MAPPING ===
    
    // stop_sequences → stop
    if (body.contains("stop_sequences")) {
        oai_body["stop"] = body.at("stop_sequences");
    }
    
    // max_tokens (required in Anthropic, permissive here)
    if (body.contains("max_tokens")) {
        oai_body["max_tokens"] = body.at("max_tokens");
    } else {
        // Default to prevent failures
        oai_body["max_tokens"] = 4096;
    }
    
    // Standard pass-through parameters
    for (const auto & key : {"temperature", "top_p", "top_k", "stream"}) {
        if (body.contains(key)) {
            oai_body[key] = body.at(key);
        }
    }
    
    // === 6. ANTHROPIC-SPECIFIC PARAMETERS ===
    
    // Thinking/reasoning configuration
    if (body.contains("thinking")) {
        json thinking = json_value(body, "thinking", json::object());
        std::string thinking_type = json_value(thinking, "type", std::string());
        
        if (thinking_type == "enabled") {
            int budget = json_value(thinking, "budget_tokens", 10000);
            oai_body["thinking_budget_tokens"] = budget;
        }
    }
    
    // Metadata passthrough
    if (body.contains("metadata")) {
        json metadata = json_value(body, "metadata", json::object());
        std::string user_id = json_value(metadata, "user_id", std::string());
        if (!user_id.empty()) {
            oai_body["__metadata_user_id"] = user_id;
        }
    }
    
    return oai_body;
}
```

### 2.2 Parameter Mapping Table

| Anthropic Field | OpenAI Field | Conversion Notes |
|-----------------|--------------|------------------|
| `system` (string) | `messages[0]` (role: system) | Direct mapping |
| `system` (block array) | `messages[0]` (role: system) | Concatenate text blocks |
| `messages[].role` | `messages[].role` | Direct |
| `messages[].content` (string) | `messages[].content` (string) | Direct |
| `messages[].content[]` (type: text) | `messages[].content[]` (type: text) | Direct |
| `messages[].content[]` (type: image) | `messages[].content[]` (type: image_url) | Build data: URL |
| `messages[].content[]` (type: tool_use) | `messages[].tool_calls[]` | Nested in assistant msg |
| `messages[].content[]` (type: tool_result) | `messages[]` (role: tool) | Separate message |
| `messages[].content[]` (type: thinking) | `messages[].reasoning_content` | Custom extension |
| `tools[].name` | `tools[].function.name` | Nested under function |
| `tools[].input_schema` | `tools[].function.parameters` | Renamed |
| `tool_choice.type: auto` | `tool_choice: "auto"` | Direct |
| `tool_choice.type: any` | `tool_choice: "required"` | Semantic mapping |
| `stop_sequences` | `stop` | Renamed |
| `max_tokens` | `max_tokens` | Direct (Anthropic requires it) |
| `thinking.budget_tokens` | `thinking_budget_tokens` | Custom extension |
| `metadata.user_id` | `__metadata_user_id` | Custom extension |

---

## 3. Response Formatting: OpenAI → Anthropic

### 3.1 Response Type Flag

The `task_response_type` enum controls output formatting:

```cpp
enum task_response_type {
    TASK_RESPONSE_TYPE_NONE,      // Native llama.cpp format
    TASK_RESPONSE_TYPE_OAI_CHAT,  // OpenAI Chat Completions
    TASK_RESPONSE_TYPE_OAI_CMPL,  // OpenAI Completions
    TASK_RESPONSE_TYPE_OAI_RESP,  // OpenAI Responses API
    TASK_RESPONSE_TYPE_OAI_EMBD,  // OpenAI Embeddings
    TASK_RESPONSE_TYPE_ANTHROPIC, // Anthropic Messages API ← Key for Claude Code
};
```

When `res_type == TASK_RESPONSE_TYPE_ANTHROPIC`, the server calls:
- `to_json_anthropic()` for non-streaming responses
- `to_json_anthropic_stream()` for streaming responses

---

## 4. Streaming Response Implementation

### 4.1 Event Sequence

Anthropic API uses typed Server-Sent Events (SSE):

```
event: message_start
data: {"type":"message_start","message":{...}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{...}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{...}}

... (more deltas)

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{...},"usage":{...}}

event: message_stop
data: {"type":"message_stop"}
```

### 4.2 Content Block Ordering

For Claude Code compatibility, content blocks MUST be in this order:

```
1. thinking (index 0) - if reasoning content exists
2. text (index 0 or 1) - if text content exists
3. tool_use (index N) - one per tool call
```

**Source**: `server-task.cpp` lines 1168-1354

```cpp
json server_task_result_cmpl_final::to_json_anthropic_stream() {
    json events = json::array();
    
    // Determine stop reason based on generation outcome
    std::string stop_reason = "max_tokens";
    if (stop == STOP_TYPE_WORD || stop == STOP_TYPE_EOS) {
        // Tool calls → "tool_use", otherwise "end_turn"
        stop_reason = oaicompat_msg.tool_calls.empty() 
            ? "end_turn" 
            : "tool_use";
    }
    
    // Check what content types exist
    bool has_thinking = !oaicompat_msg.reasoning_content.empty();
    bool has_text     = !oaicompat_msg.content.empty();
    size_t num_tool_calls = oaicompat_msg.tool_calls.size();
    
    // Calculate content block indices based on order
    size_t thinking_block_index = 0;
    size_t text_block_index     = has_thinking ? 1 : 0;
    
    // Track which blocks have been started (for incremental streaming)
    bool thinking_block_started = false;
    bool text_block_started     = false;
    std::unordered_set<size_t> tool_calls_started;
    
    // Process each diff (incremental change) from the parser
    for (const auto & diff : oaicompat_msg_diffs) {
        
        // === THINKING/REASONING DELTA ===
        if (!diff.reasoning_content_delta.empty()) {
            if (!thinking_block_started) {
                events.push_back({
                    {"event", "content_block_start"},
                    {"data", {
                        {"type", "content_block_start"},
                        {"index", thinking_block_index},
                        {"content_block", {
                            {"type", "thinking"},
                            {"thinking", ""}  // Empty initial content
                        }}
                    }}
                });
                thinking_block_started = true;
            }
            
            events.push_back({
                {"event", "content_block_delta"},
                {"data", {
                    {"type", "content_block_delta"},
                    {"index", thinking_block_index},
                    {"delta", {
                        {"type", "thinking_delta"},
                        {"thinking", diff.reasoning_content_delta}
                    }}
                }}
            });
        }
        
        // === TEXT CONTENT DELTA ===
        if (!diff.content_delta.empty()) {
            if (!text_block_started) {
                events.push_back({
                    {"event", "content_block_start"},
                    {"data", {
                        {"type", "content_block_start"},
                        {"index", text_block_index},
                        {"content_block", {
                            {"type", "text"},
                            {"text", ""}  // Empty initial content
                        }}
                    }}
                });
                text_block_started = true;
            }
            
            events.push_back({
                {"event", "content_block_delta"},
                {"data", {
                    {"type", "content_block_delta"},
                    {"index", text_block_index},
                    {"delta", {
                        {"type", "text_delta"},
                        {"text", diff.content_delta}
                    }}
                }}
            });
        }
        
        // === TOOL CALL DELTA ===
        if (diff.tool_call_index != std::string::npos) {
            // Calculate index based on content block order
            size_t content_block_index = 
                (has_thinking ? 1 : 0) + 
                (has_text ? 1 : 0) + 
                diff.tool_call_index;
            
            // First delta for this tool call → send content_block_start
            if (tool_calls_started.find(diff.tool_call_index) == tool_calls_started.end()) {
                const auto & full_tool_call = oaicompat_msg.tool_calls[diff.tool_call_index];
                
                events.push_back({
                    {"event", "content_block_start"},
                    {"data", {
                        {"type", "content_block_start"},
                        {"index", content_block_index},
                        {"content_block", {
                            {"type", "tool_use"},
                            {"id", full_tool_call.id},
                            {"name", full_tool_call.name}
                            // Note: 'input' starts empty, filled by deltas
                        }}
                    }}
                });
                tool_calls_started.insert(diff.tool_call_index);
            }
            
            // Send argument JSON delta
            if (!diff.tool_call_delta.arguments.empty()) {
                events.push_back({
                    {"event", "content_block_delta"},
                    {"data", {
                        {"type", "content_block_delta"},
                        {"index", content_block_index},
                        {"delta", {
                            {"type", "input_json_delta"},
                            {"partial_json", diff.tool_call_delta.arguments}
                        }}
                    }}
                });
            }
        }
    }
    
    // === CLOSE CONTENT BLOCKS (in order) ===
    
    // Close thinking block first (if exists)
    if (has_thinking) {
        // Anthropic requires signature_delta before closing thinking blocks
        // Empty signature for local models (no cryptographic verification)
        events.push_back({
            {"event", "content_block_delta"},
            {"data", {
                {"type", "content_block_delta"},
                {"index", thinking_block_index},
                {"delta", {
                    {"type", "signature_delta"},
                    {"signature", ""}  // Empty for local models
                }}
            }}
        });
        
        events.push_back({
            {"event", "content_block_stop"},
            {"data", {
                {"type", "content_block_stop"},
                {"index", thinking_block_index}
            }}
        });
    }
    
    // Close text block (if exists)
    if (has_text) {
        events.push_back({
            {"event", "content_block_stop"},
            {"data", {
                {"type", "content_block_stop"},
                {"index", text_block_index}
            }}
        });
    }
    
    // Close each tool_use block
    for (size_t i = 0; i < num_tool_calls; i++) {
        size_t content_block_index = 
            (has_thinking ? 1 : 0) + 
            (has_text ? 1 : 0) + i;
        
        events.push_back({
            {"event", "content_block_stop"},
            {"data", {
                {"type", "content_block_stop"},
                {"index", content_block_index}
            }}
        });
    }
    
    // === MESSAGE DELTA (stop reason + usage) ===
    events.push_back({
        {"event", "message_delta"},
        {"data", {
            {"type", "message_delta"},
            {"delta", {
                {"stop_reason", stop_reason},
                {"stop_sequence", stopping_word.empty() 
                    ? nullptr 
                    : json(stopping_word)}
            }},
            {"usage", {
                {"output_tokens", n_decoded}
            }}
        }}
    });
    
    // === MESSAGE STOP ===
    events.push_back({
        {"event", "message_stop"},
        {"data", {
            {"type", "message_stop"}
        }}
    });
    
    return events;
}
```

### 4.3 Partial Response Handling

For incremental streaming (chunk-by-chunk), `to_json_anthropic()` handles partial responses:

**Source**: `server-task.cpp` lines 1650-1787

```cpp
json server_task_result_cmpl_partial::to_json_anthropic() {
    json events = json::array();
    bool first = (n_decoded == 1);
    
    // message_start only on first chunk
    if (first) {
        events.push_back({
            {"event", "message_start"},
            {"data", {
                {"type", "message_start"},
                {"message", {
                    {"id", oaicompat_cmpl_id},
                    {"type", "message"},
                    {"role", "assistant"},
                    {"content", json::array()},
                    {"model", oaicompat_model},
                    {"stop_reason", nullptr},
                    {"stop_sequence", nullptr},
                    {"usage", {
                        {"cache_read_input_tokens", n_prompt_tokens_cache},
                        {"input_tokens", n_prompt_tokens - n_prompt_tokens_cache},
                        {"output_tokens", 0}  // Will update in message_delta
                    }}
                }}
            }}
        });
    }
    
    // Content block indices
    size_t thinking_block_index = 0;
    size_t text_block_index     = anthropic_has_reasoning ? 1 : 0;
    
    // State tracking from previous chunks
    bool thinking_started = thinking_block_started;
    bool text_started     = text_block_started;
    
    for (const auto & diff : oaicompat_msg_diffs) {
        // Thinking delta
        if (!diff.reasoning_content_delta.empty()) {
            if (!thinking_started) {
                events.push_back({
                    {"event", "content_block_start"},
                    {"data", {
                        {"type", "content_block_start"},
                        {"index", thinking_block_index},
                        {"content_block", {
                            {"type", "thinking"},
                            {"thinking", ""}
                        }}
                    }}
                });
                thinking_started = true;
            }
            
            events.push_back({
                {"event", "content_block_delta"},
                {"data": {
                    {"type", "content_block_delta"},
                    {"index", thinking_block_index},
                    {"delta", {
                        {"type", "thinking_delta"},
                        {"thinking", diff.reasoning_content_delta}
                    }}
                }}
            });
        }
        
        // Text delta
        if (!diff.content_delta.empty()) {
            if (!text_started) {
                events.push_back({
                    {"event", "content_block_start"},
                    {"data": {
                        {"type", "content_block_start"},
                        {"index", text_block_index},
                        {"content_block": {
                            {"type", "text"},
                            {"text", ""}
                        }}
                    }}
                });
                text_started = true;
            }
            
            events.push_back({
                {"event", "content_block_delta"},
                {"data": {
                    {"type", "content_block_delta"},
                    {"index", text_block_index},
                    {"delta": {
                        {"type", "text_delta"},
                        {"text", diff.content_delta}
                    }}
                }}
            });
        }
        
        // Tool call delta
        if (diff.tool_call_index != std::string::npos) {
            size_t content_block_index = 
                (anthropic_has_reasoning ? 1 : 0) + 
                (text_started ? 1 : 0) + 
                diff.tool_call_index;
            
            // Tool call header (name delta triggers block_start)
            if (!diff.tool_call_delta.name.empty()) {
                events.push_back({
                    {"event", "content_block_start"},
                    {"data": {
                        {"type", "content_block_start"},
                        {"index", content_block_index},
                        {"content_block": {
                            {"type", "tool_use"},
                            {"id", diff.tool_call_delta.id},
                            {"name", diff.tool_call_delta.name}
                        }}
                    }}
                });
            }
            
            // Tool arguments delta
            if (!diff.tool_call_delta.arguments.empty()) {
                events.push_back({
                    {"event", "content_block_delta"},
                    {"data": {
                        {"type", "content_block_delta"},
                        {"index", content_block_index},
                        {"delta": {
                            {"type", "input_json_delta"},
                            {"partial_json", diff.tool_call_delta.arguments}
                        }}
                    }}
                });
            }
        }
    }
    
    return events;
}
```

---

## 5. Non-Streaming Response Format

**Source**: `server-task.cpp` lines 1102-1166

```cpp
json server_task_result_cmpl_final::to_json_anthropic() {
    json content_blocks = json::array();
    
    // Use parsed OpenAI message or build from raw content
    common_chat_msg msg;
    if (!oaicompat_msg.empty()) {
        msg = oaicompat_msg;
    } else {
        msg.role = "assistant";
        msg.content = content;
    }
    
    // 1. Thinking block (if reasoning content exists)
    if (!msg.reasoning_content.empty()) {
        content_blocks.push_back({
            {"type", "thinking"},
            {"thinking", msg.reasoning_content},
            {"signature", ""}  // Empty for local models
        });
    }
    
    // 2. Text block (if content exists)
    if (!msg.content.empty()) {
        content_blocks.push_back({
            {"type", "text"},
            {"text", msg.content}
        });
    }
    
    // 3. Tool use blocks (if tool calls exist)
    for (const auto & tool_call : msg.tool_calls) {
        json tool_use_block = {
            {"type", "tool_use"},
            {"id", tool_call.id},
            {"name", tool_call.name}
        };
        
        // Parse arguments JSON string to object
        try {
            tool_use_block["input"] = json::parse(tool_call.arguments);
        } catch (const std::exception &) {
            tool_use_block["input"] = json::object();  // Fallback to empty
        }
        
        content_blocks.push_back(tool_use_block);
    }
    
    // Determine stop reason
    std::string stop_reason = "max_tokens";
    if (stop == STOP_TYPE_WORD || stop == STOP_TYPE_EOS) {
        stop_reason = msg.tool_calls.empty() 
            ? "end_turn" 
            : "tool_use";
    }
    
    // Build final response
    json res = {
        {"id", oaicompat_cmpl_id},
        {"type", "message"},
        {"role", "assistant"},
        {"content", content_blocks},
        {"model", oaicompat_model},
        {"stop_reason", stop_reason},
        {"stop_sequence", stopping_word.empty() 
            ? nullptr 
            : json(stopping_word)},
        {"usage", {
            {"cache_read_input_tokens", n_prompt_tokens_cache},
            {"input_tokens", n_prompt_tokens - n_prompt_tokens_cache},
            {"output_tokens", n_decoded}
        }}
    };
    
    return res;
}
```

---

## 6. Claude Code Compatibility Checklist

### 6.1 Required Features

| Feature | Status | Notes |
|---------|--------|-------|
| `/v1/messages` endpoint | ✅ Implemented | Direct route |
| `message_start` event | ✅ Implemented | First streaming event |
| `content_block_start/delta/stop` | ✅ Implemented | All block types |
| `message_delta` event | ✅ Implemented | Stop reason + usage |
| `message_stop` event | ✅ Implemented | Final event |
| Tool use blocks | ✅ Implemented | `type: tool_use` |
| Input JSON deltas | ✅ Implemented | `partial_json` |
| Thinking blocks | ✅ Implemented | Extended thinking support |
| Cache token counting | ✅ Implemented | `cache_read_input_tokens` |
| Stop reason: tool_use | ✅ Implemented | For tool call responses |

### 6.2 Tool Calling Compatibility

| Aspect | Implementation |
|--------|----------------|
| Tool definition format | `name`, `description`, `input_schema` |
| Tool choice: auto | Mapped to OpenAI `auto` |
| Tool choice: any | Mapped to OpenAI `required` |
| Tool result handling | Converted to `role: tool` messages |
| Parallel tool calls | Supported via multi-delta |
| Tool call IDs | Generated via `gen_tool_call_id()` |
| Argument streaming | Incremental `partial_json` deltas |

### 6.3 Streaming Order Guarantees

For Claude Code to correctly parse responses, blocks MUST arrive in order:

```
1. message_start
2. content_block_start (thinking, index 0) - if reasoning
3. content_block_delta (thinking_delta) × N
4. content_block_delta (signature_delta) - thinking close prep
5. content_block_stop (thinking, index 0) - if reasoning
6. content_block_start (text, index 0/1) - if text
7. content_block_delta (text_delta) × N
8. content_block_stop (text, index 0/1) - if text
9. content_block_start (tool_use, index N) × tool_calls
10. content_block_delta (input_json_delta) × N per tool
11. content_block_stop (tool_use, index N) × tool_calls
12. message_delta (stop_reason, usage)
13. message_stop
```

---

## 7. HTTP Handler Implementation

**Source**: `server-context.cpp` lines 3729-3762

```cpp
// === ANTHROPIC MESSAGES ENDPOINT ===
this->post_anthropic_messages = [this](const server_http_req & req) {
    auto res = create_response();
    std::vector<raw_buffer> files;
    
    // 1. Convert Anthropic → OpenAI
    json body = convert_anthropic_to_oai(json::parse(req.body));
    SRV_DBG("Request converted: Anthropic -> OpenAI Chat Completions\n");
    SRV_DBG("Converted request: %s\n", body.dump().c_str());
    
    // 2. Parse chat parameters
    json body_parsed = oaicompat_chat_params_parse(
        body,
        meta->chat_params,
        files);
    
    // 3. Handle with Anthropic response type
    return handle_completions_impl(
        req,
        SERVER_TASK_TYPE_COMPLETION,
        body_parsed,
        files,
        TASK_RESPONSE_TYPE_ANTHROPIC);  // ← Key flag
};

// === ANTHROPIC TOKEN COUNTING ENDPOINT ===
this->post_anthropic_count_tokens = [this](const server_http_req & req) {
    auto res = create_response();
    std::vector<raw_buffer> files;
    
    // 1. Convert Anthropic → OpenAI
    json body = convert_anthropic_to_oai(json::parse(req.body));
    json body_parsed = oaicompat_chat_params_parse(body, meta->chat_params, files);
    
    // 2. Tokenize prompt
    json prompt = body_parsed.at("prompt");
    llama_tokens tokens = tokenize_mixed(ctx_server.vocab, prompt, true, true);
    
    // 3. Return token count (Anthropic format)
    res->ok({{"input_tokens", static_cast<int>(tokens.size())}});
    return res;
};
```

---

## 8. Diff Computation for Incremental Parsing

**Source**: `common/chat.cpp` lines 153-219

The streaming implementation relies on computing diffs between successive parses of accumulated text:

```cpp
std::vector<common_chat_msg_diff> common_chat_msg_diff::compute_diffs(
        const common_chat_msg & msg_prv,
        const common_chat_msg & msg_new) {
    
    std::vector<common_chat_msg_diff> diffs;
    
    // Reasoning content delta
    if (msg_prv.reasoning_content != msg_new.reasoning_content) {
        auto & diff = diffs.emplace_back();
        diff.reasoning_content_delta = string_diff(
            msg_prv.reasoning_content, 
            msg_new.reasoning_content);
    }
    
    // Content delta
    if (msg_prv.content != msg_new.content) {
        auto & diff = diffs.emplace_back();
        diff.content_delta = string_diff(
            msg_prv.content, 
            msg_new.content);
    }
    
    // Tool call count validation
    if (msg_new.tool_calls.size() < msg_prv.tool_calls.size()) {
        throw std::runtime_error("Invalid diff: fewer tool calls now!");
    }
    
    // Existing tool call delta
    if (!msg_prv.tool_calls.empty()) {
        const size_t idx = msg_prv.tool_calls.size() - 1;
        const auto & pref = msg_prv.tool_calls[idx];
        const auto & newf = msg_new.tool_calls[idx];
        
        // Allow name to change during incremental parsing
        if (pref.name != newf.name && !pref.name.empty() && !newf.name.empty()) {
            bool is_prefix = (newf.name.rfind(pref.name, 0) == 0);
            if (!is_prefix) {
                throw std::runtime_error("Invalid diff: tool name mismatch!");
            }
        }
        
        const auto args_diff = string_diff(pref.arguments, newf.arguments);
        if (!args_diff.empty() || pref.id != newf.id || pref.name != newf.name) {
            auto & diff = diffs.emplace_back();
            diff.tool_call_index = idx;
            if (pref.id != newf.id || pref.name != newf.name) {
                diff.tool_call_delta.id = newf.id;
                diff.tool_call_delta.name = newf.name;
            }
            diff.tool_call_delta.arguments = args_diff;
        }
    }
    
    // New tool calls
    for (size_t idx = msg_prv.tool_calls.size(); idx < msg_new.tool_calls.size(); ++idx) {
        auto & diff = diffs.emplace_back();
        diff.tool_call_index = idx;
        diff.tool_call_delta = msg_new.tool_calls[idx];
    }
    
    return diffs;
}
```

---

## 9. Testing with Claude Code

### 9.1 Basic Request/Response Test

```bash
# Test basic completion
curl -X POST http://localhost:8080/v1/messages \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '{
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'

# Expected response:
{
  "id": "msg_abc123",
  "type": "message",
  "role": "assistant",
  "content": [{"type": "text", "text": "Hello! How can I help?"}],
  "model": "claude-3-sonnet-20240229",
  "stop_reason": "end_turn",
  "usage": {
    "input_tokens": 10,
    "output_tokens": 8
  }
}
```

### 9.2 Tool Calling Test

```bash
curl -X POST http://localhost:8080/v1/messages \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Weather in Istanbul?"}
    ],
    "tools": [{
      "name": "get_weather",
      "description": "Get weather for location",
      "input_schema": {
        "type": "object",
        "properties": {
          "location": {"type": "string"}
        },
        "required": ["location"]
      }
    }]
  }'

# Expected response with tool_use:
{
  "id": "msg_abc123",
  "type": "message",
  "role": "assistant",
  "content": [{
    "type": "tool_use",
    "id": "toolu_abc123",
    "name": "get_weather",
    "input": {"location": "Istanbul"}
  }],
  "stop_reason": "tool_use",
  ...
}
```

### 9.3 Streaming Test

```bash
curl -X POST http://localhost:8080/v1/messages \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-sonnet-20240229",
    "max_tokens": 1024,
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'

# Expected SSE stream:
event: message_start
data: {"type":"message_start","message":{...}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"!"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}

event: message_stop
data: {"type":"message_stop"}
```

---

## 10. Known Limitations and Workarounds

### 10.1 Image Support

Current implementation converts Anthropic image blocks to OpenAI `image_url` format:

```cpp
// Anthropic format
{"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "..."}}

// Converted to OpenAI format
{"type": "image_url", "image_url": {"url": "data:image/png;base64,..."}}
```

**Limitation**: Only base64 and URL sources supported. Local file paths require preprocessing.

### 10.2 Extended Thinking

Extended thinking blocks are mapped to the custom `reasoning_content` extension:

```cpp
// Anthropic extended thinking
{"type": "thinking", "thinking": "Let me think..."}

// Internal representation
{"reasoning_content": "Let me think..."}
```

**Limitation**: Signature verification not implemented for local models (empty signature sent).

### 10.3 Tool Choice Specificity

`tool_choice: {type: "tool", name: "specific_tool"}` maps to OpenAI's function selection:

```cpp
oai_body["tool_choice"] = {
    {"type", "function"},
    {"function", {{"name", "specific_tool"}}}
};
```

---

## 11. Summary

The llama.cpp Anthropic Messages API implementation achieves Claude Code compatibility through:

1. **Request Normalization**: `convert_anthropic_to_oai()` transforms all Anthropic inputs to internal OpenAI format
2. **Response Conversion**: `to_json_anthropic()` and `to_json_anthropic_stream()` convert internal state back to Anthropic format
3. **Streaming Order**: Strict content block ordering (thinking → text → tool_use) matches Anthropic spec
4. **Tool Call Handling**: Full support for tool definitions, deltas, and parallel calls
5. **Extended Features**: Thinking blocks, cache token counting, signature placeholders

**Key Files for Reference**:
- `tools/server/server-common.cpp` - `convert_anthropic_to_oai()` (lines 1436-1656)
- `tools/server/server-task.cpp` - `to_json_anthropic()` (1102-1166), `to_json_anthropic_stream()` (1168-1354), partial `to_json_anthropic()` (1650-1787)
- `tools/server/server-context.cpp` - HTTP handler (3729-3762)
- `common/chat.cpp` - Diff computation (153-219)

_Created: 2026-04-07_
_Source: Direct analysis of llama.cpp server implementation_
