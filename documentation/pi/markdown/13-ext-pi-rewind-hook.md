---
title: "Pi Extensions -- pi-rewind-hook"
---

# pi-rewind-hook

**Record and restore exact file state rewind points across sessions.**

pi-rewind-hook records file-state rewind points, allowing restoration during `/tree` navigation and across resumed and forked sessions.

## How It Works

Rewind metadata lives in the session itself as hidden entries, so rewind history survives:
- Across forks
- Across resumes
- During tree navigation
- After compaction

Snapshot commits are kept reachable through a single git ref rather than one ref per checkpoint. Rewind points can be resolved across session lineage via `parentSession` links.

## Retention

Retention is optional and configurable. Without retention limits, exact history is kept indefinitely.

## Requirements

| Requirement | Version |
|-------------|---------|
| Pi agent | v0.65.0+ |
| Git | Required (rewind points use git refs) |
| Node.js | For installation |

## Workflow

1. Agent makes changes to files
2. Rewind hook records the state as a git snapshot
3. Navigate to any point in history via `/tree`
4. Restore file states to that point

## Screenshots

The extension shows a message selection interface for choosing a branch point, then a restore options dialog for choosing what to restore.

## Install

```bash
pi install npm:pi-rewind-hook
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-rewind-hook` |
| Requires | pi-agent >= 0.65.0, git repo |
| Storage | Git refs + session hidden entries |
| Retention | Optional configurable limits |
