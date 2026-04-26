---
title: "Pi Extensions -- pi-boomerang"
---

# pi-boomerang

**Token-efficient autonomous task execution with automatic context collapse.**

pi-boomerang lets the agent execute long autonomous tasks without burning context. When the task completes, the entire exchange collapses to a brief summary -- work gets done, tokens get saved.

## How It Works

```
/boomerang Fix the login bug
```

The agent executes autonomously: reads files, makes edits, runs tests. When done, it returns a summary. The LLM only sees:

```
[BOOMERANG COMPLETE]
Task: Fix the login bug
Summary: Found null pointer in AuthController.java:42. Added null check.
Tests passed: 12/12
```

## Why It Exists

Long autonomous tasks consume massive context. A bug fix that reads 10 files, makes 5 edits, and runs tests might burn 50k tokens. With pi-boomerang, the token cost is just the prompt + summary, not the entire intermediate thought process.

## Context Collapse

The key innovation: instead of keeping every intermediate step in the conversation, boomerang collapses the entire autonomous execution into a concise report. This preserves the ability to review what happened without paying the token cost of watching it unfold.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-boomerang` |
| Trigger | `/boomerang <task>` |
| Best for | Long autonomous tasks, batch operations |
