# cf-agent-loop

`foundation_ai` agentic layer on Cloudflare Workers, served through the
`foundation_http` `dispatch_cf` path, with Workers AI as the model provider
(spec 65 T5). The handler runs the turn to completion and returns the
accumulated assistant text (buffered, not streamed).

- `POST /` — the prompt as the body; the reply as plain text.

A failed turn returns a `500` with `error: …` and the partial text produced
before the error.

## Build and deploy

```sh
/Users/darkvoid/.cargo/bin/worker-build --release
npx wrangler deploy
```

Use your `worker-build` binary. `npx worker-build` can fail with "could not
determine executable".

## Secrets

The Worker reads its secrets from Cloudflare's encrypted secret store. They are
never in the repo.

- `WORKERS_AI_API_TOKEN` — the Workers AI API token (required).
- `CLOUDFLARE_ACCOUNT_ID` — the Cloudflare account id (required).
- `MODEL` — the model id. This is a `[vars]` entry in `wrangler.toml`, not a
  secret.
- `WORKERS_AI_BASE_URL` — optional override for a local mock.

### Set the secrets

`wrangler secret put NAME` reads the value from stdin and never echoes it back.
The values live in `~/.userenv`. Source it, then pipe each value:

```sh
cd /Users/darkvoid/Dev/ewe_platform/examples/cf-agent-loop \
  && set -a; source ~/.userenv 2>/dev/null; set +a \
  && printf '%s' "$CLOUDFLARE_WORKERS_AI_API_TOKEN" | wrangler secret put WORKERS_AI_API_TOKEN 2>&1 | tail -4 \
  && echo "---" \
  && printf '%s' "$CLOUDFLARE_ACCOUNT_ID" | wrangler secret put CLOUDFLARE_ACCOUNT_ID 2>&1 | tail -4
```

`~/.userenv` exports `CLOUDFLARE_WORKERS_AI_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID`. `set -a` marks everything it defines for export.

### How secrets behave

- No `wrangler.toml` config pushes secrets. Setting one is an explicit command,
  done once.
- After `secret put`, `wrangler deploy` uploads only code and config. It leaves
  the secret store untouched. Secrets persist across deploys.
- For local `wrangler dev` / miniflare, put them in an ignored `.dev.vars` file
  (see the comment in `wrangler.toml`).

## Verify

```sh
curl -s --max-time 120 \
  -X POST https://cf-agent-loop.<subdomain>.workers.dev/ \
  -d 'Say hello in one sentence.'
```
