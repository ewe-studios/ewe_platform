# Decision 01: Session Architecture

**Status:** Accepted  
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
├── 1. Load WorkingMemory (user identity, preferences, dislikes, expectations)
├── 2. Load memory snapshots (observations → reflections)
├── 3. Load last 10 messages from MessageAPI (understand where we stopped)
├── 4. Semantic recall via MessageAPI vector store (load references to relevant conversations)
├── 5. Assemble full context: system prompt → WorkingMemory → ReflectionMemory
│                         → last 10 messages → semantically recalled messages
├── 6. PriorityQueue and FollowUpQueue start empty (messages were persisted + cleared on end)
└── 7. ToolCallManager starts fresh (all tool calls cancelled before session ended)

Session ends
├── All buffered messages flushed to disk
├── PriorityQueue messages persisted to MessageAPI, queue cleared
├── FollowUpQueue messages persisted to MessageAPI, queue cleared
├── All in-progress tool calls cancelled, partial results persisted
├── WorkingMemory snapshot persisted
├── ObservationMemory snapshot persisted (if changed)
├── ReflectionMemory snapshot persisted (if changed)
└── Embedded vectors flushed
```

### Session Resume Protocol

The resume protocol is deterministic — given a SessionId, context is reconstructed in this exact order:

**Step 1: Load WorkingMemory**
- Load latest working memory snapshot from DocumentStore (`session:{id}:memory:working`)
- Contains: user identity, preferences, dislikes, expectations, key facts
- If no snapshot exists (first resume after crash), WorkingMemory is empty

**Step 2: Load Memory Snapshots**
- Load observation memory from DocumentStore (`session:{id}:memory:observation`)
- Load reflection memory from DocumentStore (`session:{id}:memory:reflection`)
- If observation exists but reflection doesn't: observations are used directly (transitional state)
- If neither exists: memory is empty, agent starts fresh with just messages

**Step 3: Load Last 10 Messages**
- Query MessageAPI: `recent(10)` — returns the last 10 messages from the session
- These provide immediate context about where the conversation left off
- Fixed count of 10: enough for continuity, small enough for context budget

**Step 4: Semantic Recall**
- Use MessageAPI vector store to search for relevant older messages
- Query is derived from WorkingMemory + recent messages context
- Results are references to message IDs — loaded on demand for fast file seeks
- Number of recalled messages depends on remaining context window after steps 1-3

**Step 5: Assemble Full Context**
- System prompt (instructions, tool definitions)
- WorkingMemory (always present, ~500 tokens)
- ReflectionMemory (summarized observations, ~3k-5k tokens)
- Last 10 raw messages (immediate context)
- Semantically recalled messages (fill remaining context window)

**Step 6: Queues Start Empty**
- PriorityQueue: all messages were persisted to MessageAPI before session ended
- FollowUpQueue: all messages were persisted to MessageAPI before session ended
- No queued messages survive a resume — the agent starts with a clean slate

**Step 7: ToolCallManager Starts Fresh**
- All tool calls were cancelled and their results persisted before session ended
- On resume, ToolCallManager has an empty queue and no in-progress calls
- If a tool call was mid-execution during crash: partial results are in MessageAPI, the tool call is not re-executed

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

> **RESOLVED (2026-06-15, F00 + F01):** SessionId folds a **machine id into the scru128 entropy region** (F01 `from_name`/`new`). The `foundation_rng` crate is **folded into `foundation_compact`** (F00); scru128 ids live in `foundation_compact::ids`. Session resume is detailed in F31.

## Implications

- All downstream components (Messages, Context, ToolCallManager, queues) receive a `SessionId` and derive their own storage keys from it
- Session listing is a simple sort operation on the ID space — no database query needed
- Session archival is a single directory/file deletion by SessionId prefix
- Session replay is deterministic — the same SessionId always produces the same message sequence
