---
feature: "Context Provider & Assembly + split search()"
description: "The Context API: deterministic context assembly (system → working → reflection → recent → semantic recall), the split search() (vectors/memory/graph) vs search_file() (fff), and resolution of the observation-injection question"
status: "complete"
priority: "high"
depends_on: ["08-message-api", "07-memorystore", "28-vectorstore-trait-inmemory"]
estimated_effort: "large"
created: 2026-06-14
last_updated: 2026-06-20
author: "Main Agent"
tasks:
  completed: 12
  uncompleted: 0
  total: 12
  completion_percentage: 100%
notes: "ContextProvider with deterministic assemble() (Decision 03 order), INCON-03 resolved (observation injected only when newer than reflection), ContextConfig, AgentContext, SearchMode enum, token estimation, 8 unit tests. Semantic recall (F31), search modes (Graph/Hybrid), search_file (F32) deferred to their features."
---

# Feature 16: Context Provider & Assembly

> Implements Decision 03's context assembly + Decision 14's **split search** (TODO #6:
> `search()` = semantic/memory/graph vs `search_file()` = fff). Owns how a turn's LLM context is
> built and how the agent searches. Resolves **INCON-03** (when are observations injected?).

## WHY: Problem Statement

Each LLM call needs context assembled deterministically from the memory hierarchy + recall, within
the token budget. And the agent needs two distinct searches: **semantic/structural** (`search()` over
vectors + memory + code-graph) and **filesystem** (`search_file()` over fff). Decision 14's TODO #6
splits these (they were conflated). Decision 03's context order must be exact (replayable).

## WHAT: Solution

### Context assembly (Decision 03, exact order)

```rust
impl ContextProvider {
    /// Build the ModelInteraction context for a turn, within token budget (F04).
    fn assemble(&self, budget: &TokenLedger) -> AgentContext;
}
```

Order (Decision 03 §Context Assembly):
1. **System prompt** (instructions + tool definitions / ToolShed F10).
2. **Working Memory** (always present, ~500 tokens) — from MemoryStore (F07) fast hydrate.
3. **Reflection Memory** (summarized observations, ~3-5k) — MemoryStore.
4. **Recent raw messages** (last N) — Message API `recent(N)` (F08).
5. **Semantically recalled messages** — fill remaining budget via `search(Memory/Semantic)`.

**INCON-03 resolution (the transitional state):** observations are **NOT** injected when reflections
exist (reflections supersede them). **But** when observations exist and have **not yet been reflected
on** (the transitional window), the latest observation IS injected (else that recent context is
invisible). So: inject `reflection` always; inject `observation` only if newer than the latest
reflection (or no reflection yet). This makes Decision 03 + Decision 11 consistent.

### Split search (TODO #6)

```rust
impl ContextProvider {
    /// Semantic + memory + code-graph search (NOT the filesystem).
    fn search(&self, query: &str, mode: SearchMode, k: usize) -> SearchResult;
    /// Filesystem search via fff (native) — separate tool (F32 owns the tool surface).
    fn search_file(&self, query: &str, kind: FileSearchKind) -> SearchResult;  // native; wasm = unsupported
}
pub enum SearchMode { Semantic /*messages vectors*/, Memory /*obs/refl vectors*/, Graph /*code-graph F27*/, Hybrid /*F26 fuse*/ }
```

- `search(Semantic)` → Message API `semantic_search` (F08 → VectorStore namespace=session).
- `search(Memory)` → observation/reflection vector recall (F28 + F07).
- `search(Graph)` → code-graph `find_entity`/`callers`/`neighborhood` (F27) — "which file defines X".
- `search(Hybrid)` → F26 fusion (vector + BM25 [+ graph proximity]).
- `search_file()` → fff (native, F32). This is the Decision 14 split — `search` is knowledge,
  `search_file` is the real filesystem.

### Owns / coordinates

The ContextProvider owns the EmbeddingProvider (F31), the memory hierarchy (F15), and the search
surfaces; the Memory generation logic itself is F15 (this feature consumes its outputs + triggers it).

