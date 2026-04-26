---
title: "Pi Extensions -- pi-messenger"
---

# pi-messenger

**Multi-agent chat room -- agents in different terminals can talk to each other.**

pi-messenger lets multiple Pi sessions sharing a folder communicate like they're in a chat room. Join, see who's online and what they're doing. Claim tasks, reserve files, send messages. No daemon, no server -- just files.

## Installation

```bash
pi install npm:pi-messenger
```

## Crew Agents

Crew agents ship with the extension (`crew/agents/*.md`) and are discovered automatically. The `pi-messenger-crew` skill is auto-loaded from the extension. Workers can load domain-specific crew skills on demand during task execution.

## How It Works

All sessions point to a shared folder. Each session writes messages to files in that folder. Other sessions read the files and respond. It's a file-based pub/sub system -- zero network dependencies.

## Use Cases

| Scenario | Description |
|----------|-------------|
| Task coordination | Claim tasks, report progress |
| Research sharing | One agent researches, others implement |
| Code review | Agent A writes, Agent B reviews |
| Multi-domain work | Each agent handles its specialty |

## Architecture

```
Session A (research) ──► shared/messages/ ├── msg_001.md (from A)
Session B (implement) ◄─── shared/messages/ ├── msg_002.md (from B)
Session C (review)   ◄──── shared/messages/ └── msg_003.md (from C)
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-messenger` |
| Transport | Shared folder (file-based) |
| Daemon | None required |
| Pattern | Chat room between sessions |
