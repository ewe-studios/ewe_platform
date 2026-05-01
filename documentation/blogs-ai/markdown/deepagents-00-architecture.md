---
title: Deep Agents -- Source Architecture
---

# Deep Agents -- Source Architecture

## Purpose

Deep Agents is a batteries-included, model-agnostic agent harness built by LangChain AI. It's inspired by Claude Code's design and attempts to understand what makes Claude Code general-purpose — then make it even more so.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/deepagents/`

## Aha Moments

**Aha: Middleware intercepts LLM calls, not just tools.** `AgentMiddleware` subclasses intercept every LLM request via `wrap_model_call()`. This enables dynamic tool filtering, system-prompt injection, and cross-turn state — capabilities plain tool functions cannot provide.

**Aha: Composite Backend routes by path prefix.** Different path prefixes route to different backends. `/large_tool_results/` → temp storage, working directory → filesystem, conversation history → separate location. Clean separation of concerns.

**Aha: Shell allow-list middleware replaces HITL for non-interactive mode.** Instead of interrupt/resume (which fragments LangSmith traces), `ShellAllowListMiddleware` rejects disallowed commands inline as error `ToolMessage` objects, keeping the entire run in a single trace.

**Aha: Prompt caching middleware is positioned between user middleware and memory.** Memory updates change the system prompt, so placing cache middleware before memory prevents cache invalidation on every memory write.

## Monorepo Structure

```
deepagents/
├── libs/
│   ├── deepagents/           # Core SDK — create_deep_agent(), middleware, backends
│   ├── cli/                  # Interactive TUI coding assistant (Textual-based)
│   ├── acp/                  # Agent Context Protocol server
│   ├── evals/                # Evaluation suite + Harbor integration
│   ├── partners/             # Integration packages (Daytona, etc.)
│   └── repl/                 # LangChain REPL
├── action.yml                # GitHub Action for CI workflows
└── deepagents-deploy.md      # Deployment documentation
```

## Core SDK (`libs/deepagents/deepagents/`)

### Entry Point (`graph.py:218`)

```python
# graph.py:218
def create_deep_agent(
    model: BaseChatModel,
    tools: Sequence[BaseTool] | None = None,
    system_prompt: str | None = None,
    middleware: Sequence[AgentMiddleware] | (),
    subagents: Sequence[SubAgent] | None = None,
    skills: Sequence[Skill] | None = None,
    memory: Sequence[MemorySource] | None = None,
    permissions: Sequence[FilesystemPermission] | None = None,
    response_format: type | None = None,
    checkpointer: Checkpointer | None = None,
    store: BaseStore | None = None,
    backend: BackendProtocol | None = None,
) -> CompiledStateGraph:
    """Create a deep agent with all built-in middleware and tools."""
```

Default model: `claude-sonnet-4-6` via Anthropic. Passing `model=None` is deprecated.

### Middleware Stack (`graph.py:551-606`)

Ordered stack, each middleware wrapping every LLM call:

| Order | Middleware | Purpose |
|-------|-----------|---------|
| 1 | `TodoListMiddleware` | Built-in `write_todos` tool |
| 2 | `SkillsMiddleware` | User-defined skills (if provided) |
| 3 | `FilesystemMiddleware` | `read_file`, `write_file`, `edit_file`, `ls`, `glob`, `grep` |
| 4 | `SubAgentMiddleware` | `task` tool for sub-agent delegation |
| 5 | `SummarizationMiddleware` | Auto-compaction when context fills |
| 6 | `PatchToolCallsMiddleware` | Message patching |
| 7 | `AsyncSubAgentMiddleware` | Remote/background subagents |
| 8 | User-provided middleware | Custom interceptors |
| 9 | Provider-specific extras | Per-model adjustments |
| 10 | `_ToolExclusionMiddleware` | Profile-based tool filtering |
| 11 | `AnthropicPromptCachingMiddleware` | Prompt cache markers (before memory!) |
| 12 | `MemoryMiddleware` | Memory source injection |
| 13 | `HumanInTheLoopMiddleware` | HITL interrupts |
| 14 | `_PermissionMiddleware` | Filesystem permissions (always last) |

### Backend Protocol (`backends/protocol.py:309`)

```python
# backends/protocol.py:309
class BackendProtocol(Protocol):
    """Unified interface for pluggable file storage."""
    def ls(self, path: str) -> list[str]: ...
    def read(self, path: str) -> str: ...
    def grep(self, path: str, pattern: str) -> list[str]: ...
    def glob(self, path: str, pattern: str) -> list[str]: ...
    def write(self, path: str, content: str) -> None: ...
    def edit(self, path: str, old: str, new: str) -> None: ...
    def upload_files(self, paths: list[str]) -> None: ...
    def download_files(self, paths: list[str]) -> None: ...
    # All methods have async counterparts: als, aread, agreg, aglob, awrite, aedit
