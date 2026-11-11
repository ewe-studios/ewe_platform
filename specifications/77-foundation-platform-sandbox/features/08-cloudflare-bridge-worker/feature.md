---
feature: "Cloudflare bridge Worker"
status: "pending"
priority: "medium"
model: "sonnet"
depends_on: ["07-provider-and-doctor"]
crates: ["infrastructure/sandbox_bridge"]
---

# Feature 08: The Cloudflare bridge Worker

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); Durable Object
(DO); WebSocket (WS); Transmission Control Protocol (TCP); Hypertext Transfer Protocol
(HTTP); JavaScript Object Notation (JSON); continuous integration (CI); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Cloudflare has no public API that starts a container for a caller. A Worker must do it. This
feature writes that Worker, the "bridge". It does two things only:

1. Life cycle: start and destroy the container of a named sandbox.
2. Pipe: connect a WS from the host to the agent in the container.

All command and file logic stays in the Rust agent and the Rust client. The bridge has no
knowledge of the agent protocol.

## 2. Measured facts

From `github.com/cloudflare/sandbox-sdk` at `61cac81` (`learnings.md` section 4):

- A DO with a container uses `this.ctx.container`. Calls seen in the examples:
  `running`, `start({ image, enableInternet })`, `images.<name>`, `exec(..)`,
  `getTcpPort(port).fetch(..)`, `destroy()`.
- `docs/architecture.md` says that `getTcpPort(port).fetch()` streams bodies and carries
  WS connections.
- `wrangler.jsonc` in `examples/minimal` declares `containers[]` with `class_name`,
  `scheduling_policy: "durable_object"` and `images.<name>.dockerfile`, a DO binding, and
  the compatibility date `2026-09-02`.
- `wrangler dev` runs the container with Docker on the local machine.

## 3. Measure first

Before any other work, answer these with a small Worker in `wrangler dev`. Write the answers
in `learnings.md`.

| # | Question | Why |
|---|---|---|
| M1 | Does `this.ctx.container.getTcpPort(1024).connect()` (or an equal call) give a raw TCP socket from the DO to the container? | It decides the pipe design (section 5). |
| M2 | How does the DO know that the agent listens? Does a `connect()` or `fetch()` fail fast before the port is open? | It decides the ready wait. |
| M3 | Does the container keep running when no request is active? What stops it (an inactivity time-out)? | It decides the keep-alive rule. |
| M4 | Which options does `start()` take for environment variables and for the entry point? | Needed for `PUT /v1/sandboxes/{name}`. |
| M5 | What is the largest WS message that a Worker accepts? | The agent frame is at most 64 KiB + 10 bytes; confirm that it fits. |

## 4. HTTP API of the bridge

All requests need the header `Authorization: Bearer <token>`. The token is the Worker secret
`BRIDGE_TOKEN`. The Worker compares it in constant time. A missing or wrong token gives 401
with no body detail.

`{name}` matches `^[a-z0-9][a-z0-9-]{0,62}$`. A different name gives 400.

| Method and path | Body | Result |
|---|---|---|
| `PUT /v1/sandboxes/{name}` | `StartRequest` | 200 `SandboxInfo`. Starts the container if it does not run. Safe to repeat. |
| `GET /v1/sandboxes/{name}` | none | 200 `SandboxInfo`. |
| `DELETE /v1/sandboxes/{name}` | none | 204. Destroys the container. Safe to repeat. |
| `GET /v1/sandboxes/{name}/agent` with `Upgrade: websocket` | none | 101. The WS carries the agent byte stream (section 5). 409 when the container does not run. |
| `ANY /v1/sandboxes/{name}/ports/{port}/{path...}` | any | The answer of `getTcpPort(port).fetch()` for the same method, path, headers and streamed body. For the Cloudflare Containers provider (feature 09). |
| `GET /v1/health` | none | 200 `{"version": "<bridge version>"}`. No token needed. |

```ts
type StartRequest = {
  image: string;                 // a key of container.images, for example "sandbox"
  enable_internet: boolean;
  env?: Record<string, string>;  // only if M4 shows that start() accepts it
  agent: boolean;                // true: the image runs the agent on port 1024
};
type SandboxInfo = { name: string; running: boolean; image: string | null; agent: boolean };
type ErrorBody = { error: { code: string; message: string } };
```

Error codes: `unauthorized` (401), `bad_request` (400), `unknown_image` (400),
`not_running` (409), `container_failed` (502), `internal` (500).

## 5. The agent pipe

