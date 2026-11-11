---
feature: "Cloudflare bridge Worker"
status: "pending"
priority: "medium"
model: "sonnet"
depends_on: ["07-provider-and-doctor"]
crates: ["infrastructure/sandbox_bridge", "sandbox_agent"]
---

# Feature 08: The Cloudflare bridge Worker

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** application programming interface (API); Durable Object
(DO); WebSocket (WS); Transmission Control Protocol (TCP); Hypertext Transfer Protocol
(HTTP); JavaScript Object Notation (JSON); behaviour-driven development (BDD); continuous
integration (CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

## 1. Goal

Cloudflare has no public API that starts a container for a caller. A Worker must do it. This
feature writes that Worker, the "bridge", and gives the agent the transport that the bridge
needs. The bridge does two things only:

1. Life cycle: start and destroy the container of a named sandbox.
2. Pass-through: forward one WS upgrade request, and HTTP requests, to a port of the
   container.

All command and file logic stays in the Rust agent and the Rust client. The bridge has no
knowledge of the agent protocol.

## A. Story

- **S1**: "A host program asks the bridge to start a container for a named sandbox."
- **S2**: "The host opens one WS to the agent in that container and speaks the agent
  protocol on it."
- **S3**: "The host reaches an HTTP service on a port of a container."
- **S4**: "The host destroys the container."
- **S5**: "A caller with no token, or a wrong token, asks for anything."
- **S6**: "The container is not listening yet, crashed, or was stopped by Cloudflare for
  inactivity."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Start | host | `PUT /v1/sandboxes/{name}` | The container runs; `SandboxInfo`. |
| U2 | Inspect | host | `GET /v1/sandboxes/{name}` | `SandboxInfo`. |
| U3 | Open the agent stream | host | `GET /v1/sandboxes/{name}/agent` + upgrade | 101 and a WS to the agent; or 409. |
| U4 | Reach a port | host | `ANY /v1/sandboxes/{name}/ports/{port}/...` | The container's own answer. |
| U5 | Destroy | host | `DELETE /v1/sandboxes/{name}` | The container is gone. |
| U6 | Health | anyone | `GET /v1/health` | The bridge version. |

## B. Bounded context

**Inside this feature:** the Worker (`infrastructure/sandbox_bridge`), its DO class, its
HTTP API, the default sandbox image, and the agent transport `ws-listen`.

**Outside:** the Cloudflare container runtime (`ctx.container`); the Rust client of this API
(feature 09); the agent protocol (feature 00), which passes through untouched.

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Sandbox name | The key of one DO and its container. Matches `^[a-z0-9][a-z0-9-]{0,62}$`. | the host picks it |
| Sandbox DO | One Durable Object for each name. It is the only code that touches `ctx.container`. | `src/sandbox.ts` |
| Container | The Cloudflare container of one DO. | Cloudflare runtime, through the DO |
| Start record | `{ image, agent }` in DO storage, so `GET` can answer after a DO restart. | the DO |
| Bridge token | The secret `BRIDGE_TOKEN`. | the deployer |
| Agent stream | One WS from the host to port 1024 of the container. | the host and the agent; the bridge only forwards the upgrade |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| authorise | entity | every request but health | Compares the bearer token in constant time. |
| start | entity | `PUT` | `container.start(..)` when not running; stores the start record; sets the inactivity time-out. |
| info | entity | `GET` | Reads `container.running` and the start record. |
| forward | entity | agent or port request | `container.getTcpPort(port).fetch(request)`. |
| destroy | entity | `DELETE` | `container.destroy()` when running; clears the start record. |

**Words.** *Bridge*: the Worker. *Sandbox* (here): one name, one DO, one container. *Agent
stream*: the WS that carries agent frames. *Port proxy*: forwarding of a normal HTTP request
to a container port.

## C. Rules

### Law worksheet

```
Entity: Sandbox DO / container
Conserved value:         at most one container for each name
Bound:                   name matches the pattern; request body of PUT <= 64 KiB
Inverse pairs:           start / destroy
Idempotent verbs:        start (a second PUT with the same body changes nothing and returns 200);
                         destroy (a second DELETE returns 204); info
Ordering pairs:          start -> forward (enabling); destroy / forward (competing: after destroy, forward gives 409)
Monotone values:         none
Freeze points:           image and agent flag of a running container (a PUT with a different image gives 409 image_mismatch)
Links to other entities: SandboxInfo.running == container.running

Entity: Agent stream
Conserved value:         bytes in = bytes out, in both directions (the bridge changes nothing)
Bound:                   none in the bridge
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          byte order is kept
Monotone values:         none
Freeze points:           none
Links to other entities: at most one host at a time (the agent enforces it; a second upgrade gets 409 from the agent)
```

### Invariants

| # | Invariant |
|---|---|
| I1 | Every path but `/v1/health` needs a valid bearer token. The check is constant-time, and a failure tells the caller nothing more than 401. |
| I2 | A name that does not match the pattern never reaches a DO. |
| I3 | One name has at most one container. Two `PUT` calls at the same time start one. |
| I4 | The bridge does not read or change the bytes of the agent stream. Its source has no agent frame name. |
| I5 | `PUT` and `DELETE` are safe to repeat. |
| I6 | An image key that is not in `container.images` gives 400 `unknown_image`; nothing starts. |
| I7 | A running container keeps its image. A `PUT` that names another image gives 409 `image_mismatch`. |
| I8 | Every error answer is JSON `ErrorBody` with a code from the table in section 4. |
| I9 | The agent, on `ws-listen`, serves one host at a time and answers a second upgrade with 409. |

### Must have

- The six routes of section 4.
- The agent transport `ws-listen:<port>`.
- The default image with the agent as its entry point.

### Must do

- Set the container inactivity time-out on each start.
- Report "not listening" as its own error, so the host can retry.

### Must not

- Must not put a token, an environment value or a request body in a log line.
- Must not use a Cloudflare call that the sources do not prove (section 2).
- Must not keep state outside the DO.

## D. Acceptance scenarios

```gherkin
Feature: Cloudflare bridge Worker

  Background:
    Given the bridge runs in wrangler dev with BRIDGE_TOKEN "test"

  Scenario: Health needs no token
    When GET /v1/health is sent with no Authorization header
    Then the status is 200 and the body has a version

  Scenario: Everything else needs the token
    When GET /v1/sandboxes/a is sent with no token
    Then the status is 401
    When it is sent with the token "wrong"
    Then the status is 401

  Scenario: A bad name is refused
    When PUT /v1/sandboxes/Bad_Name is sent with the token
    Then the status is 400 and the code is bad_request

  Scenario: Start, inspect, destroy
    When PUT /v1/sandboxes/a is sent with image "sandbox" and agent true
    Then the status is 200 and running is true
    When GET /v1/sandboxes/a is sent
    Then running is true and image is "sandbox"
    When DELETE /v1/sandboxes/a is sent
    Then the status is 204
    When GET /v1/sandboxes/a is sent
    Then running is false

  Scenario: Start is safe to repeat
    When PUT /v1/sandboxes/a is sent two times with the same body
    Then both answers are 200
    And one container exists

  Scenario: An unknown image starts nothing
    When PUT /v1/sandboxes/a is sent with image "nope"
    Then the status is 400 and the code is unknown_image
    And GET /v1/sandboxes/a shows running false

  Scenario: A running sandbox keeps its image
    Given sandbox a runs the image "sandbox"
    When PUT /v1/sandboxes/a is sent with image "http-test"
    Then the status is 409 and the code is image_mismatch

  Scenario: The agent stream carries the agent's own bytes
    Given sandbox a runs with agent true
    When the host opens the agent WS and sends the 8 preface bytes
    Then the host receives the 8 preface bytes of the agent

  Scenario: The agent stream is not open before the agent listens
    Given sandbox a was started less than 50 ms ago
    When the host asks for the agent WS
    Then the status is 101, or 409 with the code not_listening

  Scenario: Only one host talks to the agent
    Given one open agent WS for sandbox a
    When a second agent WS is requested
    Then the status is 409

  Scenario: Destroy closes the stream
    Given one open agent WS for sandbox a
    When DELETE /v1/sandboxes/a is sent
    Then the WS closes

  Scenario: A port is reachable through the bridge
    Given sandbox b runs the image "http-test" with a server on port 8080
    When GET /v1/sandboxes/b/ports/8080/hello is sent
    Then the answer is the server's own answer for /hello

  Scenario: A stopped sandbox has no agent stream
    Given sandbox c does not run
    When the host asks for the agent WS
    Then the status is 409 and the code is not_running
```

## 2. Facts from the Cloudflare sources

Sources: `github.com/cloudflare/sandbox-sdk` at `61cac81`, `github.com/cloudflare/containers`
at `a17055c` (`learnings.md` section 4).

| Fact | Source |
|---|---|
| A DO with a container uses `this.ctx.container`. | `sandbox-sdk/README.md` |
| `container.running` is a boolean. | `containers/src/lib/container.ts`, lines 561 and 940 |
| `container.start({ image, instance, enableInternet, env, entrypoint, labels })`. `image` is `container.images.<key>`. `instance` is an instance type such as `"lite"`. | `sandbox-sdk/examples/minimal/src/index.ts`, lines 64 to 68; `containers/src/lib/container.ts`, lines 498 to 500 and 781 |
| `container.setInactivityTimeout(milliseconds)` returns a promise. | `sandbox-sdk/examples/artifact-workspace/src/index.ts`, lines 47 and 108 |
| `container.destroy()` returns a promise. `container.signal(number)` sends a signal. `container.monitor()` returns a promise that settles when the container exits. | `containers/src/lib/container.ts`, lines 590, 990 and 1000 |
| `container.getTcpPort(port).fetch(request)` sends an HTTP request to a container port. It streams bodies and carries WS connections. | `sandbox-sdk/docs/architecture.md`, "Adding to the public API" |
| A `fetch` to a port that nobody listens on throws an error whose message contains `the container is not listening`. | `containers/src/lib/container.ts`, line 27 (`NOT_LISTENING_ERROR`) and lines 927 to 950 |
| `wrangler.jsonc` declares `containers[]` with `class_name`, `scheduling_policy: "durable_object"`, `images.<key>.dockerfile`; a DO binding; `exports.<class>` with `storage: "sqlite"`. | `sandbox-sdk/examples/minimal/wrangler.jsonc` |
| `wrangler dev` runs the container with Docker on the local machine. | `sandbox-sdk/docs/testing.md` |

No source shows a raw TCP socket from a DO to a container port. Every use of `getTcpPort` in
both repositories calls `.fetch`. So this design uses `.fetch` only: the agent is a WS
server, and the DO forwards the upgrade request.

## 3. The agent transport `ws-listen`

This is a change to `bin/sandbox_agent` (feature 01), done in this feature.

`--transport ws-listen:<port>` is the third transport. It plugs into the transport seam of
feature 01, and it returns `AcceptNext` when a connection ends (feature 01, section 9).

| Step | Behaviour |
|---|---|
| Listen | Bind `0.0.0.0:<port>`, non-blocking, on the shared reactor. |
| Accept | Read one HTTP request. Use the WS server of `foundation_netio` (`websocket::native::server::WebSocketUpgrade` and `WebSocketServerConnection`); do not write a second WS codec. |
| Upgrade | A valid `GET` with `Upgrade: websocket` and no open host: answer 101. From here the WS is the agent byte stream. |
| Busy | A request while a host is connected: answer `409 Conflict` with the body `agent busy`, and close. |
| Not an upgrade | Any other request: answer `400 Bad Request`, and close. |
| Read | Each binary message is a chunk of the byte stream. A message is not a frame boundary; the frame decoder gets the bytes in order. A text message is a protocol error: close with WS code 1003. |
| Write | The writer task sends each agent frame as one binary message. One message is at most `max_frame_payload + 10` bytes. |
| Ping | Answer a WS ping with a pong. Send no ping. |
| Close | A WS close or a TCP end is stream loss (feature 01, section 9): kill the programs, discard the channels, accept the next host. |

The agent adds the dependency `foundation_netio` with `default-features = false` and only
the feature that the WS server needs. The verify step builds both musl targets; a part of
`foundation_netio` that does not compile for musl is fixed in the same change (`severity:
high`, it blocks this feature).

## 4. HTTP API of the bridge

All requests but health need the header `Authorization: Bearer <token>`. The token is the
Worker secret `BRIDGE_TOKEN`. The Worker compares SHA-256 digests of the two values with
`crypto.subtle.timingSafeEqual`.

| Method and path | Body | Result |
|---|---|---|
| `GET /v1/health` | none | 200 `{"version": "<bridge version>"}`. No token. |
| `PUT /v1/sandboxes/{name}` | `StartRequest` | 200 `SandboxInfo`. |
| `GET /v1/sandboxes/{name}` | none | 200 `SandboxInfo`. |
| `DELETE /v1/sandboxes/{name}` | none | 204. |
| `GET /v1/sandboxes/{name}/agent` with `Upgrade: websocket` | none | 101 from the agent, or an error. |
| `ANY /v1/sandboxes/{name}/ports/{port}/{path...}` | any | The container's answer. The bridge sends the method, the rest of the path, the query, the headers (without `Authorization`) and the streamed body. |

```ts
type StartRequest = {
  image: string;                    // a key of container.images
  enable_internet: boolean;
  env?: Record<string, string>;     // passed as start({ env })
  entrypoint?: string[];            // passed as start({ entrypoint }); omit to use the image entry point
  instance?: string;                // passed as start({ instance }); default "lite"
  inactivity_timeout_ms?: number;   // default 600000 (10 minutes, the default sleepAfter of @cloudflare/containers)
  agent: boolean;                   // true: the image runs the agent on port 1024
};
type SandboxInfo = { name: string; running: boolean; image: string | null; agent: boolean };
type ErrorBody = { error: { code: string; message: string } };
```

| Code | Status | When |
|---|---|---|
| `unauthorized` | 401 | Missing or wrong token. |
| `bad_request` | 400 | Bad name, bad JSON, body above 64 KiB, port not 1 to 65535, `agent` path on a sandbox started with `agent: false`. |
| `unknown_image` | 400 | `image` is not a key of `container.images`. |
| `image_mismatch` | 409 | The container runs with another image or agent flag. |
| `not_running` | 409 | Agent or port request, and `container.running` is false. |
| `not_listening` | 409 | `fetch` threw an error that contains `the container is not listening`. |
| `container_failed` | 502 | `start` or `fetch` threw another error. The message is the error text, cut to 1024 characters. |
| `internal` | 500 | Any other exception. |

`message` never has the token or an environment value.

### 4.1 Fault table: `PUT /v1/sandboxes/{name}`

| Step that fails | What the bridge does | What the caller sees | What is left behind |
|---|---|---|---|
| Token check | Stops. | 401 | nothing |
| Name or body check | Stops. | 400 | nothing |
| Image lookup | Stops. | 400 `unknown_image` | nothing |
| Running with another image | Stops. | 409 `image_mismatch` | the first container, unchanged |
| `container.start` throws | Clears the start record. | 502 `container_failed` | nothing |
| `setInactivityTimeout` throws | Calls `container.destroy()`; clears the start record. | 502 `container_failed` | nothing |

Two `PUT` calls at the same time: the DO runs them one after the other (a DO handles one
event at a time while it awaits storage; the start path is inside
`ctx.blockConcurrencyWhile`). The second finds `running` true and returns 200.

### 4.2 Fault table: agent and port forward

| Step that fails | What the bridge does | What the caller sees | What is left behind |
|---|---|---|---|
| `container.running` is false | Stops. | 409 `not_running` | nothing |
| `fetch` throws "not listening" | Stops. | 409 `not_listening` | the container, still starting |
| `fetch` throws another error | Stops. | 502 `container_failed` | the container (the host decides to destroy) |
| The agent answers 409 (busy) | Passes it through. | 409 with the body `agent busy` | the first stream, unchanged |

## 5. The DO

```ts
export class Sandbox extends DurableObject<Env> {
  async start(req: StartRequest): Promise<SandboxInfo>;   // inside blockConcurrencyWhile
  async info(): Promise<SandboxInfo>;
  async destroy(): Promise<void>;
  async forward(port: number, request: Request): Promise<Response>;  // getTcpPort(port).fetch(request)
}
```

- `start` reads the start record. Running with the same image and agent flag: return the
  info. Running with another: `image_mismatch`. Not running: look up
  `container.images[req.image]`, call `container.start(..)`, then
  `container.setInactivityTimeout(..)`, then store the record.
- `forward` for the agent path builds a new `Request` to `http://container/` that keeps the
  upgrade headers, and returns the `Response` of `fetch` as it is. The 101 response carries
  the WS; the Worker runtime joins the two ends.
- `forward` for a port path builds a `Request` to `http://container/<rest>?<query>`.
- The Worker entry (`src/index.ts`) does the token check, the name check and the routing:
  `env.SANDBOX.getByName(name)`.

Each request renews the inactivity time-out (`setInactivityTimeout` again). An open agent WS
counts as activity for the runtime. A container that Cloudflare stops for inactivity looks
like `running: false`; the host sees `not_running` or a closed stream and reports
`SandboxStopped` (feature 09).

## 6. Files

`infrastructure/sandbox_bridge/`:

| File | Content |
|---|---|
| `wrangler.jsonc` | Worker name `ewe-sandbox-bridge`; the DO class `Sandbox` with SQLite storage; the container declaration with the images `sandbox` and `http-test`; compatibility date `2026-09-02`. The shape is `sandbox-sdk/examples/minimal/wrangler.jsonc`. |
| `src/index.ts` | The fetch handler. |
| `src/sandbox.ts` | The DO class. |
| `src/config.ts` | The limits in one object, each with the source of its default in a comment: PUT body 64 KiB, name pattern, agent port 1024, default inactivity time-out, default instance type. |
| `images/sandbox/Dockerfile` | `FROM alpine:3.22`; `COPY sandbox-agent /usr/local/bin/sandbox-agent`; `CMD ["/usr/local/bin/sandbox-agent", "--transport", "ws-listen:1024"]`. |
| `images/http-test/Dockerfile` | `FROM alpine:3.22`; a `busybox httpd -f -p 8080` entry point with one file. For the port test only. |
| `package.json`, `tsconfig.json` | `wrangler` and TypeScript as development dependencies. No run-time dependency. |
| `README.md` | How to deploy (`npx wrangler deploy`), how to set the token (`npx wrangler secret put BRIDGE_TOKEN`), how to add an image. |

Cloudflare containers are `linux/amd64`. The image needs the
`x86_64-unknown-linux-musl` agent. A `mise` task `sandbox:bridge-image` builds the agent and
copies it to `images/sandbox/`.

## 7. Tests

`infrastructure/sandbox_bridge/test/`, run with `wrangler dev` and Docker (specifications
rule 3: the real local form exists, so no stub). The script `test/run.sh` starts
`wrangler dev`, saves its PID, waits for `/v1/health`, runs the tests, and stops the process
by that PID. With `--keep-running` it leaves the process up and prints the PID (feature 09
uses this).

| Law or scenario | Test | Oracle |
|---|---|---|
| I1, "Health needs no token", "Everything else needs the token" | `health`, `auth_required` (each route, no token and wrong token) | expected outcome |
| I2, "A bad name is refused" | `bad_name` (examples: upper case, `_`, 64 characters, empty) | expected outcome |
| "Start, inspect, destroy" | `start_info_delete` | example |
| I3, I5, "Start is safe to repeat" | `start_is_idempotent`, `start_concurrent` (two `PUT` at once; `docker ps` shows one container) | expected outcome |
| I5 | `delete_is_idempotent` | expected outcome |
| I6, "An unknown image starts nothing" | `unknown_image` | expected outcome |
| I7, "A running sandbox keeps its image" | `image_mismatch` | expected outcome |
| I4, "The agent stream carries the agent's own bytes" | `agent_pipe_carries_preface`; `agent_pipe_is_transparent` (10 MiB of random bytes through `cat` in the sandbox, compared by hash) | identity |
| "The agent stream is not open before the agent listens" | `agent_not_listening_then_ready` (retry until 101; each earlier answer is 409 `not_listening`) | expected outcome |
| I9, "Only one host talks to the agent" | `agent_pipe_single_client` | expected outcome |
| "Destroy closes the stream" | `agent_pipe_closes_on_delete` | example |
| "A port is reachable through the bridge" | `port_proxy` (GET, and a POST with a 1 MiB body that the server echoes) | identity |
| "A stopped sandbox has no agent stream" | `agent_not_running` | expected outcome |
| I8 | `errors_are_json` (each error path has an `ErrorBody` with a known code) | expected outcome |
| I4 | `no_frame_names_in_bridge` (a source scan for `Exec`, `Stdout`, `FrameKind`) | example |

Agent tests for `ws-listen`, in `bin/sandbox_agent/tests/` (Linux, no Docker). They use the
WS client of `foundation_netio` as the host.

| Scenario | Test |
|---|---|
| Upgrade then handshake then `echo` | `ws_listen_exec_echo` |
| A second upgrade while one host is connected gets 409 | `ws_listen_busy` |
| A plain `GET` with no upgrade gets 400 | `ws_listen_not_upgrade` |
| A text message closes with 1003 | `ws_listen_text_message` |
| Two hosts one after the other | `ws_listen_serves_two_hosts` |
| A frame split over two messages, and two frames in one message | `ws_listen_message_is_not_a_frame` |

## 8. Verify

```bash
cargo test -p sandbox_agent --profile uat ws_listen
cargo build -p sandbox_agent --profile agent --target x86_64-unknown-linux-musl
cd infrastructure/sandbox_bridge
npm ci
npx tsc --noEmit
mise run sandbox:bridge-image
./test/run.sh
```

## 9. Done when

- Each scenario of section D and each row of section 7 passes.
- The agent has `ws-listen`, and builds for both musl targets.
- The bridge uses only the Cloudflare calls of section 2.
- The bridge source has no agent frame name.
- The README tells a user how to deploy and how to set the token.
