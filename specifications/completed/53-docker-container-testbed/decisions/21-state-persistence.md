# 21 — State persistence (Stage 3)

**Date:** 2026-07-10
**Status:** Resolved

## Decision

`foundation_proxy` persists its routing table (services, backends, health state)
to SQLite via `foundation_db`. On restart, the proxy reloads state and reconnects
health probes — no data loss, no manual re-registration.

kamal-proxy persists to a JSON file. We use SQLite because `foundation_db`
already wraps it, giving us transactions, schema migrations, and concurrent-read
safety for free.

## Why

Without persistence:
1. **Proxy restart loses all backends** — every container must be re-registered,
   breaking the zero-downtime guarantee if the proxy itself restarts mid-deploy.
2. **Health probe state is lost** — a backend that was unhealthy before restart
   starts healthy again, receiving traffic before it's actually ready.
3. **Deployment state is lost** — a canary in progress resets to zero.

## Schema

```sql
CREATE TABLE IF NOT EXISTS services (
    name        TEXT PRIMARY KEY,
    host        TEXT NOT NULL,
    path_prefix TEXT,
    ssl_config  TEXT NOT NULL DEFAULT '{}',    -- JSON
    health_check_config TEXT,                   -- JSON, nullable
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS backends (
    id              TEXT PRIMARY KEY,
    service_name    TEXT NOT NULL REFERENCES services(name) ON DELETE CASCADE,
    url             TEXT NOT NULL,
    weight          INTEGER NOT NULL DEFAULT 1,
    max_connections INTEGER NOT NULL DEFAULT 1024,
    state           TEXT NOT NULL DEFAULT 'active',   -- active, draining, paused
    healthy         INTEGER NOT NULL DEFAULT 1,       -- boolean
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_backends_service ON backends(service_name);
```

## Startup sequence

```
ProxyServer::start(config)
  ├── 1. Open DB connection (or create if first run)
  ├── 2. Run migrations (foundation_db auto-migrate)
  ├── 3. Load persisted services + backends from DB
  ├── 4. Merge with config.services (config takes precedence for schema,
  │      DB backends are additive — registered-via-RPC backends survive restart)
  ├── 5. Build RuntimeService / BackendRuntime from merged state
  ├── 6. Spawn health probes (they pick up persisted healthy flags)
  └── 7. Start HTTP front-end + RPC server
```

## Write paths

| Event | DB write |
|-------|----------|
| Backend registered (RPC `deploy`) | INSERT into `backends` |
| Backend removed (RPC `remove`) | DELETE from `backends` |
| Backend state change (pause/drain/active) | UPDATE `backends.state` |
| Health transition | UPDATE `backends.healthy` |
| Service added (RPC or config) | INSERT into `services` |
| Service removed | DELETE from `services` (cascades) |

All writes happen inside the proxy's existing mutex/RPC-handler paths — no
extra synchronization needed.

## Dependencies

| Crate | Role |
|-------|------|
| `foundation_db` | SQLite connection, migrations, typed queries |
| `serde_json` | Serialize `HealthCheckConfig`/`SslConfig` to JSON text columns |

## Verification

1. Start proxy, register backend via RPC, kill proxy, restart — backend is
   still registered and health probe reconnects.
2. Start proxy, pause a backend via RPC, restart — backend is still paused.
3. Schema migration: add a column, old DB auto-migrates on startup.