```mermaid
sequenceDiagram
  participant H as Host (Rust, AgentClient)
  participant W as Worker fetch handler
  participant D as Sandbox DO
  participant A as sandbox-agent in the container
  H->>W: GET /v1/sandboxes/x/agent (Upgrade)
  W->>D: forward the request
  D->>A: open TCP to port 1024
  D-->>H: 101 Switching Protocols
  H->>D: WS binary message (bytes)
  D->>A: TCP write
  A->>D: TCP read
  D->>H: WS binary message (bytes)
  Note over H,A: the agent protocol runs end to end; the DO copies bytes
```

_Caption: the DO turns WS messages into TCP bytes and back; it does not read the frames._

Design with M1 = yes (preferred):

- The DO opens a TCP socket to port 1024 and accepts the WS.
- WS to TCP: each binary message is written to the socket. The DO awaits the write before
  it reads the next message, which gives back pressure. A text message closes the WS with
  code 1003.
- TCP to WS: each chunk read from the socket is sent as one binary message. A WS message is
  not a frame boundary; the client must treat the messages as one byte stream.
- When one side closes, the DO closes the other side.
- Only one agent WS is open for each sandbox. A second request gets 409 `agent_busy`.

Fall-back with M1 = no:

- The agent gets a fourth transport, `ws-listen:<port>`: a minimal HTTP upgrade and WS
  framing (binary messages, close, ping and pong), written in the agent with no new
  dependency (a SHA-1 and a Base64 function for the accept key).
- The DO forwards the upgrade request with `getTcpPort(1024).fetch(request)`.
- This adds about 300 lines to the agent. It is a change to feature 01's program, done in
  this feature, with tests beside the agent tests.

Record which design is in use in `learnings.md`. Feature 09 is the same for both.

## 6. Files

`infrastructure/sandbox_bridge/`:

| File | Content |
|---|---|
| `wrangler.jsonc` | Worker name `ewe-sandbox-bridge`, the DO class `Sandbox`, the container declaration with one image `sandbox`, the compatibility date. Copy the shape of `examples/minimal/wrangler.jsonc`. |
| `src/index.ts` | The fetch handler: token check, name check, routing to the DO by name (`env.SANDBOX.getByName(name)`). |
| `src/sandbox.ts` | The DO class: `start`, `info`, `destroy`, `agent` (the pipe), `port` (the proxy). |
| `Dockerfile` | The default sandbox image: Alpine, `COPY sandbox-agent /usr/local/bin/sandbox-agent`, `CMD ["/usr/local/bin/sandbox-agent", "--transport", "tcp-listen:1024"]`. |
| `package.json`, `tsconfig.json` | `wrangler` and TypeScript only. No run-time dependency. |
| `README.md` | How to deploy, how to set `BRIDGE_TOKEN`, how to add an image. |

The image needs the `x86_64-unknown-linux-musl` agent (Cloudflare containers are
`linux/amd64`). A `mise` task `sandbox:bridge-image` builds the agent and copies it beside
the `Dockerfile`.

Limits in the bridge are in one `config.ts` object, with the source of each default in a
comment: the largest request body for `PUT` (64 KiB), the name pattern, the agent port.

The agent in a container gets `RootSource::Plain` (feature 00, section 2.4): the container
root is already a private copy of the image, so the agent does no mount work. Feature 09
sends it.

## 7. Tests

`infrastructure/sandbox_bridge/test/` (run with `wrangler dev` and Docker; specifications
rule 3). A script `test/run.sh` starts `wrangler dev`, saves its PID, runs the tests, and
stops it by that PID.

| Test | Proves |
|---|---|
| `health` | `GET /v1/health` gives 200 with no token. |
| `auth_required` | Each other path gives 401 with no token and with a wrong token. |
| `bad_name` | `PUT /v1/sandboxes/Bad_Name` gives 400. |
| `start_info_delete` | `PUT`, then `GET` shows `running: true`, then `DELETE`, then `GET` shows `running: false`. |
| `start_is_idempotent` | Two `PUT` calls give 200, and one container exists. |
| `agent_pipe_carries_preface` | Open the WS, send the 8 preface bytes, receive the 8 preface bytes of the agent. |
| `agent_pipe_single_client` | A second WS gives 409. |
| `agent_pipe_closes_on_delete` | `DELETE` closes the WS. |
| `port_proxy` | With an image that runs a small HTTP server on 8080: `GET /v1/sandboxes/x/ports/8080/` returns its body. |

## 8. Verify

```bash
cd infrastructure/sandbox_bridge
npm ci
npx tsc --noEmit
mise run sandbox:bridge-image
./test/run.sh
```

## 9. Done when

- The answers to M1 to M5 are in `learnings.md`.
- The tests of section 7 pass in `wrangler dev`.
- The bridge has no knowledge of agent frames: `grep -n "Exec\|Stdout\|FrameKind" src/` in
  the bridge finds nothing.
- The README tells a user how to deploy with `wrangler deploy` and how to set the token.
