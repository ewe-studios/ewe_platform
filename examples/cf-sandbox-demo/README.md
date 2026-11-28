# cf-sandbox-demo

The Cloudflare demo of spec 92 feature 06. One Worker holds the sandbox
platform and the agent RPC service, and one web page shows both in real life.
The same application is the live end-to-end test: `test/live.sh` deploys a
preview, runs the proof, and removes the preview.

## What the page shows

- How many sandboxes run. The count comes from the sandbox index.
- A **Refresh** button that reads the index again. The page does not reload.
- A button that creates one sandbox and a button that creates two more. Each
  sandbox is its own Durable Object (DO) and its own container.
- A **sandboxes strip** below the create buttons. One card holds each sandbox.
  Each card has its own message box, so the user writes to that sandbox
  directly. The strip reads the index again after every create and every
  destroy.
- A **central chat** tab. Type `@name message`, for example
  `@sandbox-2 hello`. The Worker routes the turn to the named sandbox. Each
  answer shows the responder name, for example `[sandbox-2]`. An unknown name
  answers a clear error that lists the names that exist. A message without a
  name is refused.
- One tab per sandbox. A tab holds an evidence panel, a command console, a file
  write and read, and one agent conversation.
- Above each answer, one small line names the model path, its service, its
  address and its protocol. The Worker sends these values as response headers.
  It does not leave the page to guess them.
- Each tab runs its own model path, so two tabs stream two answers at the same
  time.

## The three model paths

| Tab | Model path | What it needs |
|---|---|---|
| 1 | Cloudflare Workers AI through the Worker `AI` binding | Nothing. The binding is in `wrangler.toml`. |
| 2 | OpenRouter through the agent RPC catalog, at `https://openrouter.ai/api/v1` | The Worker secret `OPENROUTER_API_KEY`. |
| 3 | A model server inside the sandbox container, on container port 8081 | Nothing. The image holds the model server. |

The tab names its path and its model. A path whose secret or binding is absent
shows the missing value in its tab. The live test prints the same value and
does not fail silently.

The third model is deterministic: it repeats the prompt in one short line. It
proves the container port path with no cost. It is not a large language model.

## Prerequisites

1. A Cloudflare account with Workers and Containers enabled.
2. Node 22 or newer.
3. The pinned tools of `ci/sandbox.toml`:
   - `wrangler` 4.144.0: `npm install -g wrangler@4.144.0`
   - `worker-build` 0.8.7: `cargo install worker-build --version 0.8.7`
4. A Rust toolchain with the `wasm32-unknown-unknown` and
   `x86_64-unknown-linux-musl` targets. The `mise run build` task adds the
   musl target.
