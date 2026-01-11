# Rules Directory Policy

## Purpose
This rule establishes the canonical location for all agent rules and policies.

## Rule
All agent rules, policies, and configuration files MUST be placed in the `.agents/rules/` directory.

## Restrictions
- **NO** rules shall be placed in `.claude/` directory
- **NO** rules shall be placed in any other directory location
- **NO** exceptions to this policy

## Directory Structure
```
.agents/
└── rules/
    └── [all rule files go here]
```

## Rationale
Centralizing all rules in a single, clearly defined location:
- Improves discoverability
- Prevents rule conflicts
- Ensures consistent rule management
- Makes it clear where to find and update rules

## Enforcement
This rule is self-referential and foundational. Any agent operating within this codebase must:
1. Check `.agents/rules/` for all applicable rules
2. Never look for rules in `.claude/` or other directories
3. Report any rules found outside `.agents/rules/` as misplaced

---
*Created: 2026-01-11*
