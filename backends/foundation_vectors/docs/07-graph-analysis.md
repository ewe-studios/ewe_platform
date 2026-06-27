# Fundamentals 07 — Graph analysis: god nodes & surprising connections

> **Scope:** analysis is the **F27c follow-on** (it builds on clustering, Doc 06).
> Concept primer here so F27c is well-grounded.

Once you have a clustered graph, a few cheap analyses turn it from "a data
structure" into "insight an agent can act on".

---

## 1. God nodes (centrality hubs)

A **god node** is a node with disproportionately high degree — usually
**in-degree** (lots of things depend on it). `god_nodes(k)` returns the top-`k` by
degree.

Why the agent cares:

- **Risk** — a high-in-degree node is a blast radius: changing it touches
  everything that calls/uses it. "Before you edit `Config`, note 200 things depend
  on it."
- **Orientation** — god nodes are the load-bearing walls of the codebase; listing
  them is a fast "what matters here" summary.
- **Caveat** — *false* god nodes from naive member-call resolution are exactly what
  Doc 04 §3 guards against. Real god nodes come from a clean graph; that's why the
  member-call exclusion is a prerequisite for trustworthy analysis.

Centrality has fancier forms (betweenness, PageRank), but degree centrality is
cheap, robust, and answers the agentic question well. F27c starts there.

## 2. Surprising connections (cross-community edges)

After clustering (Doc 06), most edges stay *within* a community. The interesting
ones are the few that **cross** community boundaries — a function in the parser
community that calls into the storage community. These **cross-community edges**
are "surprising connections": architectural seams, leaky abstractions, or the
genuinely important integration points.

`surprising_connections()` = edges whose endpoints have different community ids,
ranked by weight/confidence. They're often the most informative thing to show an
agent reasoning about how subsystems actually talk.

## 3. Suggested questions

graphify also derives **suggested questions** from the structure — "What does the
`auth` community depend on?", "Why does `parser` reach into `storage`?" — seeded by
god nodes and surprising connections. For our agent this is optional sugar: the
real value is the underlying god-node and cross-community signals, which the
agent's own planner can turn into questions.

## 4. Why analysis is deferred to F27c

All of this depends on **clustering** (community ids) and is most valuable once the
graph spans a real repo. F27a deliberately ships the **extraction + query** core
first (the thing the `search()` tool needs to answer "which file / what calls"),
and leaves clustering + analysis to F27c. The query surface (`find_entity`,
`callers_of`, budgeted `neighborhood`) already answers the high-frequency agentic
questions without any clustering; god-node/surprising-connection analysis is the
next layer of insight, not a prerequisite.

---

**Next:** Doc 08 — querying & token budgeting (the F27a query surface in depth).
