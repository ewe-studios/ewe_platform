---
feature: "Cloudflare demo: sandbox and agent RPC in one application"
status: "pending"
priority: "high"
model: "sonnet"
phase: "3"
depends_on: ["00-live-proof"]
crates: ["foundation_sandbox", "foundation_agent_rpc", "foundation_http"]
---

# Feature 06: Cloudflare demo — sandbox and agent RPC in one application

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations.** Durable Object (DO); Worker (a Cloudflare program); command-line
interface (CLI); end-to-end (e2e).

## 1. Goal

One example under `examples/` shows the system in real life on Cloudflare: a web page
creates a sandbox, runs a command in it, reads a file, and holds one agent turn. The
example deploys with `wrangler`, so the same application is also the live end-to-end
(e2e) test (#1221).

## 2. Why a demo and not a test Worker

`wrangler dev --local` gives a working container TCP port to one container per Worker
process (#1221). Two sandboxes cannot run locally. The real runtime has no such limit.
A deployed demo proves the production path, and a user can open it.

`wrangler` stays the deployment tool: `wrangler dev` for the parts that work locally,
`wrangler deploy` for the live run.

## 3. The application

| Part | What it does |
|---|---|
| Web page | Shows how many sandboxes run. A button asks the system to create two more, each its own Durable Object. One tab per sandbox holds a command console and one agent turn, so a user talks to each sandbox on its own |
| Sandbox service | The existing `SandboxDo`, `SandboxIndexDo` and container binding from `infrastructure/sandbox_worker`; `List` gives the count and the tabs |
| Agent service | The existing worker host of `foundation_agent_rpc` with one model per sandbox, one conversation per sandbox |
| Config | `wrangler.toml` with the container image, the three DO bindings, one route, and a preview name for CI |

The page is static HTML and WebAssembly (wasm) from `foundation_http::wasm`; no framework.

**The count** comes from the sandbox index through `List`. It updates after each create and destroy. Two sandboxes are the case the local runtime cannot run (#1221), so the deployed demo proves it.

**Three model paths, one per sandbox**, so the demo tests every area the platform supports:

| Tab | Model path | Configuration |
|---|---|---|
| 1 | Cloudflare Workers AI | The `AI` binding of the Worker; no network key in the page |
| 2 | OpenRouter | The OpenAI-compatible provider with base URL `https://openrouter.ai/api/v1` and `OPENROUTER_API_KEY` as a Worker secret |
| 3 | A model served from the sandbox's own container | The model slave inside the sandbox's container, reached over the container port |

Each tab names its model path. A path whose secret or binding is absent shows that plainly, and the live test names it instead of failing silently.

**Files of the example:**

| File | Content |
|---|---|
| `wrangler.toml` | Container image, the three DO bindings, the AI binding, the route and a preview name suffix |
| `mise.toml` | `deploy` (build and deploy the persistent `ewe-sandbox-demo`, print the URL), `test` (preview run with teardown), `destroy` (remove the Worker and its container) |
| `test/live.sh` | The four steps, two more sandboxes, one turn per model path, then teardown |
| `README.md` | What it shows, the three model paths, prerequisites and the three secrets, the `mise run` commands, the URL, how to use each tab, the cost of one run, and the `wrangler dev` limit (#1221) |

`cd examples/cf-sandbox-demo && mise run deploy` is the whole redeploy. The README must let a reader who does not know the repository deploy and use the demo from that file alone.

## 4. Tests

| Test | Oracle |
|---|---|
| `test/live.sh` against a deployed preview name | create, exec, file write/read, one turn, destroy, then a delete of the Worker and its container |
| `test/live.sh` creates two more sandboxes | the count rises by two, each tab's command returns its own output, one agent turn in each tab, then destroy all and the count returns to zero |
| `test/live.sh` one turn per model path | Workers AI, OpenRouter and the container model each produce a non-empty answer, and the result names the provider used |
| the local part that `wrangler dev` supports | the page loads and the control probe answers |
| teardown runs on every path | conservation: no Worker and no container remain |

## 5. CI

One job in `sandbox-e2e.yaml` deploys the example to a preview name, runs `test/live.sh`,
and deletes it. Secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`. A missing secret
prints that the live run did not happen and finishes green, as `live-kms.yaml` does. The
local `wrangler dev` job stays, non-blocking, for the 13 tests it can run.

## 6. Done when

- `cd examples/cf-sandbox-demo && mise run deploy` deploys the demo and prints its URL.
- The README lets a reader deploy and use the demo from that file alone.
- `test/live.sh` passes against the deployed preview and leaves nothing behind.
- The page shows the count, the two new sandboxes and one tab per sandbox.
- Each of the three tabs answers through its own model path.
- The CI job is green, or says plainly why it did not run.
- The persistent demo URL is in `progress.md` and in the report to the owner.
