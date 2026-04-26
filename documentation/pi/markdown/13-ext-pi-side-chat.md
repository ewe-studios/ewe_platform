---
title: "Pi Extensions -- pi-side-chat"
---

# pi-side-chat

**Fork the current conversation into a side chat while the main agent keeps working.**

pi-side-chat lets you fork the conversation into a side chat without interrupting the main thread. You're in the middle of a longer task and want to ask something small without derailing progress -- check an API detail, sanity-check an approach, search something, or peek at what the main agent is doing.

## Quick Start

```bash
pi install npm:pi-side-chat
```

Open the overlay, ask a question, close it. The main thread never gets interrupted.

## How It Works

The extension creates a fork of the current conversation at the current point. The side chat has its own context, its own model calls, and its own tool execution. When you close the side chat, the main agent's work continues exactly where it left off.

## Use Cases

| Scenario | Description |
|----------|-------------|
| Quick questions | "What's the TypeScript type for this?" |
| Sanity checks | "Does this approach look reasonable?" |
| Research | "Look up the API docs for X" |
| Monitoring | Peek at what the main agent is doing |

## Why It Exists

Interrupting a long-running agent task is expensive -- you lose context, the agent needs to reorient, and progress stalls. Side chat preserves the main thread's momentum while giving you a place to ask tangential questions.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-side-chat` |
| Pattern | Conversation fork with isolation |
| Interrupt | None -- main thread unaffected |
