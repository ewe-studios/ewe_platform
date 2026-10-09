# 16 - Server Implementation Deep Dive: llama.cpp Production Architecture

## Overview

This document provides a comprehensive analysis of the llama.cpp server implementation, covering the complete request lifecycle from HTTP endpoint through task scheduling, slot-based processing, and response formatting for multiple API formats (OpenAI Chat Completions, Anthropic Messages, OpenAI Responses).

**Source Files Analyzed**:
- `tools/server/server.cpp` - Main entry point and HTTP route registration (554 lines)
- `tools/server/server-models.h` - Multi-model routing and lifecycle management
- `tools/server/server-context.h` - Context metadata and route handler declarations
- `tools/server/server-context.cpp` - Core request handling and slot scheduling (4000+ lines)
- `tools/server/server-task.h` - Task types, parameters, and result structures (600+ lines)
- `tools/server/server-queue.h` - Task queue and response queue management
- `tools/server/server-common.h/cpp` - Shared utilities and API conversion functions
- `common/chat.h` - Chat template abstraction and tool calling structures

---

## 1. Server Architecture Overview

### 1.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      HTTP Server Layer                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────────┐ │
│  │ /v1/     │  │ /v1/     │  │ /v1/     │  │ /api/* (Ollama)│ │
│  │ chat/    │  │ messages │  │ completions│  │               │ │
│  │ completions│ │(Anthropic)│ │          │  │               │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └───────┬────────┘ │
└───────┼─────────────┼─────────────┼─────────────────┼──────────┘
        │             │             │                 │
        ▼             ▼             ▼                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Route Handler Layer                          │
│  ┌────────────────────────────────────────────────────────────┐│
│  │  ex_wrapper (exception handling)                           ││
│  └────────────────────────────────────────────────────────────┘│
│         │             │             │                          │
│         ▼             ▼             ▼                          │
│  ┌─────────────┐ ┌──────────────┐ ┌─────────────────────────┐ │
│  │post_chat_   │ │post_anthro-  │ │handle_completions_impl  │ │
│  │completions  │ │pic_messages │ │                         │ │
│  └──────┬──────┘ └──────┬───────┘ └───────────┬─────────────┘ │
└─────────┼────────────────┼─────────────────────┼───────────────┘
          │                │                     │
          ▼                ▼                     ▼
┌─────────────────────────────────────────────────────────────────┐
│                   API Conversion Layer                          │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ convert_anthropic_to_oai()                                │  │
│  │ convert_responses_to_chatcmpl()                           │  │
│  │ oaicompat_chat_params_parse()                             │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Task Queue Layer                             │
│  ┌─────────────────┐    ┌─────────────────┐                    │
│  │ server_queue    │    │ server_response │                    │
│  │ (task_queue)    │───▶│ (result_queue)  │                    │
│  │                 │    │                 │                    │
│  │ - defer queue   │    │ - streaming     │                    │
│  │ - priority      │    │ - batching      │                    │
│  └─────────────────┘    └─────────────────┘                    │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Slot Scheduler (update_slots)                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  6-State State Machine per Slot:                         │  │
│  │  IDLE → STARTED → PROCESSING_PROMPT → DONE_PROMPT →     │  │
│  │  GENERATING ──┐                                          │  │
│  │               └──────────────► (back to IDLE on complete)│  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │ Slot 0   │  │ Slot 1   │  │ Slot 2   │  │ Slot N   │       │
│  │(parallel)│  │(parallel)│  │(parallel)│  │(parallel)│       │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘       │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    llama_context (Inference)                    │
│  - KV cache management                                          │
│  - Batch token processing                                       │
│  - Speculative decoding support                                 │
│  - Grammar-constrained sampling                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 Model Lifecycle States

From `server-models.h`, each model goes through:

```
UNLOADED → LOADING → LOADED → SLEEPING
              ↑                   │
              └───────────────────┘
```

- **UNLOADED**: Model not in memory
- **LOADING**: Model being loaded from disk (mmap or traditional)
- **LOADED**: Model ready for inference
- **SLEEPING**: Model unloaded to free memory (KV cache cleared)

For multi-model deployments, subprocess management handles child server instances with automatic lifecycle transitions.

---

## 2. HTTP Endpoint Layer

### 2.1 Route Registration (server.cpp)

```cpp
// From tools/server/server.cpp (lines 148-188)

// Multi-model routing (when server runs in router mode)
if (is_router) {
    routes.post_chat_completions   = models_routes->proxy_post;
    routes.post_anthropic_messages = models_routes->proxy_post;
    // ... proxy to child subprocess
}

// Single-model mode - direct handlers
ctx_http.post("/chat/completions",    ex_wrapper(routes.post_chat_completions));
ctx_http.post("/v1/chat/completions", ex_wrapper(routes.post_chat_completions));
ctx_http.post("/api/chat",            ex_wrapper(routes.post_chat_completions)); // Ollama

ctx_http.post("/v1/messages",         ex_wrapper(routes.post_anthropic_messages));
ctx_http.post("/v1/messages/count_tokens", ex_wrapper(routes.post_anthropic_count_tokens));
```

### 2.2 Exception Wrapper Pattern

```cpp
// Exception wrapper for error handling
auto ex_wrapper = [](handler_t handler) {
    return [handler](const server_http_req & req) {
        try {
            return handler(req);
        } catch (const std::exception & e) {
            return format_error_response(e.what(), ERROR_TYPE_SERVER);
        }
    };
};
```

---

## 3. API Conversion Layer

### 3.1 Anthropic to OpenAI Conversion

**Source**: `server-common.cpp` lines 1436-1656

The `convert_anthropic_to_oai()` function transforms Anthropic Messages API format to OpenAI Chat Completions format:

```cpp
json convert_anthropic_to_oai(const json & body) {
    json oai_body;
    
    // 1. Convert system prompt
    json oai_messages = json::array();
    auto system_param = json_value(body, "system", json());
    if (!system_param.is_null()) {
        std::string system_content;
        if (system_param.is_string()) {
            system_content = system_param;
        } else if (system_param.is_array()) {
            // Handle block-based system prompts
            for (const auto & block : system_param) {
                if (json_value(block, "type", std::string()) == "text") {
                    system_content += json_value(block, "text", std::string());
                }
            }
        }
        oai_messages.push_back({
            {"role", "system"},
            {"content", system_content}
        });
    }
    
    // 2. Convert messages array
    for (const auto & msg : messages) {
        std::string role = json_value(msg, "role", std::string());
        const json & content = msg.at("content");
        
        json tool_calls = json::array();
        json converted_content = json::array();
        json tool_results = json::array();
        std::string reasoning_content;
        
        for (const auto & block : content) {
            std::string type = json_value(block, "type", std::string());
            
            if (type == "text") {
                converted_content.push_back(block);
            } else if (type == "thinking") {
                // Extract thinking/reasoning blocks
                reasoning_content += json_value(block, "thinking", std::string());
            } else if (type == "image") {
                // Convert Anthropic image format to OpenAI
                json source = json_value(block, "source", json::object());
                if (source_type == "base64") {
                    std::string media_type = json_value(source, "media_type", std::string("image/jpeg"));
                    std::string data = json_value(source, "data", std::string());
                    converted_content.push_back({
                        {"type", "image_url"},
                        {"image_url", {{"url", "data:" + media_type + ";base64," + data}}}
                    });
                }
            } else if (type == "tool_use") {
                // Convert Anthropic tool_use to OpenAI tool_calls
                tool_calls.push_back({
                    {"id", json_value(block, "id", std::string())},
                    {"type", "function"},
                    {"function", {
                        {"name", json_value(block, "name", std::string())},
                        {"arguments", json_value(block, "input", json::object()).dump()}
                    }}
                });
            } else if (type == "tool_result") {
                // Convert tool results to tool role messages
                tool_results.push_back({
                    {"role", "tool"},
                    {"tool_call_id", json_value(block, "tool_use_id", std::string())},
                    {"content", result_text}
                });
            }
        }
        
        // Build OpenAI message
        json new_msg = {{"role", role}};
        if (!converted_content.empty()) {
            new_msg["content"] = converted_content;
        }
        if (!tool_calls.empty()) {
            new_msg["tool_calls"] = tool_calls;
        }
        if (!reasoning_content.empty()) {
            new_msg["reasoning_content"] = reasoning_content;
        }
        oai_messages.push_back(new_msg);
        
        // Append tool result messages
        for (const auto & tool_msg : tool_results) {
            oai_messages.push_back(tool_msg);
        }
    }
    
    oai_body["messages"] = oai_messages;
    
    // 3. Convert tools array
    if (body.contains("tools")) {
        json oai_tools = json::array();
        for (const auto & tool : tools) {
            oai_tools.push_back({
                {"type", "function"},
                {"function", {
                    {"name", json_value(tool, "name", std::string())},
                    {"description", json_value(tool, "description", std::string())},
                    {"parameters", tool.contains("input_schema") 
                        ? tool.at("input_schema") 
                        : json::object()}
                }}
            });
        }
        oai_body["tools"] = oai_tools;
    }
    
    // 4. Convert tool_choice
    if (body.contains("tool_choice")) {
        std::string type = json_value(tc, "type", std::string());
        if (type == "auto") {
            oai_body["tool_choice"] = "auto";
        } else if (type == "any" || type == "tool") {
            oai_body["tool_choice"] = "required";
        }
    }
    
    // 5. Parameter mapping
    oai_body["stop"] = body.contains("stop_sequences") 
        ? body.at("stop_sequences") 
        : json::array();
    oai_body["max_tokens"] = body.contains("max_tokens") 
        ? body.at("max_tokens") 
        : 4096;
    
    // Pass through standard parameters
    for (const auto & key : {"temperature", "top_p", "top_k", "stream"}) {
        if (body.contains(key)) {
            oai_body[key] = body.at(key);
        }
    }
    
    // 6. Handle Anthropic-specific parameters
    if (body.contains("thinking")) {
        json thinking = json_value(body, "thinking", json::object());
        if (json_value(thinking, "type", std::string()) == "enabled") {
            int budget = json_value(thinking, "budget_tokens", 10000);
            oai_body["thinking_budget_tokens"] = budget;
        }
    }
    
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

### 3.2 Parameter Mapping Table

| Anthropic API | OpenAI Chat Completions | Notes |
|---------------|------------------------|-------|
| `system` | `messages[0]` (role: system) | Block array → concatenated string |
| `messages[].content[]` (type: text) | `messages[].content[]` | Direct mapping |
| `messages[].content[]` (type: image) | `messages[].content[]` (type: image_url) | URL format conversion |
| `messages[].content[]` (type: tool_use) | `messages[].tool_calls[]` | Nested in assistant message |
| `messages[].content[]` (type: tool_result) | `messages[]` (role: tool) | Separate message |
| `messages[].content[]` (type: thinking) | `messages[].reasoning_content` | Custom extension |
| `tools[].input_schema` | `tools[].function.parameters` | Renamed |
| `tool_choice.type: auto` | `tool_choice: "auto"` | Direct |
| `tool_choice.type: any` | `tool_choice: "required"` | Semantic mapping |
| `stop_sequences` | `stop` | Renamed |
| `max_output_tokens` | `max_tokens` | Renamed |
| `thinking.budget_tokens` | `thinking_budget_tokens` | Custom extension |
| `metadata.user_id` | `__metadata_user_id` | Custom extension |

### 3.3 OpenAI Responses to Chat Completions

**Source**: `server-common.cpp` lines 1400-1434

```cpp
json convert_responses_to_chatcmpl(const json & body) {
    json chatcmpl_body;
    json chatcmpl_messages = json::array();
    
    // Convert Responses format to Chat Completions format
    // Responses API uses: input/output structure
    // Chat Completions uses: messages array
    
    chatcmpl_body["messages"] = chatcmpl_messages;
    
    // Convert tools with strict mode
    if (response_body.contains("tools")) {
        json chatcmpl_tools = json::array();
        for (json resp_tool : response_body.at("tools")) {
            if (json_value(resp_tool, "type", std::string()) != "function") {
                throw std::invalid_argument("'type' of tool must be 'function'");
            }
            resp_tool.erase("type");
            chatcmpl_tool["type"] = "function";
            if (!resp_tool.contains("strict")) {
                resp_tool["strict"] = true;  // Default to strict mode
            }
            chatcmpl_tool["function"] = resp_tool;
            chatcmpl_tools.push_back(chatcmpl_tool);
        }
        chatcmpl_body["tools"] = chatcmpl_tools;
    }
    
    // Handle max_output_tokens → max_tokens
    if (response_body.contains("max_output_tokens")) {
        chatcmpl_body["max_tokens"] = response_body["max_output_tokens"];
    }
    
    return chatcmpl_body;
}
```

---

## 4. Task Queue System

### 4.1 Task Types (server-task.h)

```cpp
enum server_task_type {
    SERVER_TASK_TYPE_COMPLETION,     // Standard text generation
    SERVER_TASK_TYPE_EMBEDDING,      // Embedding generation
    SERVER_TASK_TYPE_RERANK,         // Document reranking
    SERVER_TASK_TYPE_INFILL,         // Code infilling (FIM)
    SERVER_TASK_TYPE_CANCEL,         // Cancel running task
    SERVER_TASK_TYPE_NEXT_RESPONSE,  // Trigger next response
    SERVER_TASK_TYPE_METRICS,        // Collect server metrics
    SERVER_TASK_TYPE_SLOT_SAVE,      // Save slot state to file
    SERVER_TASK_TYPE_SLOT_RESTORE,   // Restore slot from file
    SERVER_TASK_TYPE_SLOT_ERASE,     // Clear slot state
    SERVER_TASK_TYPE_GET_LORA,       // Get LoRA adapter info
    SERVER_TASK_TYPE_SET_LORA,       // Set LoRA adapter
};
```

### 4.2 Response Types

```cpp
enum task_response_type {
    TASK_RESPONSE_TYPE_NONE,         // Native llama.cpp format
    TASK_RESPONSE_TYPE_OAI_CHAT,     // OpenAI Chat Completions
    TASK_RESPONSE_TYPE_OAI_CMPL,     // OpenAI Completions
    TASK_RESPONSE_TYPE_OAI_RESP,     // OpenAI Responses API
    TASK_RESPONSE_TYPE_OAI_EMBD,     // OpenAI Embeddings
    TASK_RESPONSE_TYPE_ANTHROPIC,    // Anthropic Messages API
};
```

### 4.3 Task Parameters Structure

```cpp
struct task_params {
    // Streaming and caching
    bool stream          = true;
    bool include_usage   = false;
    bool cache_prompt    = true;  // Remember prompt to avoid reprocessing
    bool return_tokens   = false;
    bool return_progress = false;
    
    // Token limits
    int32_t n_keep      =  0;  // Tokens to keep from initial prompt
    int32_t n_discard   =  0;  // Tokens discardable when shifting context
    int32_t n_predict   = -1;  // New tokens to generate
    int32_t n_indent    =  0;  // Minimum line indentation
    int32_t n_cmpl      =  1;  // Number of completions (parallel sampling)
    
    // Cache reuse (for KV shift optimization)
    int32_t n_cache_reuse = 0;  // Min chunk size for cache reuse
    
    // Time limits
    int64_t t_max_prompt_ms  = -1;  // TODO: implement
    int64_t t_max_predict_ms = -1;  // Limit generation phase time
    
    // LoRA adapters
    std::map<int, float> lora;  // adapter ID → scale
    
    // Stopping criteria
    std::vector<std::string> antiprompt;  // Stop sequences
    std::vector<std::string> response_fields;
    
    // Timing and probabilities
    bool timings_per_token   = false;
    bool post_sampling_probs = false;
    
    // Sampling parameters (from common_params_sampling)
    struct common_params_sampling sampling;
    
    // Speculative decoding config
    struct common_params_speculative speculative;
    
    // Response formatting
    bool               verbose  = false;
    task_response_type res_type = TASK_RESPONSE_TYPE_NONE;
    std::string        oaicompat_model;
    std::string        oaicompat_cmpl_id;
    
    // Chat parsing parameters
    common_chat_parser_params chat_parser_params;
    
    // Embeddings
    int32_t embd_normalize = 2;  // -1=none, 0=max abs int16, 1=taxicab, 2=Euclidean
};
```

---

## 5. Slot-Based Scheduling

### 5.1 Slot State Machine

**Source**: `server-context.cpp` lines 36-101

```cpp
enum slot_state {
    SLOT_STATE_IDLE,             // Slot available for new task
    SLOT_STATE_WAIT_OTHER,       // Child task waiting for parent to process prompt
    SLOT_STATE_STARTED,          // Task assigned, about to process prompt
    SLOT_STATE_PROCESSING_PROMPT,// Currently processing prompt tokens
    SLOT_STATE_DONE_PROMPT,      // Prompt processing complete
    SLOT_STATE_GENERATING,       // Generating response tokens
};

struct server_slot {
    int id;                      // Slot ID (0 to n_parallel-1)
    int32_t n_ctx;               // Context size for this slot
    slot_state state = SLOT_STATE_IDLE;
    
    llama_context * ctx;         // Pointer to llama context
    mtmd_context * mctx;         // Multimodal context (if applicable)
    
    server_tokens prompt;        // Input prompt tokens + media chunks
    std::string generated_text;  // Generated text accumulator
    llama_tokens generated_tokens;  // Generated token IDs
    
    std::unique_ptr<server_task> task;      // Current task
    std::unique_ptr<server_task> task_prev; // Previous task (for metrics)
    
    // Generation state
    bool has_next_token = false;
    bool has_new_line = false;
    int32_t n_decoded = 0;       // Tokens generated
    int32_t n_remaining = 0;     // Tokens remaining to generate
    
    // Timing
    int64_t t_start_process_prompt;
    int64_t t_prompt_processing;
    int64_t t_token_generation;
    int32_t n_prompt_tokens_processed;
    int32_t n_prompt_tokens_cache;
    
    // Stopping
    stop_type stop = STOP_TYPE_NONE;
    std::string stopping_word;
    bool truncated = false;
    
    // Sampler state
    std::unique_ptr<common_sampler> smpl;
    common_speculative * spec = nullptr;  // Speculative decoding state
    
    // Prompt cache integration
    server_tokens prompt_cache;
    
    bool is_processing() const {
        return state != SLOT_STATE_IDLE;
    }
    
    void reset() {
        state = SLOT_STATE_IDLE;
        // ... reset all state
    }
    
    void release() {
        task.reset();
        reset();
    }
};
```

### 5.2 Slot Scheduling Algorithm

**Source**: `server-context.cpp` lines 1986-2900 (update_slots function)

```cpp
void update_slots() {
    // 1. Check if all slots are idle
    bool all_idle = true;
    for (const auto & slot : slots) {
        if (slot.is_processing()) {
            all_idle = false;
            break;
        }
    }
    if (all_idle) {
        SRV_INF("all slots are idle");
        // Signal queue to potentially enter sleeping state
        return;
    }
    
    // 2. Process each slot based on its state
    for (auto & slot : slots) {
        if (!slot.is_processing()) {
            continue;  // Skip idle slots
        }
        
        switch (slot.state) {
            case SLOT_STATE_WAIT_OTHER:
                // Child task waiting for parent
                if (slot.task->is_child()) {
                    // Check if parent has finished prompt processing
                    server_slot * parent_slot = get_slot_by_id(slot.task->id_parent);
                    if (parent_slot && parent_slot.state == SLOT_STATE_DONE_PROMPT) {
                        // Copy KV cache from parent
                        parent_slot.copy_state_to(slot);
                        slot.state = SLOT_STATE_DONE_PROMPT;
                    }
                }
                break;
                
            case SLOT_STATE_STARTED:
            case SLOT_STATE_PROCESSING_PROMPT:
                // Process prompt tokens
                if (slot.state == SLOT_STATE_STARTED) {
                    slot.state = SLOT_STATE_PROCESSING_PROMPT;
                }
                
                // Build batch from prompt tokens
                llama_batch batch = build_batch_from_prompt(slot);
                
                // Run inference on batch
                int ret = llama_decode(ctx, batch);
                
                if (ret == 0) {
                    // Prompt processing complete
                    slot.state = SLOT_STATE_DONE_PROMPT;
                    slot.t_prompt_processing = ggml_time_us() - slot.t_start_process_prompt;
                }
                break;
                
            case SLOT_STATE_DONE_PROMPT:
                // Initialize sampler and start generation
                slot.init_sampler();
                slot.state = SLOT_STATE_GENERATING;
                // Fall through to GENERATING
                
            case SLOT_STATE_GENERATING:
                // Sample next token
                completion_token_output result = sample_next_token(slot);
                
                // Check stopping criteria
                if (!process_token(result, slot)) {
                    // Generation complete
                    send_final_response(slot);
                    slot.release();
                    slot.state = SLOT_STATE_IDLE;
                } else {
                    // Send partial response (streaming)
                    send_partial_response(slot, result, false);
                }
                break;
        }
    }
    
    // 3. Handle parent-child task synchronization
    for (auto & slot : slots) {
        if (slot.state == SLOT_STATE_DONE_PROMPT && slot.task->is_parent()) {
            // Parent finished prompt processing, wake up children
            for (auto & other : slots) {
                if (other.state == SLOT_STATE_WAIT_OTHER 
                    && other.task->id_parent == slot.task->id) {
                    slot.copy_state_to(other);
                    other.state = SLOT_STATE_DONE_PROMPT;
                }
            }
        }
    }
    
    // 4. Metrics update
    metrics.on_decoded(slots);
}
```

### 5.3 Slot Selection Heuristics

**Source**: `server-context.cpp` lines 959-1000

```cpp
server_slot * get_available_slot(const server_task & task) {
    server_slot * ret = nullptr;
    
    // 1. First pass: Find slot with prompt similarity (LCP-based)
    if (slot_prompt_similarity != 0.0f) {
        float sim_best = 0;
        
        for (server_slot & slot : slots) {
            if (slot.is_processing()) {
                continue;  // Skip busy slots
            }
            
            const auto & tokens = slot.prompt.tokens;
            if (tokens.empty()) {
                continue;  // Skip empty slots
            }
            
            // Longest Common Prefix (LCP) similarity
            float sim_cur = float(tokens.get_common_prefix(task.tokens)) 
                          / task.tokens.size();
            
            if (sim_cur > sim_best && sim_cur > slot_prompt_similarity) {
                sim_best = sim_cur;
                ret = &slot;
            }
        }
        
        if (ret != nullptr) {
            // Decide whether to keep or save prompt cache
            float f_keep = (sim_best * task.tokens.size()) 
                         / ret->prompt.tokens.size();
            
            if (f_keep < 0.5f) {
                // Save to prompt cache before overwriting
                slot_save_and_clear(*ret);
            }
        }
    }
    
    // 2. Second pass: Find any idle slot
    if (ret == nullptr) {
        for (server_slot & slot : slots) {
            if (!slot.is_processing()) {
                ret = &slot;
                break;
            }
        }
    }
    
    return ret;
}
```

---

## 6. Request Processing Pipeline

### 6.1 Chat Completions Handler

**Source**: `server-context.cpp` lines 3695-3709

```cpp
this->post_chat_completions = [this](const server_http_req & req) {
    auto res = create_response();
    std::vector<raw_buffer> files;
    
    // 1. Parse JSON body
    json body = json::parse(req.body);
    
    // 2. Apply chat template and parse parameters
    json body_parsed = oaicompat_chat_params_parse(
        body,
        meta->chat_params,
        files);
    
    // 3. Handle completion with OpenAI Chat format
    return handle_completions_impl(
        req,
        SERVER_TASK_TYPE_COMPLETION,
        body_parsed,
        files,
        TASK_RESPONSE_TYPE_OAI_CHAT);
};
```

### 6.2 Anthropic Messages Handler

**Source**: `server-context.cpp` lines 3729-3745

```cpp
this->post_anthropic_messages = [this](const server_http_req & req) {
    auto res = create_response();
    std::vector<raw_buffer> files;
    
    // 1. Convert Anthropic format to OpenAI
    json body = convert_anthropic_to_oai(json::parse(req.body));
    SRV_DBG("Request converted: Anthropic -> OpenAI Chat Completions");
    
    // 2. Parse chat parameters
    json body_parsed = oaicompat_chat_params_parse(
        body,
        meta->chat_params,
        files);
    
    // 3. Handle completion with Anthropic format
    return handle_completions_impl(
        req,
        SERVER_TASK_TYPE_COMPLETION,
        body_parsed,
        files,
        TASK_RESPONSE_TYPE_ANTHROPIC);
};
```

### 6.3 Token Counting Handler

**Source**: `server-context.cpp` lines 3747-3762

```cpp
this->post_anthropic_count_tokens = [this](const server_http_req & req) {
    auto res = create_response();
    std::vector<raw_buffer> files;
    
    // 1. Convert Anthropic to OpenAI format
    json body = convert_anthropic_to_oai(json::parse(req.body));
    json body_parsed = oaicompat_chat_params_parse(body, meta->chat_params, files);
    
    // 2. Tokenize the prompt
    json prompt = body_parsed.at("prompt");
    llama_tokens tokens = tokenize_mixed(ctx_server.vocab, prompt, true, true);
    
    // 3. Return token count
    res->ok({{"input_tokens", static_cast<int>(tokens.size())}});
    return res;
};
```

---

## 7. Chat Template Integration

### 7.1 Core Structures (common/chat.h)

```cpp
struct common_chat_tool {
    std::string name;         // Function name
    std::string description;  // What the tool does
    std::string parameters;   // JSON schema as string
};

struct common_chat_tool_call {
    std::string name;         // Which tool was called
    std::string arguments;    // JSON string of arguments
    std::string id;           // Unique identifier (call_abc123)
};

struct common_chat_msg {
    std::string role;                          // system, user, assistant, tool
    std::string content;                       // Plain text content
    std::vector<common_chat_msg_content_part> content_parts;  // Multi-modal
    std::vector<common_chat_tool_call> tool_calls;            // Tool calls
    std::string reasoning_content;             // Chain-of-thought
    std::string tool_name;                     // For tool result messages
    std::string tool_call_id;                  // For tool result messages
    
    nlohmann::ordered_json to_json_oaicompat(bool concat_typed_text = false) const;
};

struct common_chat_tool_choice {
    COMMON_CHAT_TOOL_CHOICE_AUTO,     // Model decides
    COMMON_CHAT_TOOL_CHOICE_REQUIRED, // Must call a tool
    COMMON_CHAT_TOOL_CHOICE_NONE,     // No tool calling
};

struct common_chat_templates_inputs {
    std::vector<common_chat_msg> messages;
    std::string grammar;                          // GBNF grammar
    std::string json_schema;                      // For output constraints
    bool add_generation_prompt = true;
    bool use_jinja = true;
    
    // Tool calling support
    std::vector<common_chat_tool> tools;
    common_chat_tool_choice tool_choice = COMMON_CHAT_TOOL_CHOICE_AUTO;
    bool parallel_tool_calls = false;
    
    // Thinking/reasoning support
    common_reasoning_format reasoning_format = COMMON_REASONING_FORMAT_NONE;
    bool enable_thinking = true;
    
    std::map<std::string, std::string> chat_template_kwargs;
    bool add_bos = false;
    bool add_eos = false;
    bool force_pure_content = false;
};
```

### 7.2 Grammar Triggers for Tool Calling

```cpp
struct server_grammar_trigger {
    common_grammar_trigger value;
    
    // Trigger types:
    // - COMMON_GRAMMAR_TRIGGER_TYPE_TOKEN: Specific token ID
    // - COMMON_GRAMMAR_TRIGGER_TYPE_WORD: Specific word string
    // - COMMON_GRAMMAR_TRIGGER_TYPE_PATTERN: Regex pattern
    
    json to_json() const {
        json out {
            {"type", (int) value.type},
            {"value", value.value},
        };
        if (value.type == COMMON_GRAMMAR_TRIGGER_TYPE_TOKEN) {
            out["token"] = (int) value.token;
        }
        return out;
    }
};
```

---

## 8. Response Formatting

### 8.1 Partial Response (Streaming)

**Source**: `server-context.cpp` lines 1442-1480

```cpp
void send_partial_response(server_slot & slot, 
                           const completion_token_output & tkn, 
                           bool is_progress) {
    auto res = std::make_unique<server_task_result_cmpl_partial>();
    
    res->id    = slot.task->id;
    res->index = slot.task->index;
    
    if (is_progress) {
        // Progress update during prompt processing
        res->is_progress = true;
        res->progress.total = slot.task->n_tokens();
        res->progress.cache = slot.n_prompt_tokens_cache;
        res->progress.processed = slot.prompt.tokens.size();
        res->progress.time_ms = (ggml_time_us() - slot.t_start_process_prompt) / 1000;
    } else {
        // Token generation
        res->content = tkn.text_to_send;
        res->tokens = {tkn.tok};
    }
    
    res->n_decoded = slot.n_decoded;
    res->n_prompt_tokens = slot.task->n_tokens();
    res->post_sampling_probs = slot.task->params.post_sampling_probs;
    
    // Timings (if enabled)
    if (slot.stop != STOP_TYPE_NONE || slot.task->params.timings_per_token) {
        res->timings = slot.get_timings();
    }
    
    queue_results.send(std::move(res));
}
```

### 8.2 Final Response

**Source**: `server-context.cpp` lines 1482-1543

```cpp
void send_final_response(server_slot & slot) {
    auto res = std::make_unique<server_task_result_cmpl_final>();
    
    res->id = slot.task->id;
    res->id_slot = slot.id;
    res->index = slot.task->index;
    
    // Content: empty in streaming mode (already sent)
    if (slot.task->params.stream) {
        res->content = "";
        res->tokens = llama_tokens{};
    } else {
        res->content = std::move(slot.generated_text);
        res->tokens = std::move(slot.generated_tokens);
    }
    
    res->timings = slot.get_timings();
    res->prompt = slot.task->tokens.detokenize(ctx, true);
    res->n_decoded = slot.n_decoded;
    res->n_prompt_tokens = slot.task->n_tokens();
    res->stopping_word = slot.stopping_word;
    res->stop = slot.stop;
    res->oaicompat_model = slot.task->params.oaicompat_model;
    res->oaicompat_cmpl_id = slot.task->params.oaicompat_cmpl_id;
    
    // Token probabilities (if requested)
    if (slot.task->params.sampling.n_probs > 0) {
        if (!slot.task->params.stream && slot.stop == STOP_TYPE_WORD) {
            const llama_tokens stop_word_toks = common_tokenize(ctx, slot.stopping_word, false);
            size_t safe_offset = std::min(slot.generated_token_probs.size(), stop_word_toks.size());
            res->probs_output = std::vector<completion_token_output>(
                slot.generated_token_probs.begin(),
                slot.generated_token_probs.end() - safe_offset);
        } else {
            res->probs_output = std::vector<completion_token_output>(
                slot.generated_token_probs.begin(),
                slot.generated_token_probs.end());
        }
    }
    
    queue_results.send(std::move(res));
}
```

### 8.3 Server-Sent Events (SSE) Formatting

**Source**: `server-common.h` lines 342-348

```cpp
// OpenAI-style SSE (data: only)
std::string format_oai_sse(const json & data) {
    if (data.is_array()) {
        // Send multiple events for array data
        std::string result;
        for (const auto & item : data) {
            result += "data: " + item.dump() + "\n\n";
        }
        return result;
    }
    return "data: " + data.dump() + "\n\n";
}

// Anthropic-style SSE with event types
std::string format_anthropic_sse(const json & data) {
    std::string event_type = json_value(data, "type", std::string("message"));
    return "event: " + event_type + "\ndata: " + data.dump() + "\n\n";
}
```

---

## 9. Tool Calling Detection and Normalization

### 9.1 Detection via Grammar Constraints

Tool calling detection happens through grammar-constrained generation:

1. **Grammar Generation**: JSON schemas → GBNF grammar
2. **Grammar Sampler**: Added to sampler chain BEFORE temperature
3. **Lazy Activation**: For models with free-form text before tool calls

```cpp
// Grammar sampler initialization (from json-schema-to-grammar.cpp)
llama_sampler * grammar_sampler = llama_sampler_init_grammar(
    vocab,          // Vocabulary for token mapping
    grammar_str,    // GBNF grammar string
    grammar_root    // Root rule name ("root")
);

// Lazy activation for models with mixed formats
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

### 9.2 Model-Specific Format Handlers

From `common/chat.cpp`, format detection based on chat template:

| Model Family | Detection Pattern | Format Handler |
|--------------|-------------------|----------------|
| Llama 3.x | `<|python_tag|>` | `common_chat_format_peg_native` |
| Hermes 2/3 | `## 4. Tool` | `common_chat_format_peg_native` |
| Mistral | `\[TOOL_CALLS\]` | `common_chat_format_peg_native` |
| Functionary | `<tool>...</tool>` | `common_chat_format_peg_native` |
| Generic/Unknown | N/A | `common_chat_format_content_only` |

### 9.3 PEG Parser for Tool Call Extraction

**Source**: `common/chat-peg-parser.h`

The PEG (Parsing Expression Grammar) parser extracts structured tool calls from generated text:

```cpp
class common_chat_peg_mapper {
  public:
    common_chat_msg & result;
    
    virtual void from_ast(const common_peg_ast_arena & arena, 
                          const common_peg_parse_result & result);
    virtual void map(const common_peg_ast_node & node);
    
  protected:
    // Tool call handling state
    std::optional<common_chat_tool_call> pending_tool_call;  // Tool call waiting for name
    common_chat_tool_call * current_tool = nullptr;
    int arg_count = 0;
    bool closing_quote_pending = false;
    std::string args_buffer;  // Buffer arguments until tool name is known
    
    std::string & args_target();  // Returns active argument destination
};

class common_chat_peg_builder : public common_peg_parser_builder {
  public:
    // Tag constants for PEG grammar
    static constexpr const char * TOOL = "tool";
    static constexpr const char * TOOL_OPEN = "tool-open";
    static constexpr const char * TOOL_CLOSE = "tool-close";
    static constexpr const char * TOOL_ID = "tool-id";
    static constexpr const char * TOOL_NAME = "tool-name";
    static constexpr const char * TOOL_ARGS = "tool-args";
    static constexpr const char * TOOL_ARG = "tool-arg";
    static constexpr const char * TOOL_ARG_NAME = "tool-arg-name";
    static constexpr const char * TOOL_ARG_VALUE = "tool-arg-value";
    
    // Build grammar rules for specific model formats
    common_peg_parser standard_json_tools(
        const std::string & section_start,
        const std::string & section_end,
        const nlohmann::ordered_json & tools,
        bool parallel_tool_calls,
        bool force_tool_calls,
        const std::string & name_key = "",
        const std::string & args_key = "",
        bool array_wrapped = false,
        bool function_is_key = false,
        const std::string & call_id_key = "",
        const std::string & gen_call_id_key = "",
        const std::vector<std::string> & parameters_order = {});
};
```

### 9.4 Incremental Tool Call Parsing

**Source**: `server-task.cpp` lines 157-232

For streaming responses, tool calls are parsed incrementally:

```cpp
common_chat_msg task_result_state::update_chat_msg(
        const std::string & text_added,
        bool is_partial,
        std::vector<common_chat_msg_diff> & diffs,
        bool filter_tool_calls) {
    
    generated_text += text_added;
    auto msg_prv_copy = chat_msg;
    
    // Parse complete text (re-parsing each chunk)
    auto new_msg = common_chat_parse(
        generated_text,
        is_partial,
        chat_parser_params);
    
    if (!new_msg.empty()) {
        // Assign tool call IDs if not present
        new_msg.set_tool_call_ids(generated_tool_call_ids, gen_tool_call_id);
        chat_msg = new_msg;
        
        // Compute diffs between previous and current parse
        auto all_diffs = common_chat_msg_diff::compute_diffs(msg_prv_copy, chat_msg);
        
        if (filter_tool_calls) {
            // Filter to only send each tool call header once
            for (auto & d : all_diffs) {
                for (size_t i = 0; i < chat_msg.tool_calls.size(); ++i) {
                    if (sent_tool_call_names.count(i) || chat_msg.tool_calls[i].name.empty()) {
                        continue;
                    }
                    if (d.tool_call_index != i || !d.tool_call_delta.arguments.empty()) {
                        // Send header with name and id
                        common_chat_msg_diff header;
                        header.tool_call_index = i;
                        header.tool_call_delta.id = chat_msg.tool_calls[i].id;
                        header.tool_call_delta.name = chat_msg.tool_calls[i].name;
                        diffs.push_back(std::move(header));
                        sent_tool_call_names.insert(i);
                    }
                }
                
                // Only send argument deltas after header sent
                if (d.tool_call_index == std::string::npos) {
                    diffs.push_back(std::move(d));
                } else {
                    size_t i = d.tool_call_index;
                    if (sent_tool_call_names.count(i)) {
                        if (!d.tool_call_delta.arguments.empty()) {
                            d.tool_call_delta.name = "";
                            d.tool_call_delta.id = "";
                            diffs.push_back(std::move(d));
                        }
                    } else {
                        if (!d.tool_call_delta.arguments.empty() || !is_partial) {
                            d.tool_call_delta.name = chat_msg.tool_calls[i].name;
                            d.tool_call_delta.id = chat_msg.tool_calls[i].id;
                            diffs.push_back(std::move(d));
                            sent_tool_call_names.insert(i);
                        }
                    }
                }
            }
        } else {
            diffs = std::move(all_diffs);
        }
    }
    
    return chat_msg;
}
```

### 9.5 Diff Computation

**Source**: `common/chat.cpp` lines 153-219

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
        diff.content_delta = string_diff(msg_prv.content, msg_new.content);
    }
    
    // Tool call diffs
    if (msg_new.tool_calls.size() < msg_prv.tool_calls.size()) {
        throw std::runtime_error("Invalid diff: now finding less tool calls!");
    }
    
    if (!msg_prv.tool_calls.empty()) {
        const size_t idx = msg_prv.tool_calls.size() - 1;
        const auto & pref = msg_prv.tool_calls[idx];
        const auto & newf = msg_new.tool_calls[idx];
        
        // Allow tool name to change during incremental parsing
        if (pref.name != newf.name && !pref.name.empty() && !newf.name.empty()) {
            bool is_prefix = (newf.name.rfind(pref.name, 0) == 0);
            if (!is_prefix) {
                throw std::runtime_error("Invalid diff: tool call mismatch!");
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

## 10. OpenAPI Specification

### 10.1 OpenAI Chat Completions Endpoint

```yaml
POST /v1/chat/completions
Content-Type: application/json

Request:
{
  "model": "model-name",
  "messages": [
    {"role": "system", "content": "You are helpful"},
    {"role": "user", "content": "Hello"}
  ],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Get weather for location",
        "parameters": {
          "type": "object",
          "properties": {
            "location": {"type": "string"}
          },
          "required": ["location"]
        }
      }
    }
  ],
  "tool_choice": "auto",
  "stream": false,
  "max_tokens": 1024,
  "temperature": 0.7
}

Response (200 OK):
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "model-name",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": null,
        "tool_calls": [
          {
            "id": "call_abc123",
            "type": "function",
            "function": {
              "name": "get_weather",
              "arguments": "{\"location\":\"Istanbul\"}"
            }
          }
        ]
      },
      "finish_reason": "tool_calls"
    }
  ],
  "usage": {
    "prompt_tokens": 25,
    "completion_tokens": 18,
    "total_tokens": 43
  }
}
```

### 10.2 Anthropic Messages Endpoint

```yaml
POST /v1/messages
Content-Type: application/json

Request:
{
  "model": "claude-3-sonnet-20240229",
  "max_tokens": 1024,
  "messages": [
    {"role": "user", "content": "What's the weather in Istanbul?"}
  ],
  "tools": [
    {
      "name": "get_weather",
      "description": "Get weather for location",
      "input_schema": {
        "type": "object",
        "properties": {
          "location": {"type": "string"}
        },
        "required": ["location"]
      }
    }
  ],
  "stream": false
}

Response (200 OK):
{
  "id": "msg_abc123",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "tool_use",
      "id": "toolu_abc123",
      "name": "get_weather",
      "input": {"location": "Istanbul"}
    }
  ],
  "model": "claude-3-sonnet-20240229",
  "stop_reason": "tool_use",
  "usage": {
    "input_tokens": 25,
    "output_tokens": 18
  }
}
```

### 10.3 Streaming Response Format

**OpenAI Style (SSE)**:
```
data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1234567890,"model":"model","choices":[{"index":0,"delta":{"role":"assistant","content":null,"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather","arguments":""}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1234567890,"model":"model","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"loc"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1234567890,"model":"model","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"ation\":\"Istanbul\"}"}}]},"finish_reason":null}]}

data: {"id":"chatcmpl-abc","object":"chat.completion.chunk","created":1234567890,"model":"model","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: [DONE]
```

**Anthropic Style (SSE with event types)**:
```
event: message_start
data: {"type":"message_start","message":{"id":"msg_abc","type":"message","role":"assistant","content":[],"model":"claude-3","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":25,"output_tokens":1}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_abc","name":"get_weather","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"loc"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"ation\":\"Istanbul\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":18}}

event: message_stop
data: {"type":"message_stop"}
```

---

## 11. Key Implementation Patterns

### 11.1 Parent-Child Task Synchronization

For parallel tool calls, the server uses a parent-child task model:

```cpp
// Parent task creates child tasks
if (tool_choice == "required" && parallel_tool_calls) {
    server_task parent_task(SERVER_TASK_TYPE_COMPLETION);
    
    for (size_t i = 0; i < expected_tool_calls; ++i) {
        parent_task.add_child(parent_task.id, child_id++);
    }
    
    // Children wait for parent to finish prompt processing
    // Then copy KV cache state from parent
}
```

### 11.2 KV Cache Copy for Parallel Generation

```cpp
void server_slot::copy_state_to(server_slot & other) const {
    GGML_ASSERT(state == SLOT_STATE_DONE_PROMPT);
    
    // Copy KV cache via llama_memory_seq_cp
    llama_memory_seq_rm(llama_get_memory(ctx), other.id, -1, -1);
    llama_memory_seq_cp(llama_get_memory(ctx), id, other.id, -1, -1);
    
    // Copy generation state
    other.n_decoded = n_decoded;
    other.n_remaining = n_remaining;
    other.i_batch = i_batch;
    other.prompt = prompt.clone();
    other.init_sampler();
}
```

### 11.3 Response Type Routing

The `task_response_type` determines output formatting:

```cpp
json server_task_result_cmpl_final::to_json() {
    switch (res_type) {
        case TASK_RESPONSE_TYPE_OAI_CHAT:
            return to_json_oai_chat();
        case TASK_RESPONSE_TYPE_ANTHROPIC:
            return to_json_anthropic();
        case TASK_RESPONSE_TYPE_OAI_RESP:
            return to_json_oai_responses();
        default:
            return to_json_native();
    }
}
```

---

## 12. Performance Considerations

### 12.1 Prompt Caching

- **KV Cache Reuse**: Slots with similar prompts reuse KV cache via LCP (Longest Common Prefix) matching
- **Cache Threshold**: `slot_prompt_similarity` parameter (default 0.0) controls minimum similarity for slot selection
- **Cache Save/Load**: Idle slots can be saved to prompt cache when similarity is low (`f_keep < 0.5`)

### 12.2 Batch Processing

```cpp
// Batch initialization
batch = llama_batch_init(std::max(n_batch, params_base.n_parallel), 0, 1);

// Continuous batching in update_slots()
// - Multiple slots processed in single llama_decode() call
// - Batch size adapts to active slots
```

### 12.3 Speculative Decoding

Supported via draft model configuration:

```cpp
struct common_params_speculative {
    enum common_speculative_type type;
    int32_t n_min;       // Min draft tokens
    int32_t n_max;       // Max draft tokens
    float p_min;         // Minimum acceptance probability
    struct common_params_model mparams_dft;  // Draft model params
    struct llama_model * model_dft;          // Draft model pointer
};
```

---

## 13. Error Handling

### 13.1 Error Types

```cpp
enum error_type {
    ERROR_TYPE_INVALID_REQUEST,    // 400 Bad Request
    ERROR_TYPE_AUTHENTICATION,     // 401 Unauthorized
    ERROR_TYPE_SERVER,             // 500 Internal Server Error
    ERROR_TYPE_NOT_FOUND,          // 404 Not Found
    ERROR_TYPE_PERMISSION,         // 403 Forbidden
    ERROR_TYPE_UNAVAILABLE,        // 503 Service Unavailable (custom)
    ERROR_TYPE_NOT_SUPPORTED,      // 400 Not Supported (custom)
    ERROR_TYPE_EXCEED_CONTEXT_SIZE,// 400 Context Too Large (custom)
};
```

### 13.2 Error Response Format

```json
{
  "error": {
    "message": "Invalid 'messages' type: expected array",
    "type": "invalid_request_error",
    "param": "messages",
    "code": "invalid_param"
  }
}
```

---

## 14. Configuration Options

### 14.1 Server Flags (from README.md)

| Flag | Description |
|------|-------------|
| `--port` | Server listening port (default: 8080) |
| `--host` | Server listening address |
| `--api-key` | API key for authentication |
| `--slots` | Number of parallel slots (default: 1) |
| `--jinja` | Enable Jinja2 chat template support |
| `--no-jinja` | Disable Jinja2 (use simple template) |
| `--chat-template` | Custom chat template string |
| `--reasoning` | Reasoning format: off, deepseek, gemma3 |
| `--speculative` | Speculative decoding type |
| `--cache-ram` | Prompt cache RAM limit (MiB) |
| `--clear-idle` | Clear idle slots to save memory |

### 14.2 Chat Parsing Parameters

```cpp
struct common_chat_parser_params {
    common_chat_format format = COMMON_CHAT_FORMAT_CONTENT_ONLY;
    common_reasoning_format reasoning_format = COMMON_REASONING_FORMAT_NONE;
    bool reasoning_in_content = false;
    std::string generation_prompt;
    bool parse_tool_calls = true;
    bool force_pure_content = false;  // Skip chat parsing
};
```

---

## 15. Testing and Debugging

### 15.1 Logging Macros

```cpp
#define SLT_INF(slot, fmt, ...) LOG_INF("slot %12.*s: id %2d | task %d | " fmt, ...)
#define SRV_DBG(fmt, ...) LOG_DBG("srv  %12.*s: " fmt, ...)

// Usage in code
SLT_INF(slot, "processing task, is_child = %d\n", slot.task->is_child());
SRV_DBG("no slot is available, defer task, id_task = %d\n", id_task);
```

### 15.2 Slots Debug Mode

Set environment variable for verbose slot debugging:

```bash
LLAMA_SERVER_SLOTS_DEBUG=1 ./llama-server ...
```

### 15.3 Verbose Response Mode

```json
{
  "verbose": true,
  "timings_per_token": true
}
```

Response includes:
```json
{
  "timings": {
    "prompt_n": 25,
    "prompt_ms": 45.2,
    "prompt_per_token_ms": 1.81,
    "prompt_per_second": 553.1,
    "predicted_n": 100,
    "predicted_ms": 2500.0,
    "predicted_per_token_ms": 25.0,
    "predicted_per_second": 40.0
  }
}
```

---

## 16. Summary

The llama.cpp server implementation provides:

1. **Multi-API Compatibility**: OpenAI Chat Completions, Anthropic Messages, OpenAI Responses, Ollama
2. **Efficient Resource Utilization**: Slot-based parallelism with KV cache sharing
3. **Grammar-Constrained Tool Calling**: JSON Schema → GBNF → constrained sampling
4. **Model-Aware Parsing**: PEG parser adapts to model-specific tool call formats
5. **Streaming Support**: Incremental parsing with diff-based delta generation
6. **Production Features**: Prompt caching, speculative decoding, LoRA adapters, metrics

**Key Design Decisions**:
- All API formats normalize to internal `task_params` before processing
- Slot state machine enables efficient parallel request handling
- PEG parser provides robust tool call extraction across formats
- Grammar constraints ensure valid JSON output for tool calls
- Lazy grammar activation supports mixed free-form + structured output

_Created: 2026-04-07_
_Source: llama.cpp tools/server/, common/chat*.h/cpp_
