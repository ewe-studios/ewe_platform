# Hermes Agent -- Architecture

## System Layers

```mermaid
flowchart TD
    subgraph "User Interfaces"
        CLI[CLI/TUI<br/>hermes_cli/]
        ACP[ACP Adapter<br/>acp_adapter/]
        GW[Gateway<br/>gateway/]
    end

    subgraph "Core"
        AGENT[AIAgent<br/>run_agent.py]
        TOOLS_REG[Tool Registry<br/>tools/registry.py]
        MODEL_TOOLS[Tool Bridge<br/>model_tools.py]
        TOOLSETS[Toolset Manager<br/>toolsets.py]
    end

    subgraph "LLM Layer"
        ANTHROPIC[Anthropic Adapter]
        BEDROCK[Bedrock Adapter]
        GEMINI[Gemini Adapter]
        CODEX[Codex Adapter]
        COPILOT[Copilot ACP Client]
    end

    subgraph "Intelligence"
        MEMORY[Memory Manager<br/>agent/memory_manager.py]
        CONTEXT[Context Engine<br/>agent/context_engine.py]
        PROMPT[Prompt Builder<br/>agent/prompt_builder.py]
        SKILLS_SYS[Skills System<br/>agent/skill_utils.py]
    end

    subgraph "Tools (40+)"
        TERM[Terminal]
        BROWSER[Browser]
        FILES[File Operations]
        WEB[Web Search/Extract]
        MEM_TOOL[Memory Tool]
        DELEGATE[Delegate/Subagent]
        MEDIA[Media/Vision]
        API_TOOLS[API Tools]
    end

    subgraph "Plugins"
        MEM_PLUGINS[Memory Providers<br/>Honcho, Mem0, Hindsight]
        CTX_PLUGINS[Context Engines]
        IMG_PLUGINS[Image Generation]
    end

    subgraph "Scheduling"
        CRON[Cron Scheduler<br/>cron/]
    end

    CLI --> AGENT
    ACP --> AGENT
    GW --> AGENT
    GW --> CRON

    AGENT --> MODEL_TOOLS
    MODEL_TOOLS --> TOOLS_REG
    TOOLS_REG --> TERM & BROWSER & FILES & WEB & MEM_TOOL & DELEGATE & MEDIA & API_TOOLS
    AGENT --> TOOLSETS

    AGENT --> ANTHROPIC & BEDROCK & GEMINI & CODEX & COPILOT
    AGENT --> MEMORY
    AGENT --> CONTEXT
    AGENT --> PROMPT
    AGENT --> SKILLS_SYS

    MEMORY --> MEM_PLUGINS
    CONTEXT --> CTX_PLUGINS
    MEDIA -.-> IMG_PLUGINS
```

## Module Dependencies

### Core → Everything Flows Through AIAgent

`run_agent.py` contains the `AIAgent` class -- the central orchestrator. It's 3,600+ lines and coordinates:
- LLM calls via adapter selection
- Tool dispatch via the registry
- Memory reading/writing
- Context compression
- Prompt assembly
- Skill loading

Every user interface (CLI, gateway, ACP) creates an `AIAgent` instance and runs it.

### LLM Adapters → Provider Abstraction

Each adapter converts Hermes's internal message format to/from a provider's API:

| Adapter | Provider | API Format |
|---------|----------|-----------|
| `anthropic_adapter.py` | Anthropic | Messages API |
| `bedrock_adapter.py` | AWS Bedrock | Bedrock Messages |
| `gemini_native_adapter.py` | Google Gemini | Gemini API |
| `gemini_cloudcode_adapter.py` | Google Cloud Code | Cloud API |
| `codex_responses_adapter.py` | OpenAI Codex | Responses API |
| `copilot_acp_client.py` | GitHub Copilot | ACP Protocol |

The default path uses the OpenAI SDK's chat completions format. Adapters handle cases where a provider's API diverges significantly.

### Tool Registry → Self-Registration

Tools self-register on import:

```python
# In tools/terminal_tool.py
from tools.registry import registry

@registry.register(
    name="terminal",
    description="Execute shell commands",
    input_schema={...}
)
async def terminal_tool(params, context):
    ...
```

When `tools/__init__.py` is imported, it imports all tool modules, which triggers registration. The registry then provides tool schemas (for LLM) and dispatch (for execution).

### Gateway → Per-Platform Adapters

The gateway creates per-platform adapter instances based on configuration:

