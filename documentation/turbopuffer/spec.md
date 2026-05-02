# Turbopuffer Documentation — Spec

## Source Codebase

- **Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.turbopuffer/`
- **Language:** Rust (primary), Go, Python, TypeScript, Kotlin/Java (SDKs)
- **License:** Mixed (various crates have different licenses)
- **Website:** https://turbopuffer.com

## What the Project Is

Turbopuffer is a serverless vector and full-text search database built on object storage (S3/GCS). It combines approximate nearest neighbor (ANN) vector search via SPFresh (centroid-based index) with BM25 full-text search, native metadata filtering, and aggregations. The source code here contains the official SDKs (Go, Python, TypeScript, Java), a semantic code search CLI tool (`turbogrep`), an API code generator (`turbopuffer-apigen`), and benchmark tooling.

## Documentation Goal

A reader should understand:
1. Turbopuffer's architecture: compute-storage separation, object storage as source of truth, three-tier caching
2. The WAL (write-ahead log) pattern and how it enables durability without a consensus plane
3. SPFresh centroid-based ANN index vs graph-based alternatives (HNSW, DiskANN)
4. BM25 full-text search and inverted index design on object storage
5. Native filtering and how it avoids recall loss from post-filtering
6. The `turbogrep` CLI: tree-sitter chunking, Voyage AI embeddings, sync/search pipelines
7. SDK architecture across 4 languages (Stainless-generated + apigen-supplemented)
8. How S3/object storage is used efficiently: small ranged reads, columnar layout, cache hierarchy
9. Pricing model, system limits, and production scale numbers
10. Consistency model: strong by default, eventual for sub-10ms warm queries

## Documentation Structure

```
turbopuffer/
├── spec.md                     ← Project tracker
├── markdown/
│   ├── README.md               ← Index / table of contents
│   ├── 00-overview.md          ← What turbopuffer is, philosophy, architecture at a glance
│   ├── 01-architecture.md      ← Storage engine, cache hierarchy, compute-storage separation
│   ├── 02-storage-s3.md        ← S3 efficiency: WAL, columnar format, ranged reads, compression
│   ├── 03-vector-index.md      ← SPFresh centroid-based ANN index
│   ├── 04-full-text-search.md  ← BM25, inverted indexes, FTS v2, MAXSCORE/WAND
│   ├── 05-native-filtering.md  ← Attribute indexes, native vs post-filtering
│   ├── 06-consistency.md       ← Strong vs eventual consistency, WAL batching
│   ├── 07-api-and-sdks.md      ← REST API, SDK generation, turbolisp query syntax
│   ├── 08-turbogrep.md         ← Semantic code search CLI: chunking, embeddings, sync, search
│   ├── 09-pricing-and-limits.md ← Pricing tiers, system limits, regions
│   └── 10-performance.md       ← Benchmarks, cold vs warm latency, production scale
├── html/                       ← Generated HTML
└── build.py                    ← Shared build script (parent directory)
```

## Tasks

| # | Document | Status |
|---|----------|--------|
| 1 | spec.md | DONE |
| 2 | README.md (index) | DONE |
| 3 | 00-overview.md | DONE |
| 4 | 01-architecture.md | DONE |
| 5 | 02-storage-s3.md | DONE |
| 6 | 03-vector-index.md | DONE |
| 7 | 04-full-text-search.md | DONE |
| 8 | 05-native-filtering.md | DONE |
| 9 | 06-consistency.md | DONE |
| 10 | 07-api-and-sdks.md | DONE |
| 11 | 08-turbogrep.md | DONE |
| 12 | 09-pricing-and-limits.md | DONE |
| 13 | 10-performance.md | DONE |
| 14 | Grandfather review | DONE |
| 15 | HTML generation | DONE |

## Build System

```bash
cd documentation && python3 build.py turbopuffer
```

Shared `documentation/build.py` — Python 3.12+ stdlib only, no external dependencies.

## Quality Requirements

Follows the Iron Rules from `documentation/markdown_engineering/documentation_directive.md`:
1. Detailed sections with code snippets from source
2. Teach key facts quickly — first sentence is thesis
3. Clear articulation — one idea per sentence
4. Minimum 2 mermaid diagrams per document
5. Tables, ASCII art, code blocks as appropriate
6. HTML generation with navigation
7. Cross-references between documents
8. Source path references (file:line)
9. Aha moments — non-obvious design decisions
10. Index + Prev/Next navigation (build.py handles)

## Expected Outcome

A reader can understand turbopuffer's architecture deeply enough to evaluate it against alternatives (Pinecone, Milvus, Weaviate), use the SDKs correctly, and understand why the object storage design makes it 10x cheaper.

## Resume Point

All documents complete. Grandfather review done, all fixes applied. HTML builds cleanly.
