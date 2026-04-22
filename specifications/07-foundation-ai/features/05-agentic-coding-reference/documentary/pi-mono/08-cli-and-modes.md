# pi-mono CLI and Run Modes

## File Locations

- CLI entry: `packages/coding-agent/src/main.ts`
- CLI arguments: `packages/coding-agent/src/cli/args.ts`
- Settings: `packages/coding-agent/src/core/settings-manager.ts`
- Session manager: `packages/coding-agent/src/core/session-manager.ts`
- Resource loader: `packages/coding-agent/src/core/resource-loader.ts`

## CLI Entry Point

The main entry point (`main.ts`) performs three steps:
1. Parse CLI arguments
2. Detect run mode (interactive/print/rpc)
3. Read piped stdin if present

Run mode detection:
- If `--mode` is explicitly set, use that
- If stdout is a TTY, default to interactive
- If stdout is piped, default to print
- If `--print` flag is set, force print mode

## Run Modes

### Interactive Mode (default when TTY)

Full terminal UI with:
- Streaming output as the agent responds
- Chat-style interface with tool execution overlays
- Animated indicators during API calls
- Slash command support with autocomplete
- Color-coded output for different message types

### Print Mode (default when piped or `--print`)

Text or JSON output:
- No interactive elements
- Assistant responses printed as plain text
- Tool execution shown as text descriptions
- Suitable for scripting and piping to other commands

### JSON Mode (`--mode json`)

Structured JSON output:
- Each event serialized as a JSON line
- Includes message deltas, tool execution events, usage data
- Suitable for programmatic consumption

### RPC Mode (`--mode rpc`)

JSON-RPC protocol for programmatic access:
- Enables IDE integration (VS Code, Zed, JetBrains)
- Request/response pattern for prompts
- Streaming responses via JSON-RPC notifications
- Session management via RPC commands

## CLI Arguments

| Flag | Short | Purpose |
|------|-------|---------|
| `--provider` | | Provider name (e.g., "anthropic", "openai") |
| `--model` | | Model ID (e.g., "claude-opus-4-5") |
| `--api-key` | | API key override |
| `--thinking` | | Thinking level: off\|minimal\|low\|medium\|high\|xhigh |
| `--continue` | `-c` | Continue the last session |
| `--resume` | `-r` | Resume a specific session by ID |
| `--session` | | Session ID to use |
| `--fork` | | Fork from a specific session |
| `--session-dir` | | Custom session directory path |
| `--models` | | Comma-separated model list for cycling |
| `--tools` | | Tool allowlist |
| `--no-tools` | | Disable all tools |
| `--extension` | `-e` | Load an extension |
| `--no-extensions` | `-ne` | Disable all extensions |
| `--skill` | | Load a skill |
| `--prompt-template` | | Load a prompt template |
| `--theme` | | Load a theme |
| `--export` | | Export session as HTML |
| `--list-models` | | List available models |
| `--offline` | | Offline mode (no API calls) |
| `--print` | `-p` | Force print mode |

## File Arguments

The CLI accepts file arguments that are read and included in the initial prompt:
- `--file <path>` -- Read file contents and include in prompt
- Stdin piped input -- Read from stdin and include in prompt

This enables patterns like:
```bash
cat some-code.ts | pi-mono "explain this code"
pi-mono --file README.md "summarize this project"
```

## Configuration Files

| Path | Purpose |
|------|---------|
| `~/.pi/agent/auth.json` | OAuth credentials (github-copilot, etc.) |
| `~/.pi/agent/models.json` | Custom model configurations |
| `~/.pi/agent/settings.json` | Global settings |
| Project `.pi/` directory | Project-local settings |

## SettingsManager

The settings manager handles configuration at two levels:
1. **Global**: Stored in `~/.pi/agent/`
2. **Project-local**: Stored in `.pi/` in the project root

Settings include:
- Default provider and model
- Thinking level
- Tool configuration
- Extension enablement
- Theme preferences

## ResourceLoader

The resource loader manages loadable resources:
- **Skills**: Prompt templates and tool-use guidance per skill
- **Prompt templates**: Custom system prompt templates
- **Themes**: CLI theming configuration (data-driven, no code changes needed)

## SessionManager

The session manager handles JSONL session files:
- Create new sessions with unique IDs
- List recent sessions
- Load session for continue/resume
- Fork sessions at specific message points
- Export sessions to HTML

## Key Patterns for foundation_ai

1. **Run mode detection** based on TTY vs pipe is a good default with explicit override
2. **File arguments** enable natural patterns like piping code into the agent
3. **Settings at two levels** (global + project-local) allow project-specific overrides
4. **Resource loading** (skills, templates, themes) is separate from core logic
5. **JSONL session format** enables easy export, fork, and resume operations
6. **RPC mode** enables IDE integration without modifying the core agent loop
