# hermes-agent API Surface

## File Locations

- CLI: `cli.py` (HermesCLI class, `process_command()`)
- CLI commands: `hermes_cli/commands.py` (CommandDef objects)
- Gateway: `gateway/run.py`
- ACP: `acp_adapter/`
- Plugin hooks: various locations

## CLI Entry Point (`cli.py`)

Interactive TUI with `prompt_toolkit`:
- Rich banner/panels, KawaiiSpinner animated faces during API calls
- Slash command autocomplete via `SlashCommandCompleter`
- `process_command()` dispatches on canonical command name resolved via `resolve_command()`
- Commands defined as `CommandDef` objects in `hermes_cli/commands.py`

### CLI Subcommands (`hermes_cli/`)

| Command | Purpose |
|---------|---------|
| `setup` | Initial configuration wizard |
| `config` | Configuration management |
| `models` | Model listing and selection |
| `skills` | Skill management |
| `tools` | Tool configuration |
| `auth` | Authentication and API key management |

## Gateway (`gateway/run.py`)

Main loop for messaging platforms:

### Supported Platforms

| Platform | Integration Type |
|----------|-----------------|
| Telegram | Bot API |
| Discord | Bot API |
| Slack | Bot API |
| WhatsApp | Business API |
| Signal | Signal CLI |
| Home Assistant | Integration |
| Email | IMAP/SMTP |
| Mattermost | Bot API |
| Matrix | Bot API |
| DingTalk | Bot API |
| Feishu | Bot API |
| WeCom | Bot API |
| SMS | SMS gateway |
| Webhook | HTTP webhook |

### Gateway Features

- **Session store** for conversation persistence per user/channel
- **Background process watcher** for `terminal(background=true)` commands
- **Status system** with token locks for multi-profile support
- Same `AIAgent` core shared across all platforms

## ACP Adapter (`acp_adapter/`)

VS Code / Zed / JetBrains integration via JSON-RPC:
- Provides coding-focused tools without messaging/audio UI
- Request/response pattern for prompts
- Streaming responses via JSON-RPC notifications
- Session management via ACP commands

## Plugin System

Hooks available at key lifecycle points:

| Hook | When Fired | Purpose |
|------|------------|---------|
| `pre_llm_call` | Before API call | Inject context, modify messages |
| `post_api_request` | After API call completes | Log, transform response |
| `pre_api_request` | Before API call sent | Modify request params |
| `pre_tool_call` | Before tool execution | Modify args, block, cache |
| `post_tool_call` | After tool execution | Transform result, record metrics |
| `on_session_start` | New session begins | Initialize state, load context |

Plugins can inject context into user messages via the `pre_llm_call` hook.

## Configuration

### User Configuration

| Path | Purpose |
|------|---------|
| `~/.hermes/config.yaml` | User settings |
| `~/.hermes/.env` | API keys and secrets |

### Profiles

Multiple isolated instances via `HERMES_HOME` environment variable:
- Each profile has its own config, sessions, and tools
- Enables running multiple agents with different configurations

### Config Structure

```yaml
# ~/.hermes/config.yaml
model: claude-sonnet-4-20250514
provider: openrouter
max_iterations: 90
thinking_level: medium
tools: ["core", "hermes-cli"]
plugins: []
memory_provider: null
```

## Key Patterns for foundation_ai

1. **Multi-platform gateway** shares the same AIAgent core -- Telegram, Discord, Slack, etc.
2. **Plugin hooks** at lifecycle points enable extensibility without modifying core code
3. **Profile isolation** via `HERMES_HOME` enables multiple agents with different configs
4. **ACP adapter** provides IDE integration via JSON-RPC
5. **CLI with slash commands** and autocomplete via `prompt_toolkit`
6. **Background process watcher** for async terminal commands
7. **Session store** per user/channel in gateway mode