## Architecture

```mermaid
graph TD
    A[assemble turn] --> SYS[system + ToolShed]
    A --> WM[Working Memory - MemoryStore]
    A --> RM[Reflection Memory]
    A --> OBS{obs newer than refl?}
    OBS -->|yes| INJ[inject latest observation]
    A --> REC[recent N - Message API]
    A --> SR[semantic recall - search Memory]
    S[search] --> SEM[Semantic: messages vectors]
    S --> MEM[Memory: obs/refl vectors]
    S --> GR[Graph: code-graph F27]
    S --> HY[Hybrid: F26 fuse]
    SF[search_file] --> FFF[fff native]
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: context window management & deterministic assembly; the working/
reflection/observation hierarchy & when each is injected (the transitional-state problem); RAG &
semantic recall pipelines; **hybrid retrieval** orchestration (vector+BM25+graph); knowledge search vs
filesystem search (why split); token-budgeted context packing; replayability of assembled context.
(Task — see list.)

## HOW: Implementation Steps

1. `ContextProvider` + `assemble` (exact Decision 03 order, budget-aware via F04).
2. INCON-03: inject-observation-if-newer-than-reflection logic.
3. `search` (Semantic/Memory/Graph/Hybrid) over F08/F28/F27/F26.
4. `search_file` (fff, native; wasm unsupported — graceful).
5. Wire EmbeddingProvider (F31) for query embedding; MemoryStore (F07) hydrate.
6. Tests: assembly order + budget truncation; INCON-03 transitional injection; each search mode;
   search_file native + wasm-absent; recall correctness.

## Open Decisions

- **OD-16-1 — INCON-03:** inject observation only if newer than latest reflection (rec). Confirm.
          Sure, share with me to touch base

- **OD-16-2 — budget packing:** when recall + recent exceed budget, drop oldest recall first
  (rec) vs summarize. Rec: drop recall, keep recent + working + reflection.
        Make sense

- **OD-16-3 — graph search availability:** code-graph (F27) is native-build; on wasm, `search(Graph)`
  queries a prebuilt graph (F27 query path) or is unavailable. Rec: prebuilt-graph query if present.
        
- **OD-16-4 — search_file on wasm:** fff is native-only (Decision 14). wasm `search_file` returns a
  clear "unsupported on wasm" result. Confirm.
        - Just use basic FileSystem operations if possible via whatever vfs is there else not supported

- **OD-16-5 — ContextProvider ownership boundary: RESOLVED (user, 2026-06-15; Item #15) → keep
  separate.** F16 (ContextProvider) = assembler: reads F15's snapshots from F07 MemoryStore, packs the
  prompt in deterministic order within budget. Synchronous, on the hot path. F15 (MemoryHierarchy) =
  generator: watches counters, calls memory model, writes snapshots to F08+F07. Background valtron task.
  Handoff via F07 (F15 writes latest, F16 reads). F16 checks thresholds and tells F15 to generate; F15
  does the work. The generator is background, the assembler is hot-path — never mix them.


        

## Target Files

- `backends/foundation_ai/src/agentic/context.rs` (new)
- coordinates F04 (budget), F07 (MemoryStore), F28 (VectorStore), F31 (Embedding), F08 (Message API),
  F27 (code-graph), F26 (hybrid), F15 (memory gen), F32 (fff tool)

## Tests

```bash
cargo test -p foundation_ai -- agentic::context
cargo build -p foundation_ai --target wasm32-unknown-unknown
```

## Verification

```bash
cargo build -p foundation_ai
cargo build -p foundation_ai --target wasm32-unknown-unknown
cargo clippy -p foundation_ai -- -D warnings
cargo test  -p foundation_ai -- agentic::context
```

## Done When

- `assemble` builds context in the exact Decision 03 order within budget; INCON-03 transitional
  injection resolved; `search()` (semantic/memory/graph/hybrid) + `search_file()` (fff) split per
  TODO #6; wasm degrades gracefully (no fff).
- OD-16-1..5 resolved; fundamentals authored.
