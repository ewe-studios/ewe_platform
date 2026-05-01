# Conductor -- Production Agent Architecture

Durable execution workflows for AI agents — every step is a checkpoint.

## Documents

- [00 Production Agent Architecture](00-production-agent-architecture.md) — Canonical agent pattern, DO_WHILE loop, durable checkpoints, human approval, compensation

## Core Principles

1. **Every step is a durable checkpoint.** Server restarts don't lose work.
2. **The agent loop is a workflow pattern.** `DO_WHILE` + `LLM_CHAT_COMPLETE` + `SWITCH` = production agent.
3. **Compensation handles side effects.** Failed workflows can undo actions.
4. **Observability is automatic.** Every task's I/O is recorded.

## Quick Pattern

```
Input: goal, mcpServerUrl, maxIterations
  → LIST_MCP_TOOLS (discover available tools)
  → SET_VARIABLE (initialize memory)
  → DO_WHILE loop:
      → LLM_CHAT_COMPLETE (plan next action)
      → SWITCH (done? needs_approval? execute?)
      → CALL_MCP_TOOL (execute)
      → SET_VARIABLE (update memory)
  → Output: answer, iterations, actions_taken
```
