# Voice Agent Server -- Deployment

## Docker

The project includes a multi-stage Dockerfile optimized for production:

```
Stage 1 (base):    node:22.18.0-slim
Stage 2 (build):   Install dev deps, compile TypeScript
Stage 3 (final):   Copy built output + production deps only
```

### Build

```bash
docker build -t voice-agent-server .
```

### Run

```bash
docker run -p 3000:3000 \
  -e VAPI_API_KEY=your-key-here \
  voice-agent-server
```

### Volume Persistence

The `data.json` file is written inside the container at `/app/data.json`. To persist it across container restarts, mount a volume:

```bash
docker run -p 3000:3000 \
  -e VAPI_API_KEY=your-key-here \
  -v $(pwd)/data.json:/app/data.json \
  voice-agent-server
```

## Fly.io

The project includes a `fly.toml` configuration file, pre-configured for deployment:

```toml
app = 'server-hidden-fog-8293'
primary_region = 'yyz'

[http_service]
  internal_port = 3000
  force_https = true
  auto_stop_machines = 'stop'
  auto_start_machines = true
  min_machines_running = 0

[[vm]]
  memory = '1gb'
  cpu_kind = 'shared'
  cpus = 1
```

### Deploy

```bash
fly launch          # First-time setup (generates fly.toml if missing)
fly deploy          # Build and deploy
```

### Set Environment Variables

```bash
fly secrets set VAPI_API_KEY=your-vapi-api-key
```

### Data Persistence on Fly.io

Fly.io machines are ephemeral when `auto_stop_machines = 'stop'`. The `data.json` file will be lost when the machine stops. For production use, you have two options:

1. **Keep machines running:** Set `min_machines_running = 1` so the machine never stops
2. **Use a Fly.io volume:** Mount a persistent volume to `/app` for `data.json`

For a simple project like this, option 1 (always-running machine) is the easiest.

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `VAPI_API_KEY` | Yes | API key for Vapi AI platform. Used to create/manage assistants and phone numbers. |

Obtain your Vapi API key from the [Vapi dashboard](https://dashboard.vapi.ai).

## Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 3000 | HTTP | REST API |

Fly.io terminates HTTPS at its edge and forwards to port 3000 internally (`force_https = true`).

## License

ISC -- permissive open source license. Use freely for personal and commercial projects.
