---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/05-agentic-coding-reference"
this_file: "specifications/07-foundation-ai/features/05-agentic-coding-reference/feature.md"

feature: "agentic-coding-reference"
description: "Deep reference analysis of pi-mono (TypeScript) and hermes-agent (Python) agentic coding projects -- full API, model, agentic loop, provider integrations, tool handling, token usage, message handling, and execution behavior"
status: unapproved
priority: high
depends_on:
  - "04-tool-calling-formatter"
estimated_effort: "reference"
created: 2026-04-22
last_updated: 2026-04-22
author: "Main Agent"

tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# Agentic Coding Projects Reference

## Overview

This feature is a comprehensive documentary analysis of two agentic coding AI projects that serve as reference implementations for building agentic capabilities in foundation_ai. It covers the full API surface, model catalogs, agentic loop behavior, provider integrations, tool handling, token usage calculation, message handling, and execution patterns of both projects.

**This feature is a reference/documentation feature. It details what exists in the reference projects to inform future implementation decisions. It is not an implementation feature itself.**

## Reference Projects

| Project | Language | Structure | Description |
|---------|----------|-----------|-------------|
| **pi-mono** | TypeScript (Bun monorepo) | `@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.Pi/pi-mono/` | Multi-provider AI abstraction layer with agent loop, coding agent CLI, TUI, web UI |
| **hermes-agent** | Python (Nous Research) | `@formulas/src.rust/src.llamacpp/src.All/hermes-agent/` | Agentic coding agent with conversation-driven tool calling, messaging gateway, ACP adapter |

## Architecture Comparison at a Glance

| Dimension | pi-mono | hermes-agent |
|-----------|---------|-------------|
| Language | TypeScript (Bun) | Python |
| Layering | 3 layers: pi-ai -> pi-agent-core -> coding-agent | Single orchestrator: AIAgent (run_agent.py, ~9000+ lines) |
| Provider routing | API registry with lazy-loading provider modules | Centralized provider resolution with credential pool |
| API modes | 11 API types (openai-completions, anthropic-messages, etc.) | 3 API modes (chat_completions, codex_responses, anthropic_messages) |
| Tool registry | TypeBox schema validation, AgentTool.execute() | Registry singleton, self-registering at import time |
| Context management | Compaction via LLM summarization | Context compression with auxiliary model, head/tail protection |
| Run modes | Interactive TUI, print, JSON, RPC | Interactive CLI, messaging gateway (Telegram/Discord/etc.), ACP |
| Session persistence | JSONL files via SessionManager | JSON + SQLite via session DB |
| Token tracking | Usage struct with per-request cost | CanonicalUsage with session cumulative counters |
| Credential management | Per-provider env vars + OAuth | PooledCredential with failover strategies |
| Parallel tools | Sequential by default, configurable | ThreadPoolExecutor (8 workers), path-overlap detection |

## Documentary Structure

Each sub-document drills into a specific aspect of one or both projects:

### pi-mono Deep Dives (documentary/pi-mono/)

- **[00-overview.md](documentary/pi-mono/00-overview.md)** -- Project structure, three-layer architecture, package relationships
- **[01-agent-loop.md](documentary/pi-mono/01-agent-loop.md)** -- Core agent loop: outer/inner loop structure, steering, follow-up, event lifecycle
- **[02-agent-session.md](documentary/pi-mono/02-agent-session.md)** -- AgentSession: session persistence, compaction, extension system, run modes
- **[03-api-registry.md](documentary/pi-mono/03-api-registry.md)** -- Provider registry, lazy loading, stream/streamSimple dual interface
- **[04-model-catalog.md](documentary/pi-mono/04-model-catalog.md)** -- Model interface, auto-generated catalog, model resolution, pricing
- **[05-tool-system.md](documentary/pi-mono/05-tool-system.md)** -- Tool definitions, AgentTool interface, execution flow, built-in tools
- **[06-token-usage.md](documentary/pi-mono/06-token-usage.md)** -- Usage tracking, cost calculation, context token estimation
- **[07-message-handling.md](documentary/pi-mono/07-message-handling.md)** -- Message transformation, cross-provider normalization, thinking blocks
- **[08-cli-and-modes.md](documentary/pi-mono/08-cli-and-modes.md)** -- CLI arguments, run modes, configuration files, settings

### hermes-agent Deep Dives (documentary/hermes-agent/)

