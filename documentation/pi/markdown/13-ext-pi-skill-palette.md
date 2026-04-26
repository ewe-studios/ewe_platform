---
title: "Pi Extensions -- pi-skill-palette"
---

# pi-skill-palette

**Command palette for selecting which skill to inject with your next message.**

Pi Skill Palette gives you explicit control over which skill gets activated. Instead of relying on automatic detection based on task context, you select a skill from a palette and it gets sent alongside your next message.

## Usage

```
/skill
```

This opens a command palette showing all available skills. Select one and it gets injected with your next message.

## Why It Exists

Agents don't always know when to read their skills. Automatic detection works most of the time, but sometimes the agent misses the right skill for the task. pi-skill-palette gives you direct control -- select the skill you want, ensure it gets loaded.

## Install

```bash
pi install npm:pi-skill-palette
```

## Screenshot

The palette shows all installed skills with their descriptions, filtered by relevance to the current task.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-skill-palette` |
| Trigger | `/skill` command |
| Pattern | Explicit skill selection |
