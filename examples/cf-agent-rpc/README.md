# cf-agent-rpc

`agent-rpc` on Cloudflare Workers. One Worker serves the Connect surface; one
Durable Object (DO) holds each instance; D1 holds the trust store and the
instance index; R2 holds the history archive of a destroyed instance.

The crate is the thin platform glue of spec 83 feature 05, task T21. The logic
lives in `foundation_agent_rpc::host::worker`; this crate builds the host once
per isolate and forwards each DO call to `AgentInstanceObject`.

## Build and run

The crate is its own workspace root, like the other `examples/cf-*` crates.

```bash
cargo install worker-build
worker-build --release
npx wrangler dev --local
```

`worker-build` runs the `build.command` in `wrangler.toml`. A plain
`cargo check --target wasm32-unknown-unknown` proves the crate compiles; it
does not run the Worker.

## Configuration

Every setting is a Worker var or binding:

| Name | Kind | Meaning |
|---|---|---|
| `AGENT_RPC_PUBLIC_URL` | var | The public base URL; the audience of a bearer secret |
| `AGENT_RPC_MODELS` | var | JSON list of remote models; each names a secret |
| `AGENT_RPC_SECRET_PUBLIC_KEYS` | var | JSON list of the public keys that verify bearer secrets |
| `AGENT_RPC_CORS_ORIGINS` | var | The exact origins that a browser may call from |
| `AGENT_RPC_DB` | D1 binding | The trust store and the instance index |
| `AGENT_INSTANCES` | DO binding | One `AgentInstance` for each instance |
| `AGENT_RPC_ARCHIVE` | R2 binding | The history archive of a destroyed instance |

A model provider key is a Worker secret, never a var:
`npx wrangler secret put WORKERS_AI_API_TOKEN`.

## Routes

The Worker serves the Connect path `/ewe.agent.v1.AgentService/<Method>` and
answers a Cross-Origin Resource Sharing (CORS) preflight for a configured
origin. It removes every incoming `ewe-forwarded-*` header before it checks the
caller, so a forged header has no effect.

`DestroyInstance` uploads the session history to R2 as gzip JSON lines, then
deletes the DO storage. A failed upload keeps the storage and reports
`UNAVAILABLE`.
