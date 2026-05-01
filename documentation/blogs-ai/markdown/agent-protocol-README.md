# Agent Protocol -- Standardized API for LLM Agents

Framework-agnostic API specification for serving LLM agents in production.

## Documents

- [00 Specification](00-specification.md) — Runs, Threads, Store primitives, streaming protocol (CDDL), server implementation

## Core Primitives

1. **Run** — A single agent execution
2. **Thread** — Multi-turn session organizing related runs
3. **Store** — Long-term memory with namespace-based access

## Streaming Channels

`messages` | `tools` | `lifecycle` | `input` | `values` | `updates` | `checkpoints` | `tasks` | `custom`
