---
purpose: Central location for all agents and coding rules for Claude
description: This file serves as the primary configuration and rule set for AI agents working on this project. All agents must load and follow these rules before performing any tasks.
version: 2.0.0
last_updated: 2026-01-11
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

## File Structure and Directory Purposes

### Directory Tree

```
.agents/
├── rules/              # Project-specific rules and conventions
│   ├── 01-rule-naming-and-structure.md
│   ├── 02-rules-directory-policy.md
│   ├── 03-work-commit-rules.md
│   ├── 04-coding-practice-agent-orchestration.md
│   ├── 05-git-auto-approval-and-push.md
│   ├── 06-specifications-and-requirements.md
│   ├── 07-language-conventions-and-standards.md
│   └── ...             # Additional rule files (incrementing numbers)
│
├── stacks/             # Language-specific coding standards
│   ├── javascript.md   # JavaScript/TypeScript standards and conventions
│   ├── rust.md         # Rust standards and conventions
│   ├── python.md       # Python standards and conventions
│   └── ...             # Additional language standards as needed
│
└── specifications/     # Project specifications and requirements tracking
    ├── Spec.md         # Master index of all specifications
    ├── 01-specification-name/
    │   ├── requirements.md  # Detailed requirements and conversation summary
    │   └── tasks.md         # Task tracking with checkboxes
    ├── 02-another-spec/
    │   ├── requirements.md
    │   └── tasks.md
    └── ...             # Additional specification directories

AGENTS.md              # This file - central agent configuration
CLAUDE.md              # Backward compatibility redirect to AGENTS.md
```

### Directory Descriptions

#### `.agents/rules/`

**Purpose**: Contains all project-specific rules, conventions, and workflow requirements that govern agent behavior.

**Characteristics**:

- Numbered files (01, 02, 03...) for ordered loading
- Flat structure (no subdirectories)
- Each rule in its own file
- Rules loaded in numerical order
- Later rules take precedence over earlier ones

**Key Rules**:

- **01**: Rule naming and structure policy
- **02**: Rules directory policy
- **03**: Work commit rules (commit after every change with verification status)
- **04**: Agent orchestration and mandatory code verification (iron-clad)
- **05**: Git auto-approval and push workflow (after verification)
- **06**: Specifications and requirements management
- **07**: Language conventions and coding standards

**Agent Requirements**:

- ✅ MUST read all rule files at session start
- ✅ MUST follow rules in order of precedence
- ✅ MUST comply with all rules without deviation

#### `.agents/stacks/`

**Purpose**: Contains language-specific coding standards, conventions, and best practices for each programming language used in the project.

**Characteristics**:

- One file per programming language
- Comprehensive coding standards
- Self-improving via Learning Log sections
- Examples of good and bad code
- Tool configurations and requirements

**Each Stack File Contains**:

- Setup and tool requirements
- Naming conventions
- Code organization patterns
- Documentation requirements
- Best practices and idioms
- Error handling approaches
- Testing requirements
- Performance guidelines
- Security practices
- Common pitfalls and solutions
- Good/bad code examples
- Learning Log (mistakes and learnings)
- **Code Verification Workflow** (mandatory checks before commit)

**Agent Requirements**:

- ✅ MUST only load relevant stack files(s) related to what stack they will use and nothing else
- ✅ MUST read relevant stack file(s) before writing code
- ✅ MUST follow all standards with zero deviation
- ✅ MUST update Learning Log when mistakes are made
- ✅ **MUST report to Main Agent after implementation (never commit directly)**
- ✅ **Main Agent MUST delegate to ONE verification agent per stack**
- ✅ **Verification agent MUST run all checks from stack file**
- ✅ **ONLY commit after ALL verification checks PASS**
- ❌ FORBIDDEN to deviate from documented standards
- ❌ **FORBIDDEN to commit code without verification (ZERO TOLERANCE)**

#### `.agents/specifications/`

**Purpose**: Contains all project specifications, requirements, and task tracking for features and enhancements.

**Characteristics**:

- Master index file (Spec.md) lists all specifications
- Numbered directories for each specification
- Each spec has requirements.md and tasks.md
- Requirements document the "what" and "why"
- Tasks track the "how" with checkboxes

**Requirements File (`requirements.md`)**:

- Conversation summary with user
- Detailed functional requirements
- Technical specifications
- Language stack identification
- Success criteria
- Agent instructions

**Tasks File (`tasks.md`)**:

- YAML frontmatter with counts
- Task list with checkboxes
- Tools and technologies used
- Progress tracking
- Notes and blockers

**Agent Requirements**:

- ✅ MUST create specification before starting work
- ✅ MUST document language stack in requirements
- ✅ MUST launch review agent before implementation
- ✅ MUST read both requirements.md and tasks.md
- ✅ MUST verify actual implementation (not trust checkboxes)
- ✅ MUST update tasks.md as work progresses
- ❌ FORBIDDEN to start coding without documented requirements

---

## Directory Usage Workflows

### For Rules Directory

1. **At Session Start**: Load and read all files in `.agents/rules/`
2. **When Creating Rules**: Follow Rule 01 naming conventions
3. **When Conflicts Arise**: Later rules override earlier ones
4. **When Updating**: Commit rule changes immediately

### For Stacks Directory

1. **Before Coding**: Read relevant language stack file(s)
2. **During Implementation**: Follow all documented standards
3. **After Mistakes**: Update Learning Log in stack file
4. **Before Commit**: Run all required checks for that language

