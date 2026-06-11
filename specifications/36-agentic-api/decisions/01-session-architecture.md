# Decision 01: Session Architecture

**Status:** Proposed  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

Every AI agent interaction needs a durable, replayable identity. Sessions must survive restarts, support continuation, and provide ordered access to all interactions that occurred within them.

## Decision

Every agent session is identified by a **SessionId** that is globally unique, time-ordered, and machine-identifiable. The SessionId is the single handle through which all session concerns (messages, memory, tool calls, context) are accessed.

### SessionId Properties

| Property | Mechanism | Why |
|----------|-----------|-----|
| Uniqueness | scru128 or similar ULID-style ID | Collision-free across machines and restarts |
| Time-ordering | Timestamp-embedded ID format | Natural chronological sorting without secondary indexes |
| Machine identity | Machine-ID embedded in ID | Global uniqueness across distributed deployments |
| Human-friendly naming | Optional user-provided name → deterministic ID derivation | Users can reference sessions by name while maintaining ordering guarantees |

### Session Lifecycle

```
SessionId created
├── Messages store initialized (lazy, write-buffered)
├── Context API initialized (WorkingMemory, ObservationMemory, ReflectionMemory)
├── ToolCallManager initialized (empty queue)
├── PriorityQueue initialized (empty)
├── FollowUpQueue initialized (empty)
└── EmbeddingProvider initialized (shared LRU cache)

Session resumed (by SessionId)
├── Messages replayed from disk
├── WorkingMemory hydrated from latest snapshot
├── ObservationMemory loaded (or regenerated from observations past threshold)
├── ReflectionMemory loaded (or regenerated from reflections past threshold)
└── ToolCallManager restored (any incomplete tool calls are cancelled)

Session ends
├── All buffered messages flushed to disk
├── WorkingMemory snapshot persisted
├── ObservationMemory snapshot persisted (if changed)
├── ReflectionMemory snapshot persisted (if changed)
└── Embedded vectors flushed
```

### Rationale

**Why a SessionId-first design?**  
- Restarting an agent is a single lookup — no state reconstruction needed
- All session data is co-located by SessionId, making garbage collection and archival simple
- Parallel sessions don't contend — each SessionId maps to independent valtron tasks
- The ID itself encodes ordering, eliminating the need for a separate "created_at" index

**Why scru128 / ULID-style IDs?**  
- Time-ordered by construction — listing sessions is just sorting IDs
- 128-bit namespace provides sufficient collision resistance
- Machine-ID embedding ensures global uniqueness in distributed deployments
- Compatible with both JSON serialization and Arrow flat encoding

**Why optional human-friendly names?**  
Users prefer naming sessions ("fix auth bug", "write API spec"). A deterministic derivation function maps `(name, timestamp, machine_id) → SessionId` so the name is recoverable while preserving ordering guarantees.

## Alternatives Considered

### UUID v4
- **Pros:** Universally supported, simple
- **Cons:** Not time-ordered — requires a separate `created_at` field and index for chronological listing
- **Rejected because:** Ordering is a first-class requirement for session listing and replay

### Sequential integer IDs
- **Pros:** Simple, naturally ordered
- **Cons:** Not globally unique, requires centralized coordination, breaks in distributed deployments
- **Rejected because:** Must work across machines and restarts without coordination

### String-based slugs only
- **Pros:** Human-readable
- **Cons:** Collision risk, not time-ordered, no global uniqueness
- **Rejected because:** Cannot guarantee uniqueness or ordering

## Implications

- All downstream components (Messages, Context, ToolCallManager, queues) receive a `SessionId` and derive their own storage keys from it
- Session listing is a simple sort operation on the ID space — no database query needed
- Session archival is a single directory/file deletion by SessionId prefix
- Session replay is deterministic — the same SessionId always produces the same message sequence
