# Coding Practice: Agent Orchestration

## Purpose
This rule establishes the mandatory practice for code development and building, requiring the use of multiple specialized agents orchestrated by a main controller agent.

## Rule

### Main Agent Role (Controller/Orchestrator)
When building and developing code, the main agent (Claude) **MUST**:
- **Act as a controller and orchestrator ONLY**
- **NEVER perform coding tasks directly**
- **Launch specialized agents** to handle all code development work
- **Review and consolidate** results from specialized agents
- **Coordinate** multiple agents working in parallel or sequence
- **Report** final consolidated results to the user

### Specialized Agent Requirements
When launching agents for code development:

#### Mandatory Agent Count
- Launch **as many agents as needed**, up to a maximum of **10 concurrent agents**
- Each agent should handle a specific, well-defined task or component

#### Agent Independence
- Each agent **MUST operate in its own context window**
- Agents work independently and report back to the main orchestrator
- No direct communication between specialized agents (all coordination through main agent)

#### Agent Initialization
Before performing any work, each specialized agent **MUST**:
1. **Read the `AGENTS.md` file** to understand core rules and guidelines
2. **Load all rules** from `.agents/rules/*` directory
3. **Follow all established rules and guidelines** from those files
4. **Understand the project context** and conventions

#### Agent Reporting
Each specialized agent **MUST**:
- **Summarize its work** upon completion
- **Report final output** back to the main orchestrator agent
- **Include relevant details** (files modified, decisions made, issues encountered)
- **Provide clear status** (success, partial completion, blocked, failed)

## Workflow

### Typical Development Process
```
User Request
    ↓
Main Agent (Orchestrator)
    ├─→ Agent 1: Task A (reads AGENTS.md + rules) → Reports back
    ├─→ Agent 2: Task B (reads AGENTS.md + rules) → Reports back
    ├─→ Agent 3: Task C (reads AGENTS.md + rules) → Reports back
    └─→ Agent N: Task N (reads AGENTS.md + rules) → Reports back
    ↓
Main Agent Reviews & Consolidates
    ↓
Final Report to User
```

### Parallel vs Sequential Execution
- **Launch agents in parallel** when tasks are independent
- **Launch agents sequentially** when tasks have dependencies
- **Maximum 10 agents** can be active at any given time

## Examples

### Good Practice ✅
```
User: "Implement user authentication feature"

Main Agent:
1. Breaks down into tasks:
   - Backend API endpoints
   - Database models
   - Frontend components
   - Tests
   - Documentation

2. Launches 5 agents in parallel:
   - Agent 1: Backend API (reads AGENTS.md first)
   - Agent 2: Database models (reads AGENTS.md first)
   - Agent 3: Frontend components (reads AGENTS.md first)
   - Agent 4: Tests (reads AGENTS.md first)
   - Agent 5: Documentation (reads AGENTS.md first)

3. Reviews all agent reports
4. Consolidates results
5. Reports to user
```

### Bad Practice ❌
```
User: "Implement user authentication feature"

Main Agent:
1. Directly writes backend code
2. Directly writes frontend code
3. Directly writes tests
4. Reports to user

❌ Violation: Main agent performed work directly
```

## Rationale
- **Scalability**: Multiple agents can work in parallel, improving efficiency
- **Specialization**: Each agent can focus on a specific aspect of the task
- **Context Management**: Each agent has its own context window for deep focus
- **Quality**: All agents follow the same rules and guidelines from AGENTS.md
- **Oversight**: Main agent can review and ensure consistency across all work
- **Separation of Concerns**: Clear distinction between orchestration and execution

## Exceptions
This rule applies to **all code development and building tasks**. There are no exceptions.

For non-coding tasks (research, file reading, answering questions), the main agent may work directly.

## Enforcement
Any violation of this rule (main agent performing coding work directly) should be:
1. Immediately stopped
2. Restarted with proper agent orchestration
3. Reported to the user as a rule violation

---
*Created: 2026-01-11*
