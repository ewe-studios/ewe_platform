# Graphify -- Data Flow

This document traces the five primary data flows through graphify, from initial corpus detection through query and MCP server. Each flow is illustrated with a sequence diagram using actual function names and data shapes from the source code.

Related: [Overview](00-overview.md) -- [Caching](10-caching-performance.md) -- [LLM Backend](12-llm-backend.md)

## Flow 1: Full Graph Build (`graphify .`)

A full build processes the entire corpus through all seven pipeline stages. This is the most common entry point.

```mermaid
sequenceDiagram
    participant User
    participant CLI as graphify CLI
    participant D as detect.py
    participant E as extract.py
    participant C as cache.py
    participant B as build.py
    participant Cl as cluster.py
    participant A as analyze.py
    participant R as report.py
    participant X as export.py
    participant Out as graphify-out/

    User->>CLI: graphify .
    CLI->>D: detect(root=".")
    D-->>CLI: {"files": {"code": [...], "document": [...], ...}, "total_files": N, "total_words": W}

    CLI->>E: extract(code_files, cache_root=root)
    loop Each code file
        E->>C: load_cached(path, kind="ast")
        alt cache hit
            C-->>E: cached result
        else cache miss
            E->>E: tree-sitter AST parse
            E->>C: save_cached(path, result, kind="ast")
        end
    end
    E-->>CLI: {"nodes": [...], "edges": [...], "hyperedges": [], "input_tokens": 0, "output_tokens": 0}

    CLI->>B: build_from_json(extraction)
    B-->>CLI: nx.Graph (nodes + edges)

    CLI->>Cl: cluster(G)
    Cl-->>CLI: {cid: [node_ids], ...}

    CLI->>A: god_nodes(G), surprising_connections(G, communities)
    A-->>CLI: {"gods": [...], "surprises": [...], "questions": [...]}

    CLI->>R: generate(G, communities, cohesion, labels, gods, surprises, detection, tokens, root)
    R-->>CLI: str (GRAPH_REPORT.md content)

    CLI->>X: to_json(G, communities, "graph.json")
    CLI->>X: to_html(G, communities, "graph.html")

    CLI->>Out: write graph.json, graph.html, GRAPH_REPORT.md, .graphify_root
    Out-->>User: Done: N nodes, M edges, K communities
```

The data shapes flowing between stages are:

| Stage Output | Type | Key Fields |
|-------------|------|-----------|
| `detect()` | `dict` | `files.code`, `files.document`, `total_files`, `total_words` |
| `extract()` | `dict` | `nodes[]`, `edges[]`, `hyperedges[]`, `input_tokens`, `output_tokens` |
| `build_from_json()` | `nx.Graph` | Nodes with `id`, `label`, `file_type`, `source_file`; Edges with `source`, `target`, `relation`, `confidence` |
| `cluster()` | `dict` | `{community_id: [node_ids]}` |
| `analyze()` | multiple | God nodes (high degree), surprising connections (cross-community), suggested questions |
| `generate()` | `str` | Full `GRAPH_REPORT.md` markdown |
| `to_json()` / `to_html()` | files | `graph.json`, `graph.html` |

## Flow 2: Incremental Update (`graphify update .`)

An incremental update detects which files have changed, re-extracts only those files, and merges the results into the existing graph.

```mermaid
sequenceDiagram
    participant User
    participant CLI as graphify CLI
    participant D as detect.py
    participant C as cache.py
    participant E as extract.py
    participant B as build.py
    participant Cl as cluster.py
    participant A as analyze.py
    participant R as report.py
    participant X as export.py
    participant Out as graphify-out/

    User->>CLI: graphify --update .
    CLI->>D: detect(root=".")
    D-->>CLI: {"files": {"code": [...], "document": [...], ...}, ...}

    CLI->>C: cached_files(root)
    C-->>CLI: set of SHA256 hashes for cached files

    CLI->>C: check_semantic_cache(doc_files)
    C-->>CLI: (cached_nodes, cached_edges, cached_hyperedges, uncached_files)

    CLI->>E: extract(uncached_code_files, cache_root=root)
    E-->>CLI: {"nodes": [...], "edges": [...], ...}

    Note over CLI: Merge cached + fresh results

    CLI->>B: build_from_json(merged_extraction)
    B-->>CLI: nx.Graph

    CLI->>Cl: cluster(G)
    CLI->>A: analyze(G, communities)
    CLI->>R: generate(G, ...)
    CLI->>X: to_json(G, ...), to_html(G, ...)

    CLI->>Out: overwrite graph.json, graph.html, GRAPH_REPORT.md
    Out-->>User: Update done: only changed files re-extracted
```

Key difference from full build: `check_semantic_cache` (`cache.py:149`) splits document/paper/image files into cached (hash unchanged) and uncached (new or modified). Only uncached files go through LLM extraction. Code files are re-extracted via AST only, using `load_cached` with `kind="ast"`.

