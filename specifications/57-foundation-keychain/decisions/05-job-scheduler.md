# Decision 05: Native Job Scheduler — tokio-cron-scheduler

## Problem

The keychain needs scheduled jobs (purge expired sends, trash cleanup, etc.):

1. **tokio-cron-scheduler** — cron-compatible, tokio-native
2. **clockwork** — newer, less battle-tested
3. **Custom timer** — simple but reinvents cron parsing

## Analysis

- **tokio-cron-scheduler**: Mature, cron-syntax compatible, integrates with tokio runtime. 500+ GitHub stars, actively maintained. Supports the exact cron expressions the Cloudflare backend uses (`0 */6 * * *`, `0 0 * * *`).
- **clockwork**: Less mature. No clear advantage.
- **Custom timer**: Would need cron parsing, timezone handling, error recovery. Not worth it.

## Decision: tokio-cron-scheduler

The native backend uses `tokio-cron-scheduler` to run the same cron jobs the Cloudflare backend runs via Workers cron triggers. The job implementations are identical — only the scheduling mechanism differs.

### Job Schedule

| Cron Expression | Job | Frequency |
|----------------|-----|-----------|
| `0 */6 * * *` | Purge expired sends + R2/filesystem cleanup | Every 6 hours |
| `0 0 * * *` | Purge trashed ciphers (30+ days) | Daily at midnight |
| `0 */12 * * *` | Purge expired auth requests + old login attempts | Every 12 hours |
| `0 1 * * *` | Emergency access timeout processing | Daily at 1am |
| `0 2 * * *` | Event log cleanup | Daily at 2am |

## Consequences

- tokio-cron-scheduler adds one dependency to the native backend
- Job implementations are shared (portable functions that take `&dyn KeychainStore`)
- Cloudflare backend uses Workers cron triggers; native uses tokio-cron-scheduler
