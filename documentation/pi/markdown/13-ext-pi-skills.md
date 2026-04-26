---
title: "Pi Extensions -- pi-skills"
---

# pi-skills

**Collection of skills for Pi coding agent and compatible tools.**

pi-skills is a curated collection of skills for the Pi coding agent, compatible with Claude Code, Codex CLI, Amp, and Droid.

## Installation

### pi-coding-agent

```bash
# User-level (available in all projects)
git clone https://github.com/badlogic/pi-skills ~/.pi/agent/skills/pi-skills

# Or project-level
git clone https://github.com/badlogic/pi-skills .pi/skills/pi-skills
```

### Codex CLI

```bash
git clone https://github.com/badlogic/pi-skills ~/.codex/skills/pi-skills
```

### Claude Code

```bash
git clone https://github.com/badlogic/pi-skills .claude/skills/pi-skills
```

## What Are Skills

Skills are self-contained instruction packages that teach agents domain-specific knowledge and workflows. Each skill is a directory with a `SKILL.md` file that describes:

- When the skill should be loaded
- What the agent should know about the topic
- How to perform specific tasks

## Compatibility

| Agent | Compatible |
|-------|-----------|
| Pi coding agent | Yes |
| Claude Code | Yes |
| Codex CLI | Yes |
| Amp | Yes |
| Droid | Yes |

## Package Details

| Property | Value |
|----------|-------|
| Source | github.com/badlogic/pi-skills |
| Install | `git clone` to skills directory |
| Format | `SKILL.md` files in directories |
