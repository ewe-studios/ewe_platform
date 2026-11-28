---
feature: "Google Cloud Run demo: two slaves and a local master"
status: "pending"
priority: "high"
model: "sonnet"
phase: "3"
depends_on: ["00-live-proof"]
crates: ["foundation_sandbox", "foundation_agent_rpc", "foundation_connectrpc"]
---

# Feature 07: Google Cloud Run demo — two slaves and a local master

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations.** Command-line interface (CLI); Google Cloud Platform (GCP); Hypertext
Transfer Protocol (HTTP); Transport Layer Security (TLS).

## 1. Goal

An example under `examples/` deploys **two Google Cloud Run services**, each running the
slave (`agent-slave`). A **local master** chats with both and does work through them. The
slaves call the **OpenRouter** model interface for their answers.

This proves the other half of the platform: not only a Cloudflare Worker master with a
container, but a native master that reaches remote slaves over the public network.

## 2. What it shows

| Part | Where it runs | What it does |
|---|---|---|
| Master | The operator's machine | Lists the two slaves, sends one turn to each, shows both answers |
| Slave A | Cloud Run service `ewe-slave-a` | Runs `agent-slave` in serve mode; calls OpenRouter for the model |
| Slave B | Cloud Run service `ewe-slave-b` | The same, a separate service and instance |
| Image | Artifact Registry | A small image: the static `agent-slave` binary, on a slim base, listening on `$PORT` |

Two services, not two instances of one: each has its own URL and its own secret, so the
master proves it talks to two independent remote slaves.

## 3. The contract

- **Transport.** The slave serves h2c on `$PORT`; Cloud Run terminates TLS. Deploy with
  `--use-http2` so HTTP/2 reaches the container. The master dials the service URL over
  HTTPS with the Connect protocol (`foundation_connectrpc`'s h2 client).
- **Auth.** Each service holds `EWE_SLAVE_TOKEN` as a Cloud Run secret; the master sends it
  as the bearer token. A call without the token is refused with `Unauthenticated`.
- **Model.** Each slave reads `OPENROUTER_API_KEY` from a Cloud Run secret and calls the
  OpenAI-compatible endpoint `https://openrouter.ai/api/v1`; the master never holds the key.
- **Streaming.** A turn streams over the single response; Cloud Run allows it with no
  change. Set the request timeout (`--timeout`) from configuration, default 300 s.
- **Scale.** `--min-instances=0`, `--max-instances=1` per service: one slave each, and no
  cost when idle.

## 4. Files

| File | Content |
|---|---|
| `Dockerfile` | Static `agent-slave` binary from the musl build, non-root user, `$PORT` |
| `deploy.sh` or `mise.toml` | `deploy` (build, push the image, deploy both services, print both URLs), `test` (one turn per service), `destroy` (delete both services and the image) |
| `test/live.sh` | Reads both URLs and tokens, sends one turn to each, asserts a non-empty answer that names the provider, then the destroy path |
| `README.md` | What it shows, prerequisites (an authenticated `gcloud`, billing, the APIs), the `mise run` commands, both URLs, the cost, and how the local master connects |

`mise run deploy` in the example directory is the whole redeploy.

## 5. Tests

| Test | Oracle |
|---|---|
| `test/live.sh` one turn per service | each answer is non-empty and names OpenRouter |
| a call without the token | `Unauthenticated` |
| `destroy` | conservation: neither service exists afterwards |
| the local master run | prints two different answers, one per service |

## 6. Done when

- `mise run deploy` deploys both services from a clean checkout and prints both URLs.
- `test/live.sh` passes, and `destroy` leaves nothing behind.
- The README lets a reader deploy and use it from that file alone.
- Both URLs are in `progress.md` and in the report to the owner.