5. Docker. `wrangler` uses it to build and push the container image.
6. [mise](https://mise.jdx.dev/) for the three tasks.
7. These values in the shell:

| Name | What it is |
|---|---|
| `CLOUDFLARE_ACCOUNT_ID` | The Cloudflare account identifier. |
| `CLOUDFLARE_API_TOKEN` | An API token with Workers Scripts Edit, Containers Edit and Workers AI Read. |
| `OPENROUTER_API_KEY` | The OpenRouter key. It becomes the Worker secret of the same name. |

`CLOUDFLARE_WORKERS_AI_API_TOKEN` is accepted in place of
`CLOUDFLARE_API_TOKEN` when it holds the same permissions.

## Deploy

```bash
cd examples/cf-sandbox-demo
mise run deploy
```

`mise run deploy` does four steps:

1. Builds the static sandbox slave and the container model server for
   `linux/amd64`.
2. Builds the Worker module with `worker-build --release`.
3. Deploys the Worker, its three Durable Object classes and its container.
4. Sets the `OPENROUTER_API_KEY` secret and prints the URL.

The URL shape is `https://ewe-sandbox-demo.<account-subdomain>.workers.dev`.
Open it in a browser.

`mise run deploy` is the whole redeploy. A later deploy keeps the secrets.

### Run the live proof

```bash
cd examples/cf-sandbox-demo
mise run test
```

`mise run test` deploys a preview name, runs `test/live.sh` against it, and
deletes the preview Worker and its container. The proof does these steps:

1. The probe answers 200.
2. The count starts at 0.
3. It creates one sandbox.
4. Tab 1 runs `uname -a`.
5. Tab 1 writes a file and reads the same bytes.
6. Tab 1 holds one Workers AI turn.
7. It creates two more sandboxes; the count is 3.
8. Tab 2 and tab 3 each run their own command and get their own output.
9. Tab 2 holds one OpenRouter turn and tab 3 holds one container model turn.
10. It destroys all sandboxes; the count returns to 0.
11. It deletes the Worker and its container and lists both, to prove that
    nothing remains.

The script exits non-zero when a step fails. A machine without the Cloudflare
values prints which value is missing and returns 0; the live run did not
happen.

### Remove the persistent demo

```bash
cd examples/cf-sandbox-demo
mise run destroy
```

## Use a tab

- **Evidence.** Click **Run evidence**. The tab runs four commands in its own
  sandbox: `hostname`, `uname -a`, the first three lines of
  `/etc/os-release`, and `uptime`. It also shows the sandbox identifier and the
  Durable Object name from the API. Three tabs show three different
  hostnames. The page runs `head -n 3 /etc/os-release`, because no shell runs a
  command line.
- **Command console.** Type a command line and click **Run**. The console shows
  the standard output and the standard error as they arrive. The last line has
  the exit code. No shell runs the line: the page splits it on spaces and sends
  the argument vector. A quoting character is a literal part of an argument.
- **File write and read.** Type a path and the content, then click **Write**.
  Click **Read** to read the same path. The path is relative to `/workspace`.
- **Agent turn.** Type a prompt and click **Send**. The answer streams into the
  box. The tab names the model path. An absent secret shows its name in the
  box.
- **Destroy this sandbox.** The container and the Durable Object stop, the
  count falls by one, and the other tabs stay.

## Write to a sandbox

- **From a card.** Type the message in the card of the sandbox and click
  **Send**. The turn runs in that sandbox only.
- **From the central chat.** Type `@name message`. The names are `sandbox-1`,
  `sandbox-2`, and so on, in creation order. The sandbox identifier is also
  accepted. Each answer shows the responder name and its metadata line.

## The metadata line

Each answer has one line with four values from the Worker:

| Tab | Model path | Service | Address | Protocol |
|---|---|---|---|---|
| 1 | `workers-ai` | Workers AI | `binding AI` | `HTTPS` |
| 2 | `openrouter` | OpenRouter | `https://openrouter.ai/api/v1` | `HTTPS` |
| 3 | `container-model` | in-sandbox model server | `http://127.0.0.1:8081` | `HTTP in the sandbox` |

The Worker sets these response headers: `x-ewe-model-path`, `x-ewe-model`,
`x-ewe-model-service`, `x-ewe-model-address`, `x-ewe-model-protocol`,
`x-ewe-sandbox-id` and `x-ewe-sandbox-name`. `GET /api/sandboxes` sends the same
values as JSON fields, with the name and the Durable Object name.

## Cost of one run

- The Workers AI path uses the free daily Workers AI allocation. One short turn
  uses a small part of it.
- The OpenRouter path uses the free model in `OPENROUTER_MODEL` (default
  `google/gemma-4-31b-it:free`). It costs nothing while the model is free.
- The container path runs a local string function in the container. It costs
  no model fee.
- Cloudflare bills the containers by the time that they run. The live proof
  keeps three `lite` containers for a few minutes. One full proof costs cents
  or less.

Change a model in `wrangler.toml` (`WORKERS_AI_MODEL`, `OPENROUTER_MODEL`) and
deploy again to use another model.

## Limits

`wrangler dev --local` gives a working container TCP port to one container per
Worker process (issue #1221). Two sandboxes cannot run locally. The real
runtime has no such limit, so the demo is a real deploy. `wrangler dev` is
useful for the page and the control probe only:

```bash
mise run build
mkdir -p .dev.vars
printf '%s\n' 'OPENROUTER_API_KEY=test' > .dev.vars
wrangler dev --local
curl -s http://127.0.0.1:8787/api/probe
```

The one local sandbox works, but a second create fails. Use the deployed demo
for the count and the three tabs.

## Files

| File | What it holds |
|---|---|
| `src/lib.rs` | The Worker entry, the effect bundle and the reused DO classes. |
| `src/api.rs` | The control API under `/api/`. |
| `src/sandbox.rs` | The Connect JSON client of the sandbox and container services. |
| `src/agent.rs` | The three model paths. |
| `src/page.rs` | The page, served from `foundation_http::wasm`. |
| `src/config.rs` | Every var and limit, with its default. |
| `static/index.html` | The page. Plain HTML and JavaScript; no framework. |
| `model/` | The deterministic model server of the container. |
| `Dockerfile`, `entrypoint.sh` | The container image. |
| `wrangler.toml` | The Worker, the three DO bindings, the AI binding and the container. |
| `mise.toml` | The `deploy`, `test` and `destroy` tasks. |
| `test/live.sh` | The live end-to-end proof. |

## Troubleshooting

- **The deploy says the token is not authorized.** The token needs Workers
  Scripts Edit and Containers Edit. A Workers AI token alone cannot deploy.
- **The container image build says `docker-credential-osxkeychain` is
  missing.** The Kiri runtime is the Docker runtime. Put `$HOME/.kiri/bin`
  first in `PATH`, use the `kiri` context, and use a Docker config without
  `credsStore`. The old OrbStack link at `/usr/local/bin/docker` is broken.
- **The container image build fails.** Start Docker, then run
  `mise run deploy` again.
- **The OpenRouter tab shows `the Worker secret OPENROUTER_API_KEY is not
  set`.** Run `printf '%s' "$OPENROUTER_API_KEY" | wrangler secret put
  OPENROUTER_API_KEY --config wrangler.toml`, then open the tab again.
- **The Workers AI tab fails.** The account must have Workers AI enabled and
  the `AI` binding. Check the model id in `WORKERS_AI_MODEL`.
- **The count does not rise by two.** The maximum is `MAX_SANDBOXES` (default
  3). Destroy a sandbox first.