```mermaid
flowchart TD
    GW[Gateway Runner] --> CONFIG[Load Config]
    CONFIG --> ADAPTERS{Create Adapters}
    ADAPTERS --> TG[Telegram Adapter]
    ADAPTERS --> DC[Discord Adapter]
    ADAPTERS --> SL[Slack Adapter]
    ADAPTERS --> WA[WhatsApp Adapter]
    ADAPTERS --> SIG[Signal Adapter]
    ADAPTERS --> MTX[Matrix Adapter]
    ADAPTERS --> EMAIL[Email Adapter]

    TG --> SESSION_TG[User Sessions]
    DC --> SESSION_DC[User Sessions]
    SL --> SESSION_SL[User Sessions]
```

Each adapter handles platform-specific:
- Authentication
- Message format conversion
- Media upload/download
- Thread/reply handling
- Rate limiting

### Memory → Layered Architecture

```mermaid
flowchart TD
    AGENT[AIAgent] --> MANAGER[Memory Manager]
    MANAGER --> BUILTIN[Built-in Memory<br/>MEMORY.md files]
    MANAGER --> EXTERNAL[External Provider<br/>max 1 active]

    EXTERNAL --> HONCHO[Honcho]
    EXTERNAL --> MEM0[Mem0]
    EXTERNAL --> HINDSIGHT[Hindsight]

    MANAGER --> SEARCH[Session Search<br/>Past conversations]

    BUILTIN --> GLOBAL[Global MEMORY.md]
    BUILTIN --> USER_MEM[USER.md]
    BUILTIN --> SOUL[SOUL.md]
    BUILTIN --> PROJECT[.hermes.md]
```

The memory manager orchestrates one built-in memory system (markdown files) plus at most one external provider. It prefetches relevant memories before each LLM call and syncs updates after.

## Communication Patterns

### 1. Synchronous: CLI → Agent → LLM

Direct function calls. The CLI calls `AIAgent.run()`, which calls the LLM adapter, which makes HTTP requests. Streaming responses flow back through callbacks.

### 2. Async: Gateway → Agent → Platform

The gateway uses asyncio. Incoming messages trigger async agent runs. Responses are delivered back to the originating platform asynchronously.

### 3. Self-Registration: Tool Loading

Tools register themselves when their module is imported. The registry maintains a dictionary of name → handler mappings. No central configuration file lists all tools.

### 4. Plugin Discovery: Abstract Base Classes

Plugins implement abstract base classes (`ContextEngine`, `MemoryProvider`). The plugin loader discovers them by package name and instantiates the configured implementation.

## Key Files Map

```
hermes-agent/
├── run_agent.py                 AIAgent class (3,600+ lines, core orchestrator)
├── model_tools.py               Tool dispatch bridge
├── toolsets.py                  Toolset composition
├── hermes_state.py              Session state persistence
├── hermes_constants.py          Global paths and constants
├── agent/
│   ├── anthropic_adapter.py     Anthropic LLM adapter
│   ├── bedrock_adapter.py       AWS Bedrock adapter
│   ├── gemini_native_adapter.py Gemini adapter
│   ├── context_engine.py        ABC for context compression
│   ├── context_compressor.py    Default compressor implementation
│   ├── memory_manager.py        Memory orchestration
│   ├── memory_provider.py       ABC for memory providers
│   ├── prompt_builder.py        System prompt assembly
│   ├── prompt_caching.py        Anthropic cache control
│   ├── model_metadata.py        Token limits, pricing
│   ├── skill_utils.py           Skill loading and execution
│   └── credential_pool.py       API key management
├── tools/
│   ├── registry.py              Central tool registry
│   ├── __init__.py              Imports all tool modules (triggers registration)
│   ├── terminal_tool.py         Shell execution
│   ├── browser_tool.py          Browser automation
│   ├── file_tools.py            File operations
│   ├── web_search_tool.py       Web search
│   ├── memory_tool.py           Memory operations
│   ├── delegate_tool.py         Subagent spawning
│   └── ... (30+ more)
├── hermes_cli/
│   ├── main.py                  CLI entry point
│   ├── commands.py              Interactive commands
│   ├── config.py                Configuration loading
│   └── auth.py                  Authentication
├── gateway/
│   ├── run.py                   Gateway runner
│   ├── session.py               Per-user sessions
│   ├── delivery.py              Message delivery
│   └── platforms/               Platform adapters
├── cron/
│   ├── scheduler.py             Cron tick scheduler
│   └── jobs.py                  Job CRUD
└── plugins/
    ├── honcho/                  Honcho memory provider
    ├── hindsight/               Hindsight memory provider
    ├── mem0/                    Mem0 memory provider
    └── image_gen/               Image generation plugins
```
