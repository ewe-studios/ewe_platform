---
title: Social Media Agent -- Source Architecture
---

# Social Media Agent -- Source Architecture

## Purpose

Social Media Agent takes a URL and generates Twitter and LinkedIn posts. Uses human-in-the-loop (HITL) flow for platform auth and post review/approval. Supports cron-based ingestion from Slack, content curation, thread generation, and content repurposing.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/social-media-agent/`

## Aha Moments

**Aha: 14 graphs in one project.** Each graph handles a specific concern — ingest, generate, upload, reflect, thread, curate, verify, supervisor, report, repurpose, and HITL variants. This is a full content pipeline, not a single agent.

**Aha: URL deduplication via LangGraph store.** Used URLs stored in LangGraph store to prevent duplicate content generation across runs.

**Aha: Reflection learns from accepted/rejected posts.** The reflection graph analyzes which posts were accepted vs rejected and updates its generation strategy accordingly.

## Architecture

TypeScript project with 14 LangGraph graphs:

| Graph | Source | Purpose |
|-------|--------|---------|
| `ingest_data` | `src/agents/ingest-data/` | Fetch messages from Slack, call generate_post |
| `generate_post` | `src/agents/generate-post/` | Main post generation |
| `upload_post` | `src/agents/upload-post/` | Post to Twitter/LinkedIn |
| `reflection` | `src/agents/reflection/` | Learn from accepted/rejected posts |
| `generate_thread` | `src/agents/generate-thread/` | Multi-post Twitter threads |
| `curate_data` | `src/agents/curate-data/` | Curate from AI news sources |
| `verify_reddit_post` | `src/agents/verify-reddit-post/` | Verify Reddit content |
| `verify_tweet` | `src/agents/verify-tweet/` | Verify tweet content |
| `supervisor` | `src/agents/supervisor/` | Supervisor orchestration |
| `generate_report` | `src/agents/generate-report/` | Marketing reports |
| `repurposer` | `src/agents/repurposer/` | Content repurposing |
| `curated_post_interrupt` | `src/agents/curated-post-interrupt/` | HITL for curated posts |
| `ingest_repurposed_data` | `src/agents/ingest-repurposed-data/` | Repurposed content ingestion |
| `repurposer_post_interrupt` | `src/agents/repurposer-post-interrupt/` | HITL for repurposed posts |

## Key Components

### Post Generation (`src/agents/generate-post/`)

Content verification → link verification (general, GitHub, YouTube, Tweet sub-graphs) → report generation → human node for HITL review.

### Data Curation (`src/agents/curate-data/`)

Loaders from AI news blogs, GitHub trending, Latent Space, Reddit, Twitter.

### Upload (`src/agents/upload-post/`)

Platform upload logic for Twitter and LinkedIn via authenticated clients.

### Memory (`memory-v2/`)

Separate Python-based memory graph (`memory-v2/memory_v2/graph.py`) for learning from post outcomes.

## Features

- **URL deduplication** — stores used URLs in LangGraph store
- **Content relevancy verification** — checks URLs against business context
- **URL exclusion** — LangChain-specific URL blacklist
- **HITL review** — human approves posts before publishing
- **Cron ingestion** — scheduled Slack message ingestion

[Back to main index → ../README.md](../README.md)
