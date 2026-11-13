# Spec 71 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-10-30: spec written. The Durable Streams research note
  (`specifications/research/edge-data/durable-streams.md`) is the input; the six features turn it
  into `foundation_core::valtron::durable`. No feature started yet (0/6).

## Feature status

- **01-durable-stream-types** — ⏳ not started (0/N)
- **02-resumable-reader** — ⏳ not started (0/N)
- **03-idempotent-producer** — ⏳ not started (0/N)
- **04-fork-and-retention** — ⏳ not started (0/N)
- **05-storage-backends** — ⏳ not started (0/N)
- **06-http-rpc-surface** — ⏳ not started (0/N)

## Next steps

1. Feature 01 (durable stream types) first, then 02 and 03 in parallel, then 04, then 05 and 06.
2. Wire `foundation_log` (issue #155) and spec 65 `01-agent-rpc` / `05-cloudflare` to consume the
   primitive once 01–03 land.

_Created: 2026-10-30_
