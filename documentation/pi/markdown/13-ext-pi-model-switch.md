---
title: "Pi Extensions -- pi-model-switch"
---

# pi-model-switch

**Let the Pi agent switch models autonomously.**

pi-model-switch gives the agent the ability to list, search, and switch models on its own. You can tell the agent "switch to a cheaper model" or "use Claude for this task" and it handles the model change itself, without you needing to use `/model` or keyboard shortcuts.

## How It Works

The extension provides a `switch_model` tool that the agent can call. The agent sees available models, their capabilities, and can select the right one for the current task.

## Installation

```bash
pi install npm:pi-model-switch
```

Restart Pi to load the extension.

## Verify Installation

After restarting Pi, the `switch_model` tool should be available. Ask the agent to "list available models" or check the tools list.

## Use Cases

| Scenario | Agent Action |
|----------|-------------|
| Cost optimization | "Switch to a cheaper model for this simple task" |
| Capability matching | "Use Claude for this code review" |
| Performance needs | "Switch to the fastest model for this search" |

## Updating

```bash
pi update pi-model-switch
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-model-switch` |
| Tool | `switch_model` |
| Control | Agent-driven model selection |