- **[00-overview.md](documentary/hermes-agent/00-overview.md)** -- Project structure, AIAgent class, module relationships, import chain
- **[01-main-loop.md](documentary/hermes-agent/01-main-loop.md)** -- run_conversation: iteration budget, retry logic, fallback chain, compression
- **[02-message-handling.md](documentary/hermes-agent/02-message-handling.md)** -- Message format, reasoning extraction, sanitization, budget warnings
- **[03-tool-system.md](documentary/hermes-agent/03-tool-system.md)** -- ToolRegistry, toolsets, parallel execution, checkpointing, argument coercion
- **[04-token-usage.md](documentary/hermes-agent/04-token-usage.md)** -- Session tracking, usage normalization, cost estimation, pricing lookup
- **[05-provider-system.md](documentary/hermes-agent/05-provider-system.md)** -- Provider resolution, API mode detection, credential pool, auxiliary client
- **[06-api-surface.md](documentary/hermes-agent/06-api-surface.md)** -- CLI, gateway, ACP adapter, plugin system, configuration
- **[07-error-recovery.md](documentary/hermes-agent/07-error-recovery.md)** -- Retry logic, context overflow, rate limits, credential rotation, fallback chains

### Provider Deep Dives (documentary/providers/)

- **[01-anthropic.md](documentary/providers/01-anthropic.md)** -- Anthropic provider in both projects: streaming, cache control, tool format, OAuth mode
- **[02-openai-completions.md](documentary/providers/02-openai-completions.md)** -- OpenAI Chat Completions: compat detection, streaming, tool format
- **[03-openai-responses.md](documentary/providers/03-openai-responses.md)** -- OpenAI Responses API: function_call items, composite IDs, stream events
- **[04-google.md](documentary/providers/04-google.md)** -- Google Gemini: three implementations, direct object args, thought signatures
- **[05-mistral.md](documentary/providers/05-mistral.md)** -- Mistral: 9-char ID constraint, hash-based derivation, collision handling
- **[06-bedrock.md](documentary/providers/06-bedrock.md)** -- AWS Bedrock: Converse Stream, toolSpec, toolChoice mapping
- **[07-openrouter.md](documentary/providers/07-openrouter.md)** -- OpenRouter: model routing, referer headers, 200+ models
- **[08-codex.md](documentary/providers/08-codex.md)** -- OpenAI Codex: Responses API adapter, chatgpt.com routing
- **[09-text-based-models.md](documentary/providers/09-text-based-models.md)** -- Open-source models: Hermes, Qwen, Llama, DeepSeek, Kimi, GLM text formats

### Behavioral Deep Dives (documentary/behaviors/)

- **[01-context-compression.md](documentary/behaviors/01-context-compression.md)** -- How each project manages context limits, summarization, head/tail protection
- **[02-parallel-tool-exec.md](documentary/behaviors/02-parallel-tool-exec.md)** -- Parallel vs sequential tool execution, safety analysis, path overlap detection
- **[03-iteration-budget.md](documentary/behaviors/03-iteration-budget.md)** -- Budget tracking, subagent budgets, refunded turns, threshold behavior
- **[04-reasoning-extraction.md](documentary/behaviors/04-reasoning-extraction.md)** -- Multi-format reasoning extraction: reasoning_content, thinking blocks, XML tags
- **[05-streaming-behavior.md](documentary/behaviors/05-streaming-behavior.md)** -- Streaming preferences, health checking, stale-stream detection, fallback to non-streaming
- **[06-session-persistence.md](documentary/behaviors/06-session-persistence.md)** -- JSONL vs SQLite session storage, fork/resume, compaction summaries
- **[07-checkpointing.md](documentary/behaviors/07-checkpointing.md)** -- Filesystem snapshots before destructive operations, restore flow
- **[08-credential-failover.md](documentary/behaviors/08-credential-failover.md)** -- Multi-credential pools, failover strategies, cooldown periods, rotation
- **[09-model-fallback-chains.md](documentary/behaviors/09-model-fallback-chains.md)** -- Ordered provider fallbacks, legacy vs new format, exhaustion handling

## Key Patterns Extracted from Both Projects

### 1. Unified Internal Representation

Both projects maintain a single internal representation of tool calls, messages, and usage data. Provider-specific formats are translated to/from this internal representation at the boundary.

```
Provider Format  ->  Internal Representation  ->  Provider Format
  (Anthropic)          (ToolCall, Message)         (OpenAI)
  (OpenAI)             (Usage, StopReason)         (Google)
  (Google)             (Tool, ToolResult)           (Bedrock)
```

This pattern is critical for foundation_ai's ToolFormatter system (feature 04-tool-calling-formatter).

### 2. Provider-Specific ID Normalization

Every provider has different tool call ID constraints:
- Anthropic: `[a-zA-Z0-9_-]+`, max 64 chars
- Mistral: exactly 9 alphanumeric chars (hash-based)
- OpenAI Responses: composite `call_id|item_id`, item must start with "fc"
- Google: no IDs returned, must generate locally
- Bedrock: max 64 chars, sanitized

The pattern: each provider builds its own normalizer callback that transforms IDs during message construction.

### 3. Streaming Tool Call Assembly

