---
title: "Pi Extensions -- Index"
---

# Pi Extensions

The `pi-extensions/` directory contains community and official extensions for the Pi coding agent. Each extension adds new tools, skills, modes, or integrations.

## Extension Catalog

### Agent Runtimes

| Extension | What It Does |
|-----------|-------------|
| [pi_agent_rust](13-ext-pi_agent_rust.md) | High-performance Rust CLI for the Pi coding agent |
| [pi-vs-claude-code](13-ext-pi-vs-claude-code.md) | Customized Pi instances showcasing advantages over Claude Code |

### Collaboration & Messaging

| Extension | What It Does |
|-----------|-------------|
| [pi-discord](13-ext-pi-discord.md) | Discord bot integration with persistent sessions |
| [pi-intercom](13-ext-pi-intercom.md) | Direct 1:1 messaging between Pi sessions on the same machine |
| [pi-messenger](13-ext-pi-messenger.md) | Multi-agent chat room -- agents in different terminals can coordinate |

### Multi-Agent & Coordination

| Extension | What It Does |
|-----------|-------------|
| [pi-coordination](13-ext-pi-coordination.md) | Multi-agent parallel task execution with dependencies and review cycles |
| [pi-foreground-chains](13-ext-pi-foreground-chains.md) | Observable multi-agent workflows with file-based handoff |
| [pi-side-chat](13-ext-pi-side-chat.md) | Fork conversations into side chats while main agent keeps working |
| [pi-subagents](13-ext-pi-subagents.md) | Delegate tasks to subagents with chains and parallel execution |

### Tools & Skills

| Extension | What It Does |
|-----------|-------------|
| [pi-annotate](13-ext-pi-annotate.md) | Visual UI annotation -- click elements, add notes, agent fixes code |
| [pi-computer-use](13-ext-pi-computer-use.md) | macOS computer use with AX-first semantic targeting |
| [pi-interview-tool](13-ext-pi-interview-tool.md) | Interactive web form for gathering user responses to clarification questions |
| [pi-mcp-adapter](13-ext-pi-mcp-adapter.md) | Use MCP servers without burning context window tokens |
| [pi-model-switch](13-ext-pi-model-switch.md) | Let the agent switch models autonomously |
| [pi-skill-palette](13-ext-pi-skill-palette.md) | Command palette for selecting which skill to inject |
| [pi-skills](13-ext-pi-skills.md) | Collection of skills compatible with Pi, Claude Code, Codex, Amp, Droid |
| [pi-web-access](13-ext-pi-web-access.md) | Web search, content extraction, and video understanding |

### UX & Interface

| Extension | What It Does |
|-----------|-------------|
| [pi-design-deck](13-ext-pi-design-deck.md) | Multi-slide visual decision decks with high-fidelity previews |
| [pi-interactive-shell](13-ext-pi-interactive-shell.md) | Run interactive CLIs in an observable TUI overlay |
| [pi-powerline-footer](13-ext-pi-powerline-footer.md) | Powerline-style status bar with working vibes |

### Compaction & Context

| Extension | What It Does |
|-----------|-------------|
| [pi-custom-compaction](13-ext-pi-custom-compaction.md) | Swap the model and template for compaction |
| [pi-rewind-hook](13-ext-pi-rewind-hook.md) | Record and restore file state rewind points |

### Prompt & Model Control

| Extension | What It Does |
|-----------|-------------|
| [pi-prompt-template-model](13-ext-pi-prompt-template-model.md) | Frontmatter for model, skill, and thinking level in templates |

### Runtime & Deployment

| Extension | What It Does |
|-----------|-------------|
| [pi-boomerang](13-ext-pi-boomerang.md) | Token-efficient autonomous task execution with context collapse |
| [pi-coding-agent-termux](13-ext-pi-coding-agent-termux.md) | Termux port (deprecated -- upstream now supports Termux) |
| [pi-gitlab-duo](13-ext-pi-gitlab-duo.md) | GitLab Duo provider extension (Claude + GPT via AI Gateway) |
| [pi-runtime-extensions](13-ext-pi-runtime-extensions.md) | Load/unload extensions dynamically during a running session |

### Review & Quality

| Extension | What It Does |
|-----------|-------------|
| [pi-review-loop](13-ext-pi-review-loop.md) | Automated code review loop until no issues remain |

### Reference

| Extension | What It Does |
|-----------|-------------|
| [pi-package-test](13-ext-pi-package-test.md) | Reference package demonstrating Pi's package system features |
