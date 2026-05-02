# Turbopuffer Documentation

Serverless vector and full-text search on object storage.

## Overview

- [What is Turbopuffer](00-overview.md) — The problem, the architecture at a glance, why object storage

## Architecture & Storage

- [System Architecture](01-architecture.md) — Compute-storage separation, cache hierarchy, query/write paths
- [S3 Storage Engine](02-storage-s3.md) — WAL pattern, columnar format, ranged reads, compression efficiency
- [Vector Index: SPFresh](03-vector-index.md) — Centroid-based ANN, why not HNSW, recall guarantees
- [Full-Text Search: BM25](04-full-text-search.md) — Inverted indexes on KV storage, MAXSCORE, FTS v2

## Query & Consistency

- [Native Filtering](05-native-filtering.md) — Attribute indexes, high-recall filtered vector search
- [Consistency Model](06-consistency.md) — Strong vs eventual, WAL batching, conditional writes

## API & SDKs

- [API & SDKs](07-api-and-sdks.md) — REST API, Stainless SDKs, turbolisp query syntax, apigen

## Application: Turbogrep

- [Turbogrep CLI](08-turbogrep.md) — Semantic code search, tree-sitter chunking, sync pipeline, speculative search

## Operations

- [Pricing & Limits](09-pricing-and-limits.md) — Tiers, system limits, regions, production scale
- [Performance Benchmarks](10-performance.md) — Cold vs warm latency, benchmarks vs Lucene/Tantivy
