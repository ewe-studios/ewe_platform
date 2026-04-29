# Resonate -- Deployment

## Overview

The Resonate server is a single Rust binary with zero external dependencies beyond the database. It runs anywhere: bare metal, VPS, containers, or serverless platforms. This document covers production deployment patterns, authentication setup, and operational configuration.

## Deployment Architecture

```mermaid
graph TD
    subgraph Internet
        Clients[Client Applications]
        Workers[SDK Workers]
    end
    
    subgraph Infrastructure
        LB[Nginx / Caddy<br/>SSL Termination]
        Server[Resonate Server<br/>:8001]
        Metrics[Metrics<br/>:9090]
        DB[(PostgreSQL / SQLite)]
    end
    
    subgraph Monitoring
        Prometheus[Prometheus]
        Grafana[Grafana]
    end
    
    Clients -->|HTTPS| LB
    Workers -->|HTTPS| LB
    LB -->|HTTP| Server
    Server --> DB
    Server --> Metrics
    Prometheus -->|scrape| Metrics
    Grafana --> Prometheus
```

## Installation Methods

### Homebrew (macOS/Linux)

```bash
brew install resonatehq/tap/resonate
resonate serve
```

### Docker

```bash
git clone https://github.com/resonatehq/resonate
cd resonate
docker-compose up
```

### Build from Source

```bash
git clone https://github.com/resonatehq/resonate
cd resonate
cargo build --release
./target/release/resonate serve
```

### Docker Image (Production)

```dockerfile
FROM rust:1.77-bookworm as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/resonate /usr/local/bin/
EXPOSE 8001 9090
CMD ["resonate", "serve"]
```

## Production Configuration

### Minimal Production Config

```toml
# /etc/resonate/resonate.toml

[server]
host = "0.0.0.0"
port = 8001
url = "https://resonate.mycompany.com"  # Public URL for callbacks
shutdown_timeout = "10s"

[server.cors]
allow_origins = ["https://app.mycompany.com"]

[storage]
type = "postgres"

[storage.postgres]
url = "postgres://resonate:secret@db.internal:5432/resonate"
pool_size = 20

[auth]
publickey = "/etc/resonate/public.pem"
iss = "mycompany"
aud = "resonate-api"

[tasks]
lease_timeout = "15s"
retry_timeout = "30s"

[timeouts]
poll_interval = "1000ms"

[messages]
poll_interval = "100ms"
batch_size = 100

[transports.http_push]
enabled = true
concurrency = 32
connect_timeout = "10s"
request_timeout = "3m"

[transports.http_poll]
enabled = true
max_connections = 5000
buffer_size = 100

[observability]
metrics_port = 9090
# otlp_endpoint = "http://localhost:4317"  # Future OTLP export
```

### Environment Variable Override

Every config field maps to an environment variable:

```bash
RESONATE_SERVER__PORT=8001
RESONATE_SERVER__URL=https://resonate.mycompany.com
RESONATE_STORAGE__TYPE=postgres
RESONATE_STORAGE__POSTGRES__URL=postgres://resonate:secret@db:5432/resonate
RESONATE_STORAGE__POSTGRES__POOL_SIZE=20
RESONATE_AUTH__PUBLICKEY=/etc/resonate/public.pem
RESONATE_TASKS__LEASE_TIMEOUT=15000
RESONATE_TASKS__RETRY_TIMEOUT=30000
RESONATE_TRANSPORTS__HTTP_PUSH__CONCURRENCY=32
RESONATE_TRANSPORTS__HTTP_PUSH__AUTH__MODE=gcp
RESONATE_OBSERVABILITY__METRICS_PORT=9090
```

## Authentication Setup

### Generate Key Pair

```bash
# RSA (most common)
openssl genrsa -out private.pem 2048
openssl rsa -in private.pem -pubout -out public.pem

# Ed25519 (faster, smaller)
openssl genpkey -algorithm ed25519 -out private.pem
openssl pkey -in private.pem -pubout -out public.pem

# EC (P-256)
openssl ecparam -genkey -name prime256v1 -out private.pem
openssl ec -in private.pem -pubout -out public.pem
```

### Server Config

```toml
[auth]
publickey = "/etc/resonate/public.pem"
iss = "mycompany"        # Required issuer claim
aud = "resonate-api"     # Required audience claim
```

### Generate JWT Tokens

```bash
# For an admin (full access)
jwt encode --secret @private.pem --alg RS256 \
    --iss mycompany --aud resonate-api \
    --exp "+30d" \
    '{"role": "admin", "sub": "admin-service"}'

# For a tenant (prefix-restricted)
jwt encode --secret @private.pem --alg RS256 \
    --iss mycompany --aud resonate-api \
    --exp "+30d" \
    '{"role": "user", "prefix": "tenant-abc/", "sub": "tenant-abc"}'
```

### Multi-Tenant Isolation

```mermaid
graph TD
    subgraph "Tenant A (prefix: tenant-a/)"
        A1[Worker A1] --> |"tenant-a/*"| Server
        A2[Worker A2] --> |"tenant-a/*"| Server
    end
    
    subgraph "Tenant B (prefix: tenant-b/)"
        B1[Worker B1] --> |"tenant-b/*"| Server
    end
    
    subgraph "Admin"
        Admin[Admin Service] --> |"*"| Server
    end
    
    Server[Resonate Server]
```

