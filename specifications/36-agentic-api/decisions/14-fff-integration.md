# Decision 14: File Search Integration (fff + Vector Search)

**Status:** Accepted  
**Date:** 2026-06-12  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

> **RESOLVED (2026-06-15, F18/F22):** Split into **`search()`** (semantic/memory/graph/hybrid — knowledge, F18/F22) vs **`search_file()`** (filesystem via fff, F22). The two tools never call each other; Decision 14's old two-phase fallback is superseded. `search_file` is native-only, target-gated off wasm.
- search(...): to support semantic search of vector store, memory and old messages, graph search e.g code graphs we've generated which can tell us which file has this given block of code or entity.
- search_file(...): to use fff  to search the file system.

The agent needs to search the filesystem for files and content. Two complementary search strategies are needed:
1. **Literal/pattern-based search** — "find all files containing `auth_check(`"
2. **Semantic search** — "what did I say before about authentication?"

## Decision

File search uses a **two-phase strategy**: fff first (real filesystem truth), then vector search (memory/observations) for contextual recall.

### Search Order

```
Agent needs to find information
├── Phase 1: fff (filesystem search)
│   ├── "Which file defines auth_check?" → fff finds real files
│   ├── "Where is the error handler?" → fff searches content + paths
│   └── If fff finds nothing or low-confidence results:
│       └── Phase 2: Vector search (semantic recall)
│           ├── "What did I learn about auth yesterday?" → observation memory
│           ├── "What was the user's preference for error handling?" → working memory
│           └── "What files did I search last time?" → previous search records in vector store
```

### When to Use Each

| Query Type | Strategy | Rationale |
|-----------|----------|-----------|
| "Which file has `X`?" | fff only | Real filesystem truth, no memory needed |
| "Where is `Y` defined?" | fff only | Literal code search |
| "What did I say about Z?" | Vector only | Memory recall, not filesystem |
| "What files should I check for W?" | fff → vector fallback | fff first, then previous search records |
| "What was the approach we decided last time?" | Vector only | Reflection/observation memory |
| "Show me all usages of `fn foo`" | fff only | Literal grep across codebase |

### fff Integration

fff is integrated as:

1. **Direct call from Context API** — when the agent needs filesystem search, Context API calls fff directly (not as a valtron task)
2. **Agent tool** — exposed as `fff_search` tool the agent can invoke explicitly
3. **Not a valtron task** — fff runs synchronously, returns results immediately

```rust
pub struct FffSearch {
    /// fff instance, native-only
    inner: FffInstance,
    /// Root directory to search
    root: PathBuf,
}

impl FffSearch {
    /// Search file content matching the query
    pub fn grep(&self, query: &str) -> Result<Vec<FffMatch>>;
    
    /// Search for files matching a path pattern
    pub fn find(&self, pattern: &str) -> Result<Vec<FffFileEntry>>;
    
    /// Multi-file grep across a workspace
    pub fn multi_grep(&self, query: &str, paths: &[String]) -> Result<Vec<FffMatch>>;
}

pub struct FffMatch {
    pub path: String,
    pub line_number: usize,
    pub content: String,
    pub score: f64,  // frecency-adjusted relevance
}
```

### Vector Search Fallback

When fff returns no results or low-confidence results, the Context API falls back to vector search:

```rust
impl ContextProvider {
    pub fn search(&self, query: &str, mode: SearchMode) -> SearchResult {
        match mode {
            SearchMode::Filesystem => {
                // fff first
                let fff_results = self.fff_search.grep(query);
                if !fff_results.is_empty() {
                    return SearchResult::from_fff(fff_results);
                }
                // Vector fallback — check if we've searched for this before
                let vector_results = self.vector_store.query(query, top_k=10);
                SearchResult::from_vector(vector_results)
            }
            SearchMode::Memory => {
                // Vector only — semantic recall from working/observation/reflection memory
                let vector_results = self.vector_store.query(query, top_k=10);
                SearchResult::from_vector(vector_results)
            }
        }
    }
}

pub enum SearchMode {
    Filesystem,  // fff → vector fallback
    Memory,      // vector only
}
```

### Platform Support

| Platform | fff | Vector Search | Strategy |
|----------|-----|--------------|----------|
| **Native (Linux/macOS/Windows)** | ✅ Yes | ✅ Yes | fff → vector fallback |
| **WASM (CF Workers, browser)** | ❌ No | ✅ Yes | Vector only |
| **WASM (native file access)** | ❌ No | ✅ Yes | Vector only |

fff uses:
- `heed` (LMDB bindings) — native only
- `memmap2` — memory-mapped files, native only
- `notify` — filesystem watcher, native only
- `git2` with vendored-libgit2 — native only
- `rayon` — multi-threaded, native only

**For WASM environments**, the agent relies entirely on vector search. The Context API detects the platform and skips fff when unavailable.

### Agent Tool Definition

fff is exposed as an agent tool:

```rust
pub struct FffTool {
    pub name: "fff_search",
    pub description: "Search file content and paths in the filesystem. \
                      Use for: finding files, searching code content, \
                      locating definitions. Not for: memory recall or \
                      semantic search — use vector search for those.",
    pub arguments: Args {  // JSON Schema
        query: String,
        mode: enum["grep", "find", "multi_grep"],
        paths: Option<Vec<String>>,
    },
}
```

### Integration with Observation Memory

When the agent performs fff searches, the results are recorded in observation memory:

```
Observation entry:
  type: assertion
  content: "Agent searched for `auth_check(` using fff, found 3 matches in auth.rs, middleware.rs, handler.rs"
  source_files: ["auth.rs", "middleware.rs", "handler.rs"]
  search_query: "auth_check("
  result_count: 3
```

This allows vector search to later recall: "the agent searched for auth_check before and found it in these files."

### Integration with Reflection Memory

When reflections are generated, previous search patterns are summarized:

```
Reflection entry:
  summary: "Session focused on authentication flow — auth_check found in auth.rs, middleware.rs, handler.rs. Agent searched for auth-related code multiple times."
  search_summary: {
    total_searches: 5,
    unique_queries: ["auth_check", "login handler", "session validation"],
    most_searched_files: ["auth.rs", "handler.rs"],
  }
```

## Rationale

**Why fff first, then vector?**
- fff searches the real filesystem — always accurate, always current
- Vector search recalls past context — useful when fff finds nothing
- Two-phase ensures we check reality first, then memory

**Why not a valtron task?**
- fff runs synchronously and returns immediately — no progress states needed
- Context API calls fff directly during context assembly
- No need for valtron's execution model for a fast, synchronous search

**Why expose as an agent tool?**
- Agent can explicitly search when needed — not just automatic
- Tool definition provides guidance on when to use fff vs vector search
- Consistent with other tools (read_file, bash, etc.)

## Alternatives Considered

### Vector search only (no fff)
- **Pros:** Simpler, WASM-compatible
- **Cons:** No real filesystem search — can't find actual file content
- **Rejected because:** Agent needs to search the codebase, not just memory

### fff only (no vector)
- **Pros:** Fast, accurate filesystem results
- **Cons:** No memory recall — agent can't remember past searches or observations
- **Rejected because:** Agent needs both real filesystem AND memory recall

### fff as a valtron task
- **Pros:** Progress reporting, cancellation
- **Cons:** Overkill for fast synchronous search — fff returns in milliseconds
- **Rejected because:** fff is fast enough to call directly from Context API