All streaming providers follow the same pattern:
1. Receive delta chunk with partial tool call data
2. Accumulate partial JSON fragment into a buffer
3. Call `parseStreamingJson(buffer)` for best-effort parse
4. Emit `toolcall_start` / `toolcall_delta` / `toolcall_end` events
5. Final arguments assembled when stream completes

The difference is in the delta format: OpenAI uses `tool_calls[index].function.arguments`, Anthropic uses `input_json_delta.partial_json`, Google returns complete objects non-streamingly.

### 4. Context Compression with Protection Zones

Both projects protect certain conversation regions from compression:
- **Head**: first 3 messages (system prompt, identity, initial context) -- never compressed
- **Tail**: last ~20 messages or token budget -- recent conversation context
- **Middle**: summarizable via auxiliary LLM call

pi-mono uses `completeSimple()` for summarization; hermes-agent uses a dedicated auxiliary model resolution chain.

### 5. Credential Failover with Cooldown

hermes-agent implements a `PooledCredential` system:
- Multiple API keys for the same provider
- Strategies: `fill_first`, `round_robin`, `random`, `least_used`
- Exhausted credential cooldown: 1 hour for 429, 24 hours for other errors
- Automatic rotation when rate-limited

pi-mono uses per-provider env vars with OAuth support but no explicit pooling.

### 6. Tool Argument Coercion

LLMs frequently return typed values as strings. Both projects implement coercion:
- `"42"` -> `42` (integer)
- `"3.14"` -> `3.14` (float)
- `"true"` -> `true` (boolean)
- Uses JSON Schema type information from tool definitions

### 7. Role Alternation Enforcement

Anthropic and some other providers require strict `user/assistant/user/assistant` alternation. Both projects enforce this during message transformation by:
- Merging consecutive messages of the same role
- Stripping orphaned tool_use/tool_result blocks
- Inserting placeholder content for empty assistant messages

### 8. Error Recovery Hierarchy

Both projects implement multi-level error recovery:
1. **Retry with backoff** (3 retries, exponential)
2. **Fallback provider** (ordered chain)
3. **Credential rotation** (next key in pool)
4. **Context compression** (if context overflow)
5. **Context length probing** (binary search for max tokens)
6. **Abort** (unrecoverable error)

## What foundation_ai Can Learn

### From pi-mono:
1. **Three-layer architecture** cleanly separates: provider abstraction (pi-ai) -> agent loop (pi-agent-core) -> application (coding-agent)
2. **Event-driven agent loop** with `AgentEvent` protocol enables TUI, print, JSON, and RPC modes from the same core
3. **Auto-generated model catalog** from `scripts/generate-models.ts` keeps hundreds of models in sync
4. **Message transformation layer** (`transformMessages`) with provider-specific normalizer callbacks is elegant
5. **Extension system** with hooks: `beforeToolCall`, `afterToolCall`, system prompt injection

### From hermes-agent:
1. **Credential pooling** with failover strategies and cooldown is production-hard
2. **Auxiliary model resolution** for side tasks (compression, vision, extraction) separate from primary model
3. **Context compression with head/tail protection** is well-thought-out
4. **Plugin hook system** with pre/post hooks for LLM calls, tool calls, session lifecycle
5. **Multi-platform gateway** (Telegram, Discord, Slack, etc.) shares the same AIAgent core
6. **Checkpointing** before destructive operations with restore capability

### Combined Patterns for foundation_ai:
1. Unified internal representation + provider formatters (already planned in 04-tool-calling-formatter)
2. Event-driven agent loop with streaming lifecycle
3. Credential pooling with failover
4. Context compression with protection zones
5. Tool argument coercion from JSON Schema
6. Role alternation enforcement
7. Multi-level error recovery hierarchy
8. Session persistence for resume/fork

## Implementation Implications

This reference analysis directly informs several future features:

1. **Agent Loop** -- The structure of pi-mono's outer/inner loop and hermes-agent's run_conversation loop inform how foundation_ai should structure its Valtron-based agent loop
2. **Credential Management** -- hermes-agent's PooledCredential system should be adapted for Rust
3. **Context Compression** -- Both projects' approaches inform the compression strategy
4. **Session Persistence** -- JSONL format from pi-mono, SQLite from hermes-agent
5. **Extension/Plugin System** -- Hooks for tool execution, LLM calls, session lifecycle
6. **Multi-Provider Routing** -- API registry pattern from pi-mono, credential pool from hermes-agent

## Local File Paths

- Reference (pi-mono): `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.Pi/pi-mono/`
- Reference (hermes-agent): `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.All/hermes-agent/`
- Tool Calling Formatter: `specifications/07-foundation-ai/features/04-tool-calling-formatter/feature.md`
- Foundation AI types: `backends/foundation_ai/src/types/mod.rs`
- Foundation AI errors: `backends/foundation_ai/src/errors/mod.rs`
