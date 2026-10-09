# hermes-agent Overview

## Project Location

`/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.All/hermes-agent/`

## Technology Stack

- **Runtime**: Python 3.10+
- **Key Dependencies**: `openai`, `anthropic`, `prompt_toolkit`, `Rich`
- **Author**: Nous Research

## Architecture

hermes-agent is organized as a single large orchestrator (`AIAgent` class in `run_agent.py`, ~9000+ lines) with supporting modules for specific concerns:

```
┌───────────────────────────────────────────────────┐
│                    run_agent.py                     │
│  AIAgent: core orchestrator (~9000+ lines)         │
│  - Conversation loop, API calling, tool execution  │
│  - Context compression, token tracking, retries    │
├───────────────────────────────────────────────────┤
│  model_tools.py    │  Tool orchestration layer     │
│                    │  - get_tool_definitions()      │
│                    │  - handle_function_call()      │
│                    │  - coerce_tool_args()          │
├───────────────────────────────────────────────────┤
│  toolsets.py       │  Toolset definitions & resolve │
│                    │  - _HERMES_CORE_TOOLS           │
│                    │  - Platform-specific toolsets   │
├───────────────────────────────────────────────────┤
│  tools/registry.py │  Central ToolRegistry singleton │
│  tools/*.py        │  50+ individual tool impls      │
├───────────────────────────────────────────────────┤
│  cli.py            │  Interactive TUI               │
│  gateway/run.py    │  Messaging platform gateway     │
│  acp_adapter/      │  ACP server for VS Code/Zed    │
│  agent/            │  Agent internals               │
│  hermes_cli/       │  CLI subcommands               │
│  cron/             │  Built-in cron scheduler       │
└───────────────────────────────────────────────────┘
```

## Import Chain

The import dependency graph is strictly layered:

```
tools/registry.py (no deps)
  <- tools/*.py (import registry)
  <- model_tools.py (imports tools)
  <- run_agent.py, cli.py, batch_runner.py (import model_tools)
```

Each tool file self-registers via `registry.register()` at import time. This means importing `model_tools.py` triggers the full tool discovery chain.

## Key Modules

| Module | Purpose |
|--------|---------|
| `run_agent.py` | `AIAgent` class -- the core orchestrator with ~100+ attributes |
| `model_tools.py` | Tool orchestration: definitions, dispatch, argument coercion, async bridging |
| `toolsets.py` | Toolset definitions: core tools + platform-specific variants |
| `cli.py` | Interactive TUI using `prompt_toolkit` and `Rich` |
| `tools/registry.py` | `ToolRegistry` singleton: register, dispatch, get definitions |
| `tools/*.py` | 50+ individual tool implementations |
| `gateway/run.py` | Messaging platform gateway (Telegram, Discord, Slack, etc.) |
| `agent/prompt_builder.py` | System prompt assembly with skills, context, memory |
| `agent/anthropic_adapter.py` | OpenAI-to-Anthropic format conversion |
| `agent/context_compressor.py` | Context compression with auxiliary model |
| `agent/auxiliary_client.py` | Side task model resolution (compression, vision) |
| `agent/usage_pricing.py` | Usage normalization and cost estimation |
| `agent/memory_manager.py` | Memory provider orchestration |
| `hermes_cli/commands.py` | CLI command definitions as `CommandDef` objects |
| `cron/` | Built-in cron scheduler |
| `acp_adapter/` | ACP (Agent Communication Protocol) server for IDEs |

## Core Data Structures

| Structure | Purpose |
|-----------|---------|
| `AIAgent` | Core orchestrator class with ~100+ attributes |
| `IterationBudget` | Thread-safe iteration counter (default 90) |
| `ToolRegistry` / `ToolEntry` | Tool metadata and dispatch |
| `CanonicalUsage` | Normalized token counts across providers |
| `BillingRoute` | Provider/model/base_url routing |
| `PricingEntry` | Per-million-token pricing data |
| `CostResult` | Cost estimation with status/source |
| `ContextCompressor` | Context management with token budgets |
| `MemoryManager` | Built-in + external memory provider orchestration |
| `CheckpointManager` | Filesystem snapshots before destructive operations |
| `TodoStore` | In-memory task planning |
| `PooledCredential` | Multi-credential failover dataclass |
| `SkinConfig` | CLI theming (data-driven) |

## Entry Points

| Entry Point | File | Purpose |
|-------------|------|---------|
| CLI | `cli.py` | `hermes` command -- interactive TUI |
| Gateway | `gateway/run.py` | Messaging platform daemon |
| ACP | `acp_adapter/` | JSON-RPC server for VS Code / Zed / JetBrains |
| Batch | `batch_runner.py` | Batch processing mode |

## Key Design Principles

### 1. Monolithic Orchestrator

Unlike pi-mono's three-layer approach, hermes-agent uses a single large `AIAgent` class that orchestrates everything. This makes the codebase easier to understand as a whole but harder to extract individual concerns.

### 2. OpenAI-Format Internal Representation

All tools, messages, and responses use OpenAI format internally. Provider-specific formats (Anthropic, etc.) are converted to/from OpenAI format at the boundary via the `anthropic_adapter.py`.

### 3. Self-Registering Tools

Each tool file calls `registry.register()` at import time. This means tool discovery is automatic -- just add a file to `tools/` and it will be discovered.

### 4. Multi-Platform Gateway

The same `AIAgent` core powers:
- Interactive CLI (TUI)
- Telegram bot
- Discord bot
- Slack bot
- WhatsApp bot
- Signal bot
- Home Assistant integration
- Email processing
- ACP server for IDEs

### 5. Credential Pooling

Multi-API-key failover with strategies (fill_first, round_robin, random, least_used), cooldown periods, and automatic rotation.
