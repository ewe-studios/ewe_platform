# pi-mono AgentSession

## File Location

- `packages/coding-agent/src/core/agent-session.ts`
- `packages/coding-agent/src/core/sdk.ts` (high-level entry point)

## What AgentSession Is

`AgentSession` is the high-level session manager that wraps the core `Agent` class with:
- Session persistence (JSONL)
- Context compaction
- Extension system
- Tool registry
- System prompt building
- Model switching
- CLI handling
- Multiple run modes (interactive TUI, print, RPC)

It is shared by all run modes, meaning the same session logic powers the TUI, the print mode, and the RPC server.

## Services Architecture

AgentSession is composed of several manager services:

| Service | Purpose |
|---------|---------|
| `SettingsManager` | Global and project-local settings (stored in `~/.pi/agent/`) |
| `SessionManager` | JSONL session file management, fork/resume |
| `AuthStorage` | OAuth credentials and API key storage |
| `ModelRegistry` | Model resolution from CLI args, settings, or existing session |
| `ResourceLoader` | Skills, prompt templates, themes |

## Session Persistence (JSONL)

Sessions are stored as JSONL files where each line is a JSON object representing a single event or message:
- User prompts
- Assistant messages
- Tool execution events
- System events (model changes, compaction summaries)

### Session Operations
- **Create**: New session with a unique ID
- **Continue**: Load last session and append new messages
- **Resume**: Load a specific session by ID
- **Fork**: Create a new session branching from a specific message in an existing session

## Context Compaction

AgentSession triggers auto-compaction when the context exceeds the model's context window minus a reserve:

```
trigger when: context_tokens > contextWindow - reserveTokens - keepRecentTokens
```

The compaction process (see `packages/coding-agent/src/core/compaction/compaction.ts`):
1. Uses `estimateContextTokens()` -- based on last assistant message's `usage.totalTokens`, with character-based estimation for subsequent messages
2. Calls an LLM (`completeSimple()`) to generate a summary of the messages being compacted
3. Stores file operations (read/modified) in compaction entries
4. Supports branch summarization when switching git branches

## Extension System

Extensions register with AgentSession and can provide:
- **Tools**: `ToolDefinition` objects with rendering and source info
- **Commands**: Slash commands for the CLI
- **Handlers**: Event handlers for specific event types
- **Hooks**: `beforeToolCall`, `afterToolCall`, system prompt injection

Built-in extensions include:
- File tools (read, write, edit, grep, find, ls)
- Bash execution tool
- Session management commands

## System Prompt Building

The system prompt is assembled from multiple sources:
- Model-specific directives
- Skill definitions (indexed by name)
- Context files (loaded from the project)
- Memory/index references
- Ephemeral prompts (injected per turn)
- Extension-contributed system prompt additions

## Run Modes

### Interactive TUI (default when TTY)
- Full terminal UI with streaming output
- Chat-style interface with overlays for tool execution
- Animated spinner during API calls
- Slash command support with autocomplete

### Print Mode (default when piped or `--print`)
- Text or JSON output
- No interactive elements
- Suitable for scripting and piping

### JSON Mode (`--mode json`)
- Structured JSON output for programmatic consumption
- Events serialized as JSON lines

### RPC Mode (`--mode rpc`)
- JSON-RPC protocol for programmatic access
- Enables IDE integration (VS Code, Zed, JetBrains)

## Event Processing Pipeline

AgentSession processes events from the agent loop:
1. Queue events to preserve ordering
2. Persist messages to JSONL session file
3. Check auto-retry conditions (retryable errors)
4. Trigger auto-compaction when threshold exceeded
5. Notify TUI/print/RPC subscribers

## Model Switching

AgentSession supports switching models mid-session:
- CLI: `--models` flag for comma-separated scoped models
- Settings: configured models in `~/.pi/agent/models.json`
- Cycle: automatic model cycling on certain error conditions

## CLI Arguments

| Flag | Purpose |
|------|---------|
| `--provider`, `--model`, `--api-key` | Model selection |
| `--thinking` | Thinking level (off\|minimal\|low\|medium\|high\|xhigh) |
| `--continue` / `-c`, `--resume` / `-r` | Session continuation |
| `--session`, `--fork`, `--session-dir` | Session management |
| `--models` | Comma-separated scoped models for cycling |
| `--tools`, `--no-tools` | Tool allowlist/disable |
| `--extension` / `-e`, `--no-extensions` / `-ne` | Extension loading |
| `--skill`, `--prompt-template`, `--theme` | Resource loading |
| `--export` | HTML session export |
| `--list-models` | Model catalog listing |
| `--offline` | Offline mode |
| `--print` / `-p` | Print mode |

## Configuration Files

| Path | Purpose |
|------|---------|
| `~/.pi/agent/auth.json` | OAuth credentials |
| `~/.pi/agent/models.json` | Custom model configurations |
| Project `.pi/` directory | Project-local settings |

## Key Patterns for foundation_ai

1. **Service composition** over monolith -- SettingsManager, SessionManager, AuthStorage, etc. are separate concerns
2. **JSONL session format** is simple, append-only, and easy to resume/fork
3. **Auto-compaction trigger** based on token estimation is proactive, not reactive
4. **Extension system** with hooks enables third-party tool and command registration
5. **Run mode abstraction** lets the same core logic power TUI, print, JSON, and RPC
6. **Model switching mid-session** with automatic cycling on errors
