# foundation_toolings — Build and dev tooling

## What it is
Build-time utilities: miniflare harness, wrangler integration, and CI helpers.

## Key modules
- **`miniflare/`** — Miniflare worker harness for testing CF Workers locally.
  Manages worker lifecycle (start/stop) and provides HTTP endpoints for tests.
- **`wrangler/`** — Wrangler CLI wrapper for deployment automation.

## Usage
The miniflare harness is used by `foundation_db`'s D1/KV integration tests:
```rust
let mut mf = Miniflare::start()?;
// Tests run against mf.base_url()
// Miniflare is stopped on Drop
```

## Integration
- `mise run cf:start` — Starts wrangler dev for local testing
- `mise run test:cf` — Runs CF integration tests with auto-start/stop