### For Specifications Directory

1. **New Feature**: Create specification directory
2. **Document Requirements**: Write requirements.md with language stack
3. **Create Tasks**: Write tasks.md with all identified tasks
4. **Launch Review Agent**: Verify specifications before implementation
5. **During Work**: Update tasks.md as progress is made
6. **After Completion**: Update Spec.md master index

---

## How the Directories Work Together

The three main directories in `.agents/` form an integrated system:

### The Workflow Chain

```
┌─────────────────┐
│  .agents/rules/ │ ──► Defines HOW agents must work
└─────────────────┘
         │
         ├─► Rule 06: Requires specifications before coding
         │           Creates .agents/specifications/
         │
         ├─► Rule 07: Requires language standards
         │           References .agents/stacks/
         │
         └─► Rule 03: Requires commits after changes
                     Ensures all changes are tracked

┌─────────────────────────┐
│ .agents/specifications/ │ ──► Documents WHAT to build
└─────────────────────────┘
         │
         ├─► requirements.md: References language stack
         │                    Points to .agents/stacks/
         │
         └─► tasks.md: Lists all work items
                      Tracks progress

┌──────────────────┐
│ .agents/stacks/  │ ──► Defines HOW to write code
└──────────────────┘
         │
         ├─► Coding standards per language
         ├─► Best practices and patterns
         └─► Learning Log for improvements
```

### Example: Complete Feature Development Flow

**Step 1: Rules Define Process**

- Agent reads `.agents/rules/06-specifications-and-requirements.md`
- Learns: Must create specification before coding
- Learns: Must document language stack in requirements

**Step 2: Specification Created**

- Agent creates `.agents/specifications/05-new-feature/`
- Creates `requirements.md` with:
  ```markdown
  ## Language Stack

  - **Rust**: Backend API (see .agents/stacks/rust.md)
  - **TypeScript**: Frontend (see .agents/stacks/javascript.md)
  ```
- Creates `tasks.md` with all work items

**Step 3: Stack Standards Applied**

- Agent reads `.agents/stacks/rust.md`
- Learns: Must use `Result<T, E>`, forbidden to use `unwrap()`
- Learns: Must run `cargo fmt`, `cargo clippy`
- Agent reads `.agents/stacks/javascript.md`
- Learns: Must use TypeScript strict mode, forbidden to use `any`
- Learns: Must run `prettier`, `eslint`

**Step 4: Implementation**

- Agent writes Rust code following rust.md standards
- Agent writes TypeScript code following javascript.md standards
- Both implementations verified against their stack files

**Step 5: Learning Loop**

- Agent makes a mistake in Rust code
- Agent fixes the mistake
- Agent updates `.agents/stacks/rust.md` Learning Log
- Future agents learn from this mistake

**Step 6: Quality Assurance**

- Rule 03 requires commits after changes
- Stack files define required checks
- All checks pass before commit
- Changes tracked in git

### Benefits of This Integration

1. **Consistency**: All agents follow same rules and standards
2. **Quality**: Language-specific standards ensure code quality
3. **Traceability**: Specifications document all features
4. **Learning**: Mistakes documented in Learning Logs
5. **Automation**: Rules enforce process automatically
6. **Clarity**: Clear documentation of what, why, and how

### Key Integration Points

| From            | To              | Purpose                         |
| --------------- | --------------- | ------------------------------- |
| Rule 06         | specifications/ | Requires specification creation |
| Rule 07         | stacks/         | Requires reading standards      |
| requirements.md | stacks/         | References language standards   |
| tasks.md        | specifications/ | Tracks implementation progress  |
| Learning Log    | stacks/         | Documents lessons learned       |
| All changes     | git             | Tracked via Rule 03             |

---

When beginning work on this project:

### Initial Setup (Every Session)

1. ✅ **Load `AGENTS.md`** - Read this file (you're reading it now)
2. ✅ **Load all rule files** - Read all files in `.agents/rules/*` in numerical order
3. ✅ **Understand project structure** - Familiarize yourself with the codebase organization
4. ✅ **Review recent changes** - Check git history and current branch status

### Before Starting Any Task

1. ✅ **Identify language(s)** - Determine which programming language(s) will be used
2. ✅ **Read stack standards** - Load and read `.agents/stacks/[language].md` for each language
3. ✅ **Check for specifications** - Look for existing spec in `.agents/specifications/`
4. ✅ **Review requirements** - If spec exists, read `requirements.md` and `tasks.md`

### When Creating New Features

1. ✅ **Discuss with user** - Have requirements conversation
2. ✅ **Create specification** - Create directory in `.agents/specifications/`
3. ✅ **Document language stack** - Include language information in requirements.md
4. ✅ **Launch review agent** - Verify specifications before implementation
5. ✅ **Begin implementation** - Follow all rules and standards

### Quick Reference Checklist

- [ ] Read AGENTS.md
- [ ] Read all files in `.agents/rules/`
- [ ] Read relevant `.agents/stacks/[language].md` files
- [ ] Read specification `requirements.md` and `tasks.md` (if applicable)
- [ ] Launch review agent before implementation (if applicable)
- [ ] Follow all coding standards with zero deviation
- [ ] Update Learning Logs when you learn something new
- [ ] Commit after every significant change
- [ ] Update task progress as you work

---

_Last updated: 2026-01-11_
