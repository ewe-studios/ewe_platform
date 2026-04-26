---
title: Paperclip -- Development Guide
---

# Paperclip -- Development Guide

## Prerequisites

| Requirement | Version |
|-------------|---------|
| Node.js | 20+ |
| pnpm | 9.15+ |

## Quickstart

### One-Command Local Install

```bash
npx paperclipai onboard --yes
```

This defaults to trusted local loopback mode for the fastest first run.

### Manual Setup

```bash
git clone https://github.com/paperclipai/paperclip.git
cd paperclip
pnpm install
pnpm dev
```

This starts:

- **API server:** `http://localhost:3100`
- **UI:** served by the API server in dev middleware mode (same origin)

An embedded PostgreSQL database is created automatically -- no external database setup required.

## Development Commands

| Command | Purpose |
|---------|---------|
| `pnpm dev` | Full dev (API + UI, watch mode) |
| `pnpm dev:once` | Full dev without file watching |
| `pnpm dev:server` | Server only |
| `pnpm dev:ui` | UI only |
| `pnpm dev:list` | Inspect current dev runner |
| `pnpm dev:stop` | Stop the current dev runner |
| `pnpm build` | Build all packages |
| `pnpm typecheck` | TypeScript type checking |
| `pnpm test` | Vitest unit tests (cheap, default) |
| `pnpm test:watch` | Vitest interactive watch mode |
| `pnpm test:e2e` | Playwright browser suite |
| `pnpm test:e2e:headed` | Playwright with headed browser |
| `pnpm db:generate` | Generate Drizzle migration |
| `pnpm db:migrate` | Apply database migrations |
| `pnpm db:backup` | Manual database backup |
| `pnpm paperclipai` | CLI entry point |

`pnpm test` does not run Playwright. Browser suites stay separate and are typically run only when working on those flows or in CI.

## Database in Development

For local development, leave `DATABASE_URL` unset. The server automatically uses embedded PostgreSQL:

- Data persists at `~/.paperclip/instances/default/db/`
- To reset local dev data: `rm -rf ~/.paperclip/instances/default/db`

To use an external database:

```bash
# Local Docker PostgreSQL
DATABASE_URL=postgres://paperclip:paperclip@localhost:5432/paperclip

# Hosted (Supabase)
DATABASE_URL=postgres://postgres.[PROJECT-REF]:[PASSWORD]@...pooler.supabase.com:6543/postgres
```

## Docker Quickstart

### Build and Run

```bash
docker build -t paperclip-local .
docker run --name paperclip \
  -p 3100:3100 \
  -e HOST=0.0.0.0 \
  -e PAPERCLIP_HOME=/paperclip \
  -v "$(pwd)/data/docker-paperclip:/paperclip" \
  paperclip-local
```

### Docker Compose

```bash
docker compose -f docker/docker-compose.quickstart.yml up --build
```

Persist the volume at `/paperclip` to keep DB state across container restarts.

## Tailscale/Private Auth Dev Mode

For remote access via Tailscale:

```bash
pnpm dev --bind lan     # Authenticated/private with private-network bind
pnpm dev --bind tailnet # Tailscale-only reachability
```

Allow additional private hostnames:

```bash
pnpm paperclipai allowed-hostname dotta-macbook-pro
```

## Worktree Development

When developing from multiple git worktrees, do not point two Paperclip servers at the same embedded PostgreSQL data directory.

### Create an Isolated Worktree Instance

```bash
# Initialize isolated instance in current worktree
paperclipai worktree init

# Or create a git worktree and initialize it in one step
pnpm paperclipai worktree:make paperclip-pr-432
```

This creates:

- Repo-local config at `.paperclip/config.json` and `.paperclip/.env`
- Isolated instance under `~/.paperclip-worktrees/instances/<worktree-id>/`
- Auto-assigned free app port and embedded PostgreSQL port
- Database seeded from the current effective instance (`minimal` mode by default)

### Seed Modes

| Mode | Description |
|------|-------------|
| `minimal` | Keeps core app state (companies, projects, issues, comments, approvals, auth) but omits heavy operational history |
| `full` | Full logical clone of the source instance |
| `--no-seed` | Empty isolated instance |

### Worktree CLI Reference

