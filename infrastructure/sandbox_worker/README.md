# The sandbox Cloudflare Worker

The Worker of the foundation sandbox platform (spec 77 feature 08). It holds
three Durable Object (DO) classes:

| Class | Purpose |
|---|---|
| `SandboxDo` | One sandbox. It holds the control plane and one container. |
| `SandboxIndexDo` | One DO named `index`. It holds the list of sandboxes. |
| `ModelSlaveDo` | One model slave (decision D16). It answers each command with one model turn. |

The Worker entry checks the bearer token, reads the request body under the
message cap, routes the call, and forwards it to the DO. The response streams,
so an `Exec` frame reaches the caller before the command ends.

**The model slave gives no operating-system isolation.** The model never runs a
program. It answers text. Use a container sandbox when a command must have a
real result.

**`List` is a view.** An entry is removed when its sandbox stops, and when a
call to that sandbox answers `NotFound`. An entry of a sandbox that ended while
nobody called it stays until the first call to it. `List` therefore means
"sandboxes that were not seen to end".

## Deploy

The primary path is the Rust deployer of `foundation_deployment_cloudflare`:
`CloudflareDeployer::deploy` applies the plan from
`DeploymentPlan::sandbox_worker()`. The deploy host supplies the Cloudflare
application programming interface (API) client, the Docker client and the secret
store. The deployer does not ship that client today; the generated Worker module
upload is the missing part (GitHub issue). Until it lands, use the fallback.

The fallback is the `wrangler` command line interface (CLI). These are the
manual steps:

1. Install Node, `wrangler` and `worker-build`.
2. Build the static slave and copy it here: `mise run sandbox:worker-image`.
3. Set the token: `npx wrangler secret put SANDBOX_TOKEN`.
4. Deploy: `npx wrangler deploy`.

`wrangler.toml` is a view of the deployment plan. The plan is the source of
truth.

## Local run

`wrangler dev` needs a `.dev.vars` file with the token and the model settings.
The file is in `.gitignore`.

```bash
SANDBOX_TOKEN=test
MODEL_BACKEND=openrouter
MODEL_BASE_URL=http://127.0.0.1:8799
MODEL_NAME=fake
MODEL_API_KEY=test
```

`test/run.sh` writes this file, starts `wrangler dev` and the local fake model
server, and runs the end-to-end tests. `--keep-running` leaves the Worker up.

## Limits

`max_instances = 10` is the most sandboxes this Worker can have at one time. It
is a starting value: change it in `wrangler.toml` and in
`DeploymentPlan::sandbox_worker`. A `Create` above it fails in
`container.start`, and the caller gets `SubstrateApi`.

`instance_type = "lite"` sets the CPU and the memory of each sandbox.
`SandboxSpec.vcpus` and `SandboxSpec.memory_mib` are not used on Cloudflare.

The model requests use `OpenRouter` with `MODEL_API_KEY`, or the Cloudflare
Model API through the `AI` binding with no key.