```

**`SandboxBackendProtocol`** (line 738) extends with `execute()`/`aexecute()` for shell commands.

**Backend implementations:**
| Backend | Location | Purpose |
|---------|----------|---------|
| `StateBackend` | Default | Files stored in LangGraph state |
| `FilesystemBackend` | Local disk | Direct filesystem access |
| `LocalShellBackend` | Local + shell | Filesystem + command execution |
| `CompositeBackend` | Router | Routes path prefixes to different backends |
| `StoreBackend` | LangGraph store | Store-backed persistence |
| `LangSmithSandbox` | LangSmith | LangSmith sandbox integration |

### SubAgent Types (`middleware/subagents.py:27`)

```python
@dataclass
class SubAgent:
    """Declarative synchronous spec."""
    name: str
    description: str
    system_prompt: str

@dataclass
class CompiledSubAgent:
    """Pre-compiled runnable."""
    runnable: Runnable

@dataclass
class AsyncSubAgent:
    """Remote/background subagent (LangSmith deployments)."""
    ...
```

Default "general-purpose" subagent auto-added if none provided (`graph.py:546`).

### Summarization (`middleware/summarization.py`)

Two approaches:
1. **`SummarizationMiddleware`** — auto-compacts when token usage exceeds threshold. Older messages summarized via LLM and offloaded to backend storage.
2. **`SummarizationToolMiddleware`** — exposes `compact_conversation` tool so the agent triggers compaction on demand.

### Permissions (`middleware/permissions.py`)

`FilesystemPermission` rules control tool access per agent/subagent. First match wins. Subagents inherit parent rules unless they specify their own.

### Harness Profiles

Provider-specific customization via `_HarnessProfile`:
- Tool description overrides
- Excluded tools
- Custom base prompts
- Extra middleware

Resolved from a registry keyed by model identifier or provider name.

## CLI (`libs/cli/deepagents_cli/`)

Three modes:

1. **Interactive TUI** — Textual-based rich terminal interface with streaming
2. **Non-interactive** (`-n` flag) — headless single-task execution
3. **ACP mode** (`--acp` flag) — Agent Context Protocol server over stdio

Key design decisions:
- **Deferred heavy imports** — LangChain/LangGraph imported at point-of-use to avoid slow startup
- **Stdin pipe restoration** — after reading piped stdin, restores `/dev/tty` to fd 0 so TUI still works
- **Unicode security** — commands and URLs scanned for dangerous Unicode (homoglyphs, bidirectional text)
- **Dual system prompt paths** — interactive mode asks for plan confirmation; headless mode instructs agent to proceed autonomously

## Deployment (`deepagents-deploy.md`)

- **Convention-over-configuration** — auto-discovers `skills/`, `subagents/`, `user/`, `mcp.json` from project layout
- **Sandbox providers** — Daytona, Modal, Runloop, LangSmith Sandbox
- **User memory** — per-user writable `AGENTS.md` persisting across conversations
- **Subagent memory isolation** — each subagent gets `/memories/subagents/<name>/`
- **MCP support** — HTTP/SSE transports only (stdio rejected at bundle time)

## GitHub Action (`action.yml`)

Composite GitHub Action for CI:
- Configurable memory persistence via `actions/cache` (keyed by agent name + scope: `pr`, `branch`, `repo`)
- Skills cloned from GitHub repos
- Shell allow-list support
- Configurable timeout

[Back to main index → ../README.md](../README.md)