| Command | Purpose |
|---------|---------|
| `paperclipai worktree init` | Create repo-local config and isolated instance |
| `paperclipai worktree repair` | Repair or create linked worktree config |
| `paperclipai worktree reseed` | Re-seed existing worktree from another instance |
| `paperclipai worktree:make <name>` | Create git worktree + init in one step |
| `paperclipai worktree env` | Print shell exports for the worktree instance |

Worktree servers use branded banners and colored favicons for easy identification.

## Testing Strategy

### Unit Tests (Vitest)

The default test command. Fast, runs on every change:

```bash
pnpm test
```

### End-to-End Tests (Playwright)

Browser-based tests for UI flows:

```bash
pnpm test:e2e                # Headless
pnpm test:e2e:headed         # With browser window
```

### Release Smoke Tests

Tests that validate release artifacts:

```bash
pnpm test:release-smoke
```

### OpenClaw Join Smoke Test

End-to-end validation of agent onboarding:

```bash
pnpm smoke:openclaw-join
```

Validates: invite creation, agent join request, board approval, API key claim, callback delivery.

### Evaluations

Prompt-based evaluations using promptfoo:

```bash
pnpm evals:smoke
```

## Telemetry

Paperclip collects anonymous usage telemetry. No personal information, issue content, prompts, file paths, or secrets are ever collected. Private repository references are hashed with a per-install salt.

Telemetry is **enabled by default** and can be disabled:

| Method | How |
|--------|-----|
| Environment variable | `PAPERCLIP_TELEMETRY_DISABLED=1` |
| Standard convention | `DO_NOT_TRACK=1` |
| CI environments | Automatically disabled when `CI=true` |
| Config file | Set `telemetry.enabled: false` in Paperclip config |

## Contributing

### Dependency Lockfile Policy

GitHub Actions owns `pnpm-lock.yaml`:

- Do not commit `pnpm-lock.yaml` in pull requests
- PR CI validates dependency resolution when manifests change
- Pushes to `master` regenerate `pnpm-lock.yaml`

### Before Submitting

1. Run `pnpm test` -- unit tests must pass
2. Run `pnpm typecheck` -- no TypeScript errors
3. For UI changes, run `pnpm test:e2e`

### Core Features vs. Plugins

If you want to work on roadmap-level core features, please coordinate in Discord (`#dev`) before writing code. Bugs, docs, polish, and tightly scoped improvements are the easiest contributions to merge.

For extending Paperclip, the best path is often the **plugin system**. Community reference implementations are valuable feedback even when not merged directly into core.

## Roadmap

### Completed

- Plugin system
- OpenClaw / claw-style agent employees
- companies.sh import/export
- Easy AGENTS.md configurations
- Skills Manager
- Scheduled Routines
- Better Budgeting
- Agent Reviews and Approvals

### In Progress / Planned

- Multiple human users
- Cloud / sandbox agents (Cursor, e2b)
- Artifacts and work products
- Memory / knowledge
- Enforced outcomes
- MAXIMIZER MODE
- Deep planning
- Work queues
- Self-organization
- Automatic organizational learning
- CEO chat
- Cloud deployments
- Desktop app

See [ROADMAP.md](https://github.com/paperclipai/paperclip/blob/master/ROADMAP.md) in the repository for the full details.

## Secrets Management

Secret values are stored with local encryption and only secret refs are persisted in agent config:

- Default local key path: `~/.paperclip/instances/default/secrets/master.key`
- Override key: `PAPERCLIP_SECRETS_MASTER_KEY`
- Override key file: `PAPERCLIP_SECRETS_MASTER_KEY_FILE`

Strict mode (blocks inline sensitive env values):

```bash
PAPERCLIP_SECRETS_STRICT_MODE=true
```

Migrate existing inline env secrets:

```bash
pnpm secrets:migrate-inline-env         # dry run
pnpm secrets:migrate-inline-env --apply # apply
```

## Health Checks

```bash
curl http://localhost:3100/api/health
curl http://localhost:3100/api/companies
```

Expected:

- `/api/health` returns `{"status":"ok"}`
- `/api/companies` returns a JSON array

## Automatic DB Backups

Paperclip can run automatic DB backups on a timer:

- Enabled by default
- Every 60 minutes
- Retain 30 days
- Backup dir: `~/.paperclip/instances/default/data/backups`

Configure via:

```bash
pnpm paperclipai configure --section database
```

Manual backup:

```bash
pnpm paperclipai db:backup
```
