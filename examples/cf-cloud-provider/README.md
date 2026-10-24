# cf-cloud-provider

`foundation_ai` on Cloudflare Workers. A chat page backed by an agent session
that calls OpenRouter and streams each reply as it is generated (spec 65 T5).

- `GET /` — the chat page.
- `POST /chat` — the prompt as the body; the reply as Server-Sent Events (SSE).

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

- `OPENROUTER_API_KEY` — the OpenRouter key (required).
- `MODEL` — the model id. This is a `[vars]` entry in `wrangler.toml`, not a
  secret.
- `OPENROUTER_BASE_URL` — optional override for a local mock.

### Set `OPENROUTER_API_KEY`

`wrangler secret put NAME` reads the value from stdin, not a command-line
argument, and never echoes it back. Pipe the value from your environment:

```sh
cd /Users/darkvoid/Dev/ewe_platform/examples/cf-cloud-provider \
  && if [ -n "$OPENROUTER_API_KEY" ]; then \
       printf '%s' "$OPENROUTER_API_KEY" | wrangler secret put OPENROUTER_API_KEY 2>&1 | tail -10; \
     else echo "OPENROUTER_API_KEY NOT in shell env"; fi
```

`OPENROUTER_API_KEY` is already exported in the shell (inherited from your
profile).

### How secrets behave

- No `wrangler.toml` config pushes secrets. Setting one is an explicit command,
  done once.
- After `secret put`, `wrangler deploy` uploads only code and config. It leaves
  the secret store untouched. Secrets persist across deploys, so a later
  `wrangler secret list` still shows them.
- For local `wrangler dev` / miniflare, put the key in an ignored `.dev.vars`
  file (see the comment in `wrangler.toml`).

## Verify

```sh
curl -s -N --max-time 90 \
  -X POST https://cf-cloud-provider.<subdomain>.workers.dev/chat \
  -d 'Write a short poem about the sea in three lines.'
```
