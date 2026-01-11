---
purpose: Central location for all agents and coding rules for Claude
description: This file serves as the primary configuration and rule set for AI agents working on this project. All agents must load and follow these rules before performing any tasks.
version: 1.0.0
---

# Agent Rules and Guidelines

## Core Rules

### Rule 1: Load AGENTS.md First
**MANDATORY:** Always load and read the `AGENTS.md` file at the start of every session or task. This file contains the central rules and guidelines that govern all agent behavior in this project.

### Rule 2: Load Project-Specific Rules
**MANDATORY:** Always load and apply all rules located in `.agents/rules/*`. These files contain project-specific guidelines, conventions, and requirements that must be followed.

The rules should be loaded in the following order:
1. Read `AGENTS.md` (this file)
2. Discover all rule files in `.agents/rules/`
3. Load and apply each rule file in alphabetical order
4. If conflicts arise, rules in `.agents/rules/*` take precedence over general guidelines in this file

---

## Additional Guidelines

### Project Context
- This is the central hub for agent configuration
- All coding standards, architectural decisions, and workflow requirements should be documented here or in `.agents/rules/*`

### Agent Behavior
- Follow established patterns in the codebase
- Maintain consistency with existing code style
- Document significant changes and decisions
- Ask for clarification when requirements are ambiguous

---

## File Structure

```
.agents/
├── rules/           # Project-specific rules and conventions
│   ├── *.md        # Individual rule files
└── ...             # Other agent-related configurations

AGENTS.md           # This file - central agent configuration
CLAUDE.md           # Backward compatibility file
```

---

## Getting Started

When beginning work on this project:
1. ✓ Load `AGENTS.md` (you're reading it now)
2. ✓ Load all files in `.agents/rules/*`
3. ✓ Understand the project structure
4. ✓ Review recent changes and context
5. ✓ Begin your task following all established rules

---

*Last updated: 2026-01-11*
