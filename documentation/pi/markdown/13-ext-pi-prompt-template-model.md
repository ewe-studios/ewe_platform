---
title: "Pi Extensions -- pi-prompt-template-model"
---

# pi-prompt-template-model

**Frontmatter for model, skill, and thinking level in Pi prompt templates.**

Adds `model`, `skill`, and `thinking` frontmatter to Pi prompt templates. Define slash commands that switch to the right model, set a thinking level, inject skill context, and auto-restore your session when done.

## Example

```markdown
---
model: claude-sonnet-4-6
skill: python-debugging
thinking: high
---
```

```
/debug-python my code crashes
  → switches to Sonnet, loads python-debugging skill, agent responds
  → restores your previous model when finished
```

## Why It Exists

Each prompt template becomes a self-contained agent mode:

- `/quick-debug` spins up a cheap model with REPL skills
- `/deep-analysis` brings in extended thinking with refactoring expertise
- When the command finishes, you're back to your daily driver without touching anything

No more manually switching models, no hoping the agent picks up on the right skill. You define the configuration once, and the slash command handles the rest.

## Frontmatter Fields

| Field | Purpose | Example |
|-------|---------|---------|
| `model` | Switch to this model | `claude-sonnet-4-6` |
| `skill` | Inject this skill | `python-debugging` |
| `thinking` | Set thinking level | `high` |

## Auto-Restore

When the template command finishes, the previous model and thinking level are automatically restored. No state leaks between commands.

## Install

```bash
pi install npm:pi-prompt-template-model
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-prompt-template-model` |
| Pattern | Template-driven agent mode switching |
| Auto-restore | Yes (model + thinking level) |