Prefix-restricted tokens can only create/access promises whose ID starts with their prefix. This provides namespace isolation without separate server instances.

## Systemd Service

```ini
# /etc/systemd/system/resonate.service
[Unit]
Description=Resonate Durable Execution Server
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=resonate
Group=resonate
ExecStart=/usr/local/bin/resonate serve
WorkingDirectory=/var/lib/resonate
Environment=RUST_LOG=resonate=info
EnvironmentFile=-/etc/resonate/env
Restart=always
RestartSec=5
LimitNOFILE=65535

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/resonate
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable resonate
sudo systemctl start resonate
sudo journalctl -u resonate -f
```

## Reverse Proxy (Nginx)

```nginx
upstream resonate {
    server 127.0.0.1:8001;
}

server {
    listen 443 ssl http2;
    server_name resonate.mycompany.com;
    
    ssl_certificate /etc/letsencrypt/live/resonate.mycompany.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/resonate.mycompany.com/privkey.pem;
    
    # Main API
    location / {
        proxy_pass http://resonate;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
    
    # SSE (polling) — needs long timeouts
    location /poll/ {
        proxy_pass http://resonate;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_buffering off;
        proxy_read_timeout 3600s;  # 1 hour for SSE
        proxy_send_timeout 3600s;
    }
}
```

## Database Setup

### PostgreSQL (Recommended for Production)

```bash
# Create database and user
sudo -u postgres psql
CREATE USER resonate WITH PASSWORD 'secret';
CREATE DATABASE resonate OWNER resonate;
\q

# Configure connection
RESONATE_STORAGE__TYPE=postgres
RESONATE_STORAGE__POSTGRES__URL=postgres://resonate:secret@localhost:5432/resonate
RESONATE_STORAGE__POSTGRES__POOL_SIZE=20
```

The server auto-creates tables on first run. No migration tool needed.

### SQLite (Single-Node / Dev)

```toml
[storage]
type = "sqlite"
[storage.sqlite]
path = "/var/lib/resonate/resonate.db"
```

SQLite is sufficient for single-node deployments handling moderate traffic (<1000 req/s). For high-concurrency or multi-node, use PostgreSQL.

## Google Cloud Run Deployment

```yaml
# cloud-run.yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: resonate
spec:
  template:
    spec:
      containers:
        - image: gcr.io/my-project/resonate:latest
          ports:
            - containerPort: 8001
          env:
            - name: RESONATE_SERVER__PORT
              value: "8001"
            - name: RESONATE_SERVER__URL
              value: "https://resonate-xxxxx.run.app"
            - name: RESONATE_STORAGE__TYPE
              value: "postgres"
            - name: RESONATE_STORAGE__POSTGRES__URL
              valueFrom:
                secretKeyRef:
                  name: db-url
                  key: url
            - name: RESONATE_TRANSPORTS__HTTP_PUSH__AUTH__MODE
              value: "gcp"
          resources:
            limits:
              memory: 512Mi
              cpu: "1"
```

With `auth.mode = "gcp"`, the server automatically obtains OIDC tokens for outbound HTTP push to Cloud Functions/Cloud Run targets.

## Scaling Considerations

| Scale | Recommendation |
|-------|---------------|
| Dev / Single user | SQLite + single instance |
| Small team (<10 workers) | PostgreSQL + single instance |
| Medium (10-100 workers) | PostgreSQL + horizontal (read replicas) |
| Large (100+ workers) | PostgreSQL + multiple server instances + load balancer |

### Horizontal Scaling

Multiple Resonate server instances can share a PostgreSQL backend:

```
Load Balancer
    ├── Resonate Instance 1 ──┐
    ├── Resonate Instance 2 ──┼── PostgreSQL
    └── Resonate Instance 3 ──┘
```

Each instance runs its own timeout/message processing loop. PostgreSQL's serialization guarantees prevent double-processing.

## Health Checks

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `GET /health` | Liveness | 200 (always) |
| `GET /ready` | Readiness | 200 (DB reachable) / 503 (DB down) |

```yaml
# Kubernetes probes
livenessProbe:
  httpGet:
    path: /health
    port: 8001
  initialDelaySeconds: 5
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /ready
    port: 8001
  initialDelaySeconds: 5
  periodSeconds: 5
```

## Backup & Recovery

### SQLite

```bash
# Hot backup (WAL mode allows concurrent reads)
sqlite3 /var/lib/resonate/resonate.db ".backup /backups/resonate-$(date +%Y%m%d).db"
```

### PostgreSQL

```bash
pg_dump -Fc resonate > /backups/resonate-$(date +%Y%m%d).dump
# Restore
pg_restore -d resonate /backups/resonate-20260429.dump
```

## Source Paths

| Topic | Path |
|-------|------|
| Configuration | `resonate/src/config.rs` |
| Authentication | `resonate/src/auth.rs` |
| Health endpoints | `resonate/src/server.rs` |
| Docker setup | `resonate/Dockerfile`, `resonate/docker-compose.yml` |
| Deployment skill | `resonate-skills/resonate-server-deployment/SKILL.md` |
| Cloud Run skill | `resonate-skills/resonate-server-deployment-cloud-run/` |