## Flow 3: Graph Query (`graphify query "question"`)

A query loads the built graph, scores nodes against the question, traverses a relevant subgraph, and returns token-budgeted context.

```mermaid
sequenceDiagram
    participant User
    participant CLI as graphify CLI
    participant Sec as security.py
    participant J as json_graph
    participant Q as query/benchmark
    participant Out as graph.json

    User->>CLI: graphify query "how does authentication work"
    CLI->>Sec: validate_graph_path("graphify-out/graph.json")
    Sec-->>CLI: resolved Path (or raise ValueError)

    CLI->>Out: read graph.json
    Out-->>CLI: raw JSON string
    CLI->>J: node_link_graph(data)
    J-->>CLI: nx.Graph

    CLI->>Q: score nodes against question terms
    Q-->>CLI: scored [(score, node_id), ...]

    CLI->>Q: BFS from top-3 nodes (depth=3)
    Q-->>CLI: visited set + edge list

    CLI->>Q: format as text (NODE/EDGE lines)
    Q-->>CLI: "NODE AuthHandler src=auth.py ..."

    CLI->>User: print subgraph text (within token budget)
```

The scoring logic (`benchmark.py:18-26`) tokenizes the question into terms (words longer than 2 characters), then counts how many terms appear in each node's label. The top 3 scoring nodes serve as BFS seeds. The traversal collects neighbors up to `depth=3` hops, formatting nodes and edges as readable text lines.

## Flow 4: CLI Install (`graphify install`)

The install command copies the skill file to the target platform's configuration directory and sets up hooks.

```mermaid
sequenceDiagram
    participant User
    participant CLI as graphify CLI
    participant Plat as platform.py
    participant FS as filesystem
    participant Platform as Claude Code / Cursor / etc.

    User->>CLI: graphify install
    CLI->>Plat: detect_platform()
    Plat-->>CLI: platform name (claude, cursor, gemini, ...)

    CLI->>FS: find skill.md source
    CLI->>FS: copy skill.md to platform config dir
    FS-->>CLI: skill.md installed

    alt platform needs hook
        CLI->>FS: append to .agents/AGENTS.md or CLAUDE.md
    end

    CLI->>User: "graphify installed for <platform>"
```

Platform detection checks for known config files/directories (e.g., `~/.claude/`, `.cursor/`). The skill file (`skill.md`) contains the full graphify prompt and instructions for Claude Code subagents.

## Flow 5: MCP Server Query

The MCP server exposes graph traversal as a tool call, enabling external AI agents to query the graph programmatically.

```mermaid
sequenceDiagram
    participant Agent as External AI Agent
    participant MCP as graphify MCP server
    participant Sec as security.py
    participant G as graph.json
    participant Traversal as BFS/DFS traversal

    Agent->>MCP: start_server()
    MCP-->>Agent: MCP endpoint ready

    Agent->>MCP: tool call: query_graph(question, depth, token_budget)
    MCP->>Sec: validate_graph_path("graphify-out/graph.json")
    Sec-->>MCP: resolved Path

    MCP->>G: load graph.json -> nx.Graph
    G-->>MCP: nx.Graph with N nodes, M edges

    MCP->>MCP: score nodes against question
    MCP->>Traversal: BFS from top nodes (depth=N)
    Traversal-->>MCP: subgraph nodes + edges

    MCP->>MCP: format within token_budget
    MCP-->>Agent: JSON result: nodes, edges, context text
```

The MCP server uses the same scoring and traversal logic as the CLI query (`benchmark.py:16-52`), but returns structured JSON instead of formatted text. The `validate_graph_path` call ensures the server cannot be tricked into reading files outside `graphify-out/`.

## Cross-Cutting Concerns

### Caching in Every Flow

Every flow that reads or writes files interacts with the cache:

- **Flow 1**: `save_cached` after first extraction; `load_cached` on re-run
- **Flow 2**: `check_semantic_cache` splits cached vs. uncached; `save_semantic_cache` after LLM extraction
- **Flow 3**: No cache -- reads the pre-built `graph.json`
- **Flow 4**: No cache -- filesystem copy
- **Flow 5**: No cache -- reads `graph.json` per query

### Security Guards in Every Flow

Every flow that touches external data passes through security checks:

- **Flows 1-2**: `validate_extraction` validates LLM output before `build_from_json`
- **Flows 3, 5**: `validate_graph_path` prevents path traversal on graph.json load
- **Ingestion** (not shown): `validate_url` + `safe_fetch` protect all URL fetches

## Source Files

- `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.Graphify/graphify/graphify/cache.py` -- Per-file extraction cache
- `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.Graphify/graphify/graphify/benchmark.py` -- Token-reduction benchmark (query subgraph traversal)
- `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.Graphify/graphify/graphify/watch.py` -- Filesystem watcher with incremental rebuild
- `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.Graphify/graphify/graphify/security.py` -- Path validation for graph file loading
