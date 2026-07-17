# Decision 05: Native Job Scheduler — valtron-based

## Problem

The keychain needs scheduled jobs (purge expired sends, trash cleanup). Cloudflare uses Workers cron triggers. Native needs an equivalent.

1. **tokio-cron-scheduler** — cron-compatible but requires tokio runtime (not used in this workspace)
2. **valtron-based timer** — uses the workspace's existing executor
3. **Custom timer** — simple but reinvents cron parsing

## Analysis

The workspace uses valtron for all async execution. Pulling in `tokio-cron-scheduler` would add a second runtime for one use case. The cron parsing logic (`"0 */6 * * *"`) is trivial — a small cron parser on top of valtron's `Delayed` parking is straightforward.

## Decision: valtron-based cron scheduler

A simple valtron task that parses cron expressions, computes the next fire time, and parks via `Delayed` until then. No tokio dependency. Same cron expressions as the Cloudflare backend (`0 */6 * * *`, `0 0 * * *`).

## Consequences

- No tokio dependency in the workspace
- Consistent with how all other foundation crates schedule work
- Job implementations are portable functions that take `&dyn QueryStore` + `&dyn BlobStore` — identical between Cloudflare cron triggers and native valtron timer
