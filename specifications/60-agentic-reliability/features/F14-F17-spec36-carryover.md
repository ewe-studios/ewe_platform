---
feature: "F14–F18 — Spec 36 carryover (marked complete, actually stubbed)"
status: "not-started"
priority: "high"
depends_on: ["F05-F09"]
source_spec: "specifications/completed/36-agentic-api"
---

# F14–F18 — Spec 36 carryover

Spec 36 (agentic-api) is marked completed and every feature's `feature.md` reads
100% — but a **code audit** (not metadata) found several surfaces that are
slots-only or stubbed. These are the real gaps.

## Audit findings (verified in code)

| Surface | Feature.md says | Code reality |
|---------|-----------------|--------------|
| `read`/`edit`/`write`/`shell` tools | ToolShed slots (F10) | **No `ToolImpl` exists** — only `ShedTool`/`SearchContextTool`/`SearchFileTool` |
| `memory` tool | ToolShed slot (F10/F15) | **No `ToolImpl`** |
| `delegate` tool | ToolShed slot (F14) | **No `ToolImpl`** |
| `search_context` **Semantic** mode | "semantic recall over messages" (F16/F32) | **Keyword matching**, not embeddings — `search_messages` uses `keyword_score`. Real vector recall (F27/F31) never wired. |
| `search_context` **Graph** mode | code-graph recall (F27) | **Returns empty** — `// F27 deferred — return empty.` |
| `search_file` (fff) | "fff on native" (F32/F33) | **Uses `InCodeVfsSearcher`** (hand-rolled regex walk). `fff-search` is an OPTIONAL dep behind `vfs-search-fff`, which `foundation_ai` does NOT enable — the real fff engine is not compiled in. |

## F14 — `memory` tool
`ToolShed.memory` has no `ToolImpl`. Implement `MemoryTool` over
`MemoryHierarchy`/`MemoryStore`: `memory_add(fact)`, `memory_query(query)`.

## F15 — `delegate` tool
`ToolShed.delegate` has no `ToolImpl`. Implement bounded `DelegationTool` that
spawns a child `AgentSession` turn for a sub-task (depth/iteration caps).

## F16 — Real semantic recall (not keyword)
`ContextProvider::search` SemanticMode does keyword matching. Wire real
embedding-based recall: `EmbeddingProvider` embeds the query + messages, a
`VectorStore` returns nearest hits. Keep keyword as a fallback when no embedding
capability is injected. Cover offline with an in-memory embedding + vector store.

## F17 — Graph search (currently empty)
`SearchMode::Graph` returns empty (`F27 deferred`). Either wire the code-graph
recall (spec 36 F27 foundation_vectors code-graph) or, if out of scope, make it
return a clear "graph search not available" rather than silently empty.

## F18 — fff-backed file search in the agent
`foundation_ai` uses `vfs-search` (the basic `InCodeVfsSearcher`), not
`vfs-search-fff` (the real fff engine, already implemented as `FffSearcher` +
`CascadingVfsSearcher`). Enable fff for native builds so `search_file` uses the
production search engine, with `InCodeVfsSearcher` as the wasm/fallback path via
`CascadingVfsSearcher`. Verify the agent's `search_file` actually goes through
fff on native.

## Tasks
- [ ] F14 `MemoryTool` + tests; fill `memory` slot.
- [ ] F15 `DelegationTool` (bounded) + tests; fill `delegate` slot.
- [ ] F16 real embedding recall in `ContextProvider::search` (Semantic); in-memory-embed test; keep keyword fallback.
- [ ] F17 wire or honestly-disable Graph search.
- [ ] F18 enable `vfs-search-fff` for native `foundation_ai`; route `search_file` through `CascadingVfsSearcher` (fff → InCode fallback); test fff path on native.

## Done when

The audit table's "code reality" column matches "feature.md says": tools
implemented, Semantic recall is real vector search (not keyword), Graph is wired
or honestly disabled, and `search_file` uses fff on native.
