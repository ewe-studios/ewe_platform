# Decision 05: Cron Jobs — foundation_cronjobs

**Status:** Resolved (2026-07-18)

## Problem

The keychain needs scheduled jobs (purge expired sends, trash cleanup). Cloudflare uses Workers cron triggers. Native needs an equivalent.

1. **foundation_cronjobs** — new crate: valtron-based scheduler with foundation_db persistence
2. **Per-crate ad-hoc timers** — each crate rolls its own scheduling

## Analysis

A cron scheduler is general infrastructure. Any crate that needs periodic work (purge jobs, cleanup, reporting) would benefit from a shared implementation. The scheduler needs:
- Cron expression parsing (`"0 */6 * * *"`)
- Next fire time computation
- Valtron `Delayed` parking (no tokio)
- Persistence via foundation_db (last_run, success/failure, consecutive errors, catch-up on restart)

## Decision: foundation_cronjobs crate

A new crate that provides `CronScheduler` — parse cron expressions, compute next fire time, spawn valtron tasks that park via `Delayed` until execution. Job state persists in foundation_db (SQL table: `cron_jobs` with id, cron_expr, last_run, last_status, error_count, next_run).

Keychain uses it directly. The Cloudflare backend uses Workers cron triggers (native to the platform, no scheduler needed).

## Consequences

- One scheduler for all crates
- No tokio dependency
- Job history survives restarts
- Missed-job policy configurable: catch up once, skip, or run all
