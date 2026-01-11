---
purpose: Central entry point for AI agent configuration
description: Minimal configuration file that directs agents to detailed rules and standards
version: 3.0.0
last_updated: 2026-01-11
---

# Agent Configuration

## Core Principle

This file is the **entry point** for all AI agents. For detailed rules, standards, and workflows, agents **MUST** load the files referenced below.

---

## Mandatory Loading Sequence

All agents **MUST** follow this sequence at the start of every session:

1. ✅ **Read `AGENTS.md`** (this file)
2. ✅ **Load ALL rules from `.agents/rules/*`** (read all files in numerical order)
3. ✅ **Load relevant stack files from `.agents/stacks/[language].md`** (only languages you'll use)
4. ✅ **Read specification files** (if working on a specific feature)

**CRITICAL**: If conflicts arise, `.agents/rules/*` takes precedence over this file.

---

## Directory Structure

```
.agents/
├── rules/              # Detailed project rules (READ ALL OF THESE)
│   ├── 01-rule-naming-and-structure.md
│   ├── 02-rules-directory-policy.md
│   ├── 03-work-commit-rules.md
│   ├── 04-coding-practice-agent-orchestration.md
│   ├── 05-git-auto-approval-and-push.md
│   ├── 06-specifications-and-requirements.md
│   ├── 07-language-conventions-and-standards.md
│   └── ...
│
├── stacks/             # Language-specific standards (READ ONLY WHAT YOU USE)
│   ├── javascript.md   # JavaScript/TypeScript standards
│   ├── rust.md         # Rust standards
│   ├── python.md       # Python standards
│   └── ...
│
└── specifications/     # Feature specifications and requirements
    ├── Spec.md         # Master index
    ├── NN-spec-name/
    │   ├── requirements.md
    │   ├── tasks.md
    │   └── verification.md  # (transient, created on verification failure)
    └── ...

AGENTS.md              # This file (entry point)
CLAUDE.md              # Backward compatibility redirect
```

---

## What Each Directory Contains

### `.agents/rules/` - HOW Agents Must Work

**Purpose**: Defines workflow, orchestration, verification, and commit processes.

**Key Rules**:
- **Rule 01**: File naming conventions
- **Rule 02**: Directory policies
- **Rule 03**: Commit requirements (with verification status)
- **Rule 04**: Agent orchestration and mandatory verification (IRON-CLAD)
- **Rule 05**: Auto-push workflow (after verification)
- **Rule 06**: Specification management
- **Rule 07**: Language standards enforcement

**⚠️ MANDATORY**: Read ALL rule files before starting any work.

→ **For full details**: Read all files in `.agents/rules/`

### `.agents/stacks/` - HOW to Write Code

**Purpose**: Language-specific coding standards, conventions, verification workflows.

**Contains**:
- Coding standards and naming conventions
- Best practices and common pitfalls
- Verification workflow (commands to run)
- Learning Logs (self-improving)
- Tool configurations

**⚠️ MANDATORY**: Read relevant stack file(s) before writing ANY code.

→ **For full details**: Read `.agents/stacks/[language].md` for your language(s)

### `.agents/specifications/` - WHAT to Build

**Purpose**: Feature requirements, task tracking, verification reports.

**Contains**:
- `requirements.md`: What to build and why
- `tasks.md`: Task list with checkboxes and progress tracking
- `verification.md`: Detailed verification failure reports (transient)

**⚠️ MANDATORY**: Read specification files when working on a feature.

→ **For full details**: Read files in `.agents/specifications/NN-spec-name/`

---

## Quick Start Checklist

Before starting ANY work:

- [ ] Load AGENTS.md (this file)
- [ ] **Read ALL files in `.agents/rules/`** (detailed workflow rules)
- [ ] **Read relevant `.agents/stacks/[language].md`** (coding standards)
- [ ] Read specification `requirements.md` and `tasks.md` (if applicable)
- [ ] Understand verification workflow from Rule 04 and stack files
- [ ] Follow orchestration model: Main Agent delegates, specialized agents do work

---

## Critical Reminders

1. **Main Agent Role**: Orchestrator ONLY. Delegates ALL work to specialized agents. Never performs work directly.

2. **Verification Requirement**: NO code commits without verification. Implementation agents report to Main Agent → Verification agent runs checks → Specification agent updates tasks → Main Agent commits.

3. **Zero Deviation**: All standards in rules and stack files must be followed exactly. No exceptions.

4. **Delegation Always**: Main Agent never reads/writes specification files directly. Always spawns Specification Update Agent.

5. **Learning Logs**: Update stack file Learning Logs when mistakes are made or new patterns discovered.

→ **For complete details on these principles**: Read `.agents/rules/04-coding-practice-agent-orchestration.md` and `.agents/rules/07-language-conventions-and-standards.md`

---

## Where to Find Detailed Information

| Topic | Location |
|-------|----------|
| Workflow and orchestration | `.agents/rules/04-coding-practice-agent-orchestration.md` |
| Verification process | `.agents/rules/04-*` + `.agents/stacks/[language].md` |
| Commit requirements | `.agents/rules/03-work-commit-rules.md` |
| Language standards | `.agents/stacks/[language].md` |
| Specification format | `.agents/rules/06-specifications-and-requirements.md` |
| Complete verification summary | `.agents/VERIFICATION_WORKFLOW_SUMMARY.md` |

---

**Remember**: This file is just the entry point. The real details are in:
- `.agents/rules/*` (HOW agents work)
- `.agents/stacks/*` (HOW to write code)
- `.agents/specifications/*` (WHAT to build)

**MANDATORY**: Load and read all relevant files before starting work.

---

*Last updated: 2026-01-11*
*Version: 3.0.0 - Streamlined entry point, full details in referenced files*
