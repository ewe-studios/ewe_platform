# Fundamentals 09 — The optional LLM semantic pass

> **Scope:** the LLM pass is the **F27c follow-on** and is **optional**. Code-only
> corpora skip it entirely. This doc explains what it adds, when it's worth it, and
> the one rule that keeps it honest.

The deterministic AST pass (Docs 01–04) gets you everything *structurally present*.
A model can find edges the syntax tree can't — at a cost. This is that trade-off.

---

## 1. What the AST pass cannot see

Some relationships aren't in any single file's syntax:

- **Semantic cross-file calls** the AST resolver won't risk — e.g. a call through a
  trait object or dynamic dispatch where the concrete target isn't name-matchable.
- **Shared-data / conceptual coupling** — two modules that operate on the same
  logical entity without a literal call or import.
- **Doc-level concepts** — "this is the retry subsystem" as a relationship, not a
  symbol.

These need *understanding*, not parsing. That's the LLM pass's niche.

## 2. The third confidence tier: AMBIGUOUS

The AST pass emits only **EXTRACTED** and **INFERRED** (Doc 00 §3). The LLM pass is
the *only* source of **AMBIGUOUS** edges — relationships the model proposes but
can't be sure of. Keeping AMBIGUOUS strictly LLM-sourced means a consumer can
always tell deterministic facts from model guesses by the confidence tier alone.

## 3. How graphify runs it (the reference)

- Model: Claude `claude-sonnet-4-6` (temp 0) or Kimi `kimi-k2.6`,
  `max_tokens = 8192`, processed in **20-file chunks**.
- System prompt (verbatim): *"extract a knowledge graph fragment… Output ONLY valid
  JSON… EXTRACTED/INFERRED/AMBIGUOUS… node id `{stem}_{entity}`…"*.
- Rules the prompt enforces: for code files, find **semantic** edges the AST can't —
  **don't re-extract imports** (the AST already has them); `calls` edges are always
  `source = caller, target = callee` (never reversed); rationale is an **attribute**,
  not a node.

## 4. How *our* port runs it (the important divergence)

In this platform the LLM pass is **not a bespoke API client**. It routes through
the agentic loop's own **`ModelProviderRouter` (F12)** / memory model — the same
provider abstraction everything else uses. Benefits: one place for credentials,
routing, fallback, and budget accounting (F04); the code-graph doesn't grow its own
model integration. The node-id scheme stays `make_id(stem, entity)` (Doc 02b) so
LLM-proposed nodes merge cleanly with AST nodes.

## 5. When to skip it (usually)

For the agentic "which file defines X / what calls Y" use case — the reason F27
exists — the **deterministic AST pass is sufficient and is the default**. The LLM
pass is opt-in, costs tokens, and is non-deterministic (so it breaks the
"re-running gives identical graphs" property that caching relies on, Doc 10). Reach
for it only when structural extraction demonstrably misses edges you need, and
treat its AMBIGUOUS output accordingly.

## 6. Why it's F27c, not F27a

It depends on the provider router (F12, already built) and adds non-determinism and
cost to an otherwise free, reproducible pipeline. The structural core (F27a) must
stand on its own first — and it does. Layering the optional semantic pass on top is
a clean follow-on, not a foundation.

---

**Next:** Doc 10 — incremental update & caching (F27a).
