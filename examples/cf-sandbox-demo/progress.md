# cf-sandbox-demo progress

## What landed

- The page has a sandbox strip below the create buttons. One card holds each
  sandbox, and each card has its own message box. The strip reads the index
  again after every create and every destroy.
- The page has a central chat tab. The Worker route `POST /api/chat` resolves
  `@name message` against the sandbox index and calls the named sandbox. An
  unknown name answers a message that lists the names that exist. A message
  without `@name` is refused.
- `GET /api/sandboxes` answers the sandbox name, the Durable Object name and
  binding, and the model path metadata of each sandbox.
- Each chat response carries the model path, the model identifier, the service,
  the address, the protocol, the sandbox identifier and the sandbox name as
  response headers. The page reads the headers, so it does not guess them.
- Each sandbox tab has an Evidence panel. It runs `hostname`, `uname -a`, the
  first three lines of `/etc/os-release`, and `uptime` in its own sandbox. It
  shows the sandbox identifier and the Durable Object name from the API.
- A Refresh button reads the sandbox index on demand. The page does not reload.

## Verification

- `cargo fmt`: the crate is formatted.
- `cargo check --target wasm32-unknown-unknown --release`: it passes. The
  command finished the release profile in 5 minutes 43 seconds on 2026-11-28.
  It printed warnings in other crates. The demo crate has no warning.
- `mise run build`: it passes. It wrote `agent-slave`, `demo-model` and
  `build/index.js`.
- The preview Worker
  `https://ewe-sandbox-demo-ui-k3f9.trinoxf.workers.dev` deployed with the
  Kiri Docker runtime on 2026-11-28.
  - `/api/probe` answers 200.
  - The page holds the sandbox strip, the Refresh button, the central chat,
    the Evidence panel and the metadata text.
  - `GET /api/sandboxes` answers `count: 0` and `max: 3`.
  - `POST /api/chat` with `@sandbox-1 hello` answers 409 and
    `there are no sandboxes; create one first`.
- The live three-sandbox run waits for the create central processing unit
  (CPU) defect that another agent owns. The create stream answers the error
  `the /ewe.sandbox.control.v1.SandboxService/Create call: Error: Durable
  Object exceeded its CPU time limit and was reset.`
- The preview Worker, its container and its managed image are deleted. The
  persistent `ewe-sandbox-demo` and its container are untouched.
- `test/live.sh` now checks the page controls, the index metadata, the three
  hostnames, the central chat routing and the unknown-name error. The checks
  after the create wait for the create CPU fix.

## Next

1. Create three sandboxes after the create CPU fix merges. Run one turn per
   tab and record the three answers with their metadata lines.
2. Run the Evidence panel in all three tabs and record the three hostnames.
3. Add the preview URL and the results to the specification 92 progress file.
