---
feature: "F14–F17 — Spec 36 carryover (slots left unimplemented / deferred)"
status: "not-started"
priority: "medium"
depends_on: ["F05-F09"]
source_spec: "specifications/completed/36-agentic-api"
---

# F14–F17 — Spec 36 carryover

Spec 36 (agentic-api) is marked completed, but several tool/recall surfaces were
left as `ToolShed` slots with no implementation, or explicitly deferred to
"F31/F32 wiring" that never landed. Surface them here as real features.

## F14 — `memory` tool

`ToolShed.memory: Option<MemoryTool>` (spec 36 F10/F15) has no `ToolImpl`. The
agent can't add/query long-term memory as a tool. Implement a `MemoryTool` over
the existing `MemoryHierarchy`/`MemoryStore`:
- `memory_add(fact)`, `memory_query(query)` sub-tools (the ToolShed already
  models memory sub-tools via names starting `memory_`).
- Fills the `memory` slot when a memory store capability is present.

## F15 — `delegate` tool

`ToolShed.delegate: Option<DelegationTool>` (spec 36 F14) has no `ToolImpl` —
sub-agent delegation. Implement `DelegationTool` that spawns a child
`AgentSession`/turn for a delegated sub-task and returns its result. Bounded
(depth/iteration caps) so delegation can't recurse unboundedly.

## F16 — `search_context` semantic recall wiring

Spec 36 F16 `assemble_from_memory` says *"Semantic recall — deferred to F31
(EmbeddingProvider) wiring"* and F32's `search_context` wraps it. Verify the
recall path is actually wired (not a stub): an `EmbeddingProvider` +
`VectorStore` produce real semantic hits that `search_context` returns and that
context assembly injects. If it's a stub, wire it.

## F17 — Embedding/vector recall end-to-end

Spec 36 F24–F31 (foundation_vectors + EmbeddingProvider + VectorStore) — confirm
the end-to-end path works: embed a message → store → `search_context` recalls it
→ appears in the assembled context. Cover with an offline test (an in-memory
embedding + vector store) so it doesn't need a live embedding model.

## Tasks
- [ ] F14 `MemoryTool` + tests; fill `memory` slot.
- [ ] F15 `DelegationTool` (bounded) + tests; fill `delegate` slot.
- [ ] F16 audit + wire semantic recall in `search_context`; test with in-memory embed/vector.
- [ ] F17 end-to-end embed→store→recall→context test (offline).

## Done when

The `memory` and `delegate` ToolShed slots are implemented and tested; semantic
recall is proven end-to-end offline (not a deferred stub).
