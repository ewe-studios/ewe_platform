# Social Media Agent -- Content Generation Pipeline

Takes a URL and generates Twitter/LinkedIn posts. Uses HITL flow for platform auth and post review.

## Documents

- [00 Architecture](00-architecture.md) — 14 graphs covering ingest, generate, upload, reflection, threading, curation, verification, and repurposing

## Key Graphs

| Graph | Purpose |
|-------|---------|
| `ingest_data` | Fetch from Slack |
| `generate_post` | Main post generation |
| `upload_post` | Post to Twitter/LinkedIn |
| `reflection` | Learn from accepted/rejected posts |
| `generate_thread` | Multi-post Twitter threads |
| `curate_data` | Curate from AI news sources |
