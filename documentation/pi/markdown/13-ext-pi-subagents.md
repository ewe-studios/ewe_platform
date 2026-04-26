---
title: "Pi Extensions -- pi-subagents"
---

# pi-subagents

**Delegate tasks to subagents with chains, parallel execution, TUI clarification, and async support.**

pi-subagents lets the Pi agent delegate tasks to subagents -- child agents that run independently with their own context and tools.

## Features

- **Task delegation** -- Parent agent delegates specific tasks to subagents
- **Chains** -- Sequential subagent execution with handoff
- **Parallel execution** -- Multiple subagents run simultaneously
- **TUI clarification** -- User can clarify parameters before subagent launch
- **Async support** -- Subagents run asynchronously, parent continues working

## Installation

```bash
pi install npm:pi-subagents
```

## Integration with Prompt Templates

If you use [pi-prompt-template-model](13-ext-pi-prompt-template-model.md), you can wrap subagent delegation in a slash command:

```markdown
---
model: claude-sonnet-4-6
skill: code-review
---
```

This lets you define delegation patterns as reusable templates.

## Use Cases

| Scenario | Pattern |
|----------|---------|
| Code review | Parent delegates review to subagent |
| Research | Parallel subagents research different aspects |
| Testing | Subagent runs tests while parent continues |
| Documentation | Subagent writes docs for completed work |

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-subagents` |
| Pattern | Parent-child agent delegation |
| Execution | Sequential chains + parallel |
