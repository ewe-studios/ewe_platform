---
feature: "F15 — Agent tool (background sub-agent delegation)"
status: "complete"
deviations: "per-call `persist` override not implemented — durability comes from the D/M type parameters; see the start-contract table"
priority: "high"
depends_on: ["F14", "F19"]
source_spec: "specifications/completed/36-agentic-api (F14)"
supersedes: "the reverted run-to-completion delegate.rs"
references: [".agents/skills/rust-valtron-usage/skill.md"]
---

# F15 — Agent tool

## What it is

A **background sub-agent tool**: the caller hands a self-contained sub-task to an
LLM the user *explicitly names*, run **in the background as a valtron task** on the
**existing providers/router** the current agent already has. `start` returns a
handle immediately; the work runs on the pool; the handle can be **paused,
resumed, and closed** at any time.

The sub-agent runs in its own **disk-backed session** (recoverable by default) and
writes its result to **a file** via the standard `write` tool — so the delegator
never buffers output and **OOM is impossible by construction**. When done, the
main agent gets the **file location** (local path or remote key) and reads it with
`read`; the sub-agent's session is then cleaned up unless asked to keep it.

The tool is named **`agent`** (not `delegate` — that reads as a generic remote-job
hook). Under F19 it is one `Tool::MultiCommands("agent", [start, stop, pause,
resume, check, result])` sitting in `ToolShed.tools` — no special struct, no
dedicated slot.

## Relationship to the generic protocol

`start / stop / pause / resume / check / result` is a generic "kick off async
work, get an id, poll it, control it" protocol — an integrator could back it with
a remote service. This feature ships the **agentic** implementation: the async
work is an LLM turn on the local router.

## Why the first attempt was wrong (the valtron model)

The reverted cut called `child.run_turn(...)` — a blocking drive-to-completion
inside the tool's `async execute`, which pins a worker for the whole child turn
and can't be cancelled. Per `rust-valtron-usage`: **schedule the work with
`execute()`, keep the returned stream, and control/observe it through signals and
the stream — block only at boundaries.** `execute()` schedules the task on the
pool and hands back a `DrivenStreamIterator` *immediately*; the work runs on the
pool while the stream carries progress (`Pending` = in flight, `Next(v)` = a value
arrived, `None` at `next()` = finished).

## Design — keep it simple, build on the public APIs

We author one small **driver task** that owns the sub-agent turn. When we
`valtron::execute()` it, we hand it the control signals it needs; it drives the
sub-agent, honours the signals, routes output to a file + session, and emits a
purpose-built `AgentStatus` on its stream. The delegator just holds that stream +
the signals and watches. Nothing bespoke beyond that one driver, no side pools.

### Control handles (given to the task at creation)

1. **`stop: Arc<AtomicBool>`** — an abort flag. The task checks it recurringly at
   its boundaries; when set, it stops/aborts the sub-agent (via
   `AgentSession::abort()`, which trips the `AgentLoop`'s shared cancel signal).
   For work that needs a richer, `Send` abort (e.g. killing a shell handle mid-run),
   we can pass a more capable abort signal, or spawn a small listener thread inside
   the task that watches whatever construct we handed it and tears the work down.
2. **`pause: Arc<AtomicBool>`** — a suspend flag. When set, the task wraps it in an
   `AntiBoolEventReadiness` (watches for the bool to flip back to `false`) so it
   parks between agent-loop iterations and resumes cleanly when unpaused — no busy
   loop, no dropped state.

### The stream is a *status signal* we author — not the data path

**We own and write the task**, so its stream emits exactly the status we choose to
communicate. Note the valtron shape: the task's internal `next_status()` returns
`TaskStatus`, but the `DrivenStreamIterator` we hold yields **`Stream<Ready,
Pending>` enums** — `Stream::Next(Ready)`, `Stream::Pending(Pending)`,
`Stream::Ignore`/`Wait`/…, and finally `None`. It never hands us a `TaskStatus`.
So we make our task's `Ready = AgentStatus` and read it out of `Stream::Next`:

```rust
enum AgentStatus {
    Started,
    Working { iteration: u32 },
    Paused,
    Failed(String),
    // Terminal: emitted as the LAST item, right before the stream closes (None).
    // Carries the result location + a brief 1–2 line summary the sub-agent wrote,
    // so the delegator has both without reading the file.
    Done { location: String, summary: String },
}
```

The sub-agent's **output does not travel up this stream** — it goes to the file
(via `write`) and the session store. The stream only reports state:

- `Stream::Next(AgentStatus::…)` / `Stream::Pending` → a heartbeat: the task is
  alive and what it is doing (iteration, paused, …). We read the *status*, never a
  result payload. `Failed` marks failure.
- `AgentStatus::Done { location, summary }` → the sub-agent finished; the result is
  at `location` and here is a 1–2 line gist. This is the **last** item — we expect
  `next()` to yield `None` right after.
- `next()` → `None` → stream closed, the unambiguous completion confirmation.

Because we author the task, `check` maps the latest `AgentStatus` (plus
`is_empty()` / `is_closed()`) straight to the reported state — "still working"
(`Working`/`Paused`) vs "done" (`Done`, then `None`) — draining without parking and
without capturing output. The result is always in the file + session, never in the
delegator.

### Why OOM is structurally impossible

Two things guarantee a flat delegator footprint no matter how much the sub-agent
produces:

- **The result lives in a file** (local or remote), written by the sub-agent via
  the `write` tool — never buffered in the delegator.
- **The session lives in a store** (disk file by default, or remote) — never
  duplicated in memory.

So `check` never accumulates output; it only asks the stream **"is the result
still flowing, or is it done?"** — draining just enough to advance the task and
observe closure (`None`). And `result`, on completion, hands back the **output
location** (or, only if the content is under a small size threshold, copies the
content over) — the main agent resolves a location with its own `read` tool,
whether that's a local path or an R2 key. The default is to return the location:
the whole result is "in this file", and the main agent knows how to read it.

### Which LLM

`start` takes an **optional** `model` argument (and optional `system`); the
sub-agent is built on the **host's `ProviderRouter`** pointed at that model — the
same OpenAI / Anthropic / llama.cpp / candle providers are reused, no new wiring.
When `model` is omitted, the sub-agent **inherits the spawning session's primary
model** (the sensible default: run the sub-task on the same model the parent is
using), so a caller can delegate with just a `task`.

### Sub-agent session — durable by default, swappable underneath

The sub-agent runs in its **own `AgentSession`**, never the host's. By default it
persists to a **file-based session store on disk**, so a long delegation is
recoverable/resumable after a crash without any special handling. The store is
swappable behind the same trait: **in-memory** (explicitly, when the caller wants
throwaway), or **remote** (e.g. Cloudflare R2) in a cloud deployment — the
delegator just hands the sub-agent whatever store it is configured with; the
sub-agent is unaware of the mechanism.

`start` records the sub-agent's `SessionId` in the handle (and returns it), so the
delegation can be **resurrected/resumed** later by re-attaching to that session.

### Result capture via the `write` tool — never in the delegator

The sub-agent writes its result to **a file, continuously**, using the standard
`write` tool (F05–F09) — and reads via `read`. It is provisioned with `write` /
`read` over a **configured FS base**: a local directory by default, or a remote
backend (R2, etc.) in the cloud. **The sub-agent only ever sees `write`** — the
mechanics (local file vs R2 vs anything) are the tool implementation's concern,
which we supply. So the output never accumulates in the delegator's memory at all.

At `start` we assign the sub-agent an **output location** (a local path, or a
remote key) and instruct it (task/system prompt) to write its result there as it
works. The delegator therefore always knows where the result is, without buffering
it.

### The handle

```
AgentRun {
    id: String,               // delegation id
    session: AgentSession<D, M>, // the sub-agent's session (disk-backed) — abort + resume
    session_id: SessionId,    // recorded/returned for resume
    output_location: String,  // the file path / remote key the sub-agent writes to
    stream: DrivenStreamIterator<AgentDriverTask>, // OUR task; yields AgentStatus, not output
    stop: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    status: Running | Paused | Completed | Failed | Stopped,
    keep_session: bool,       // else the session is deleted after result retrieval
}
```
Lives in `Arc<Mutex<HashMap<String, AgentRun<D, M>>>>` on the `AgentDelegator`.
There is **no record buffer** here — output is in a file, history is in the
session store, so the footprint is flat.

### `start` — explicit contract

`start` is the only command that takes real inputs; the rest take just an `id`.
Its argument schema (the `command: "start"` branch of the `agent` tool):

| Argument | Type | Req? | Default | Meaning |
|----------|------|:----:|---------|---------|
| `command` | `"start"` | yes | — | the multi-command discriminator (F19) |
| `task` | string | **yes** | — | the self-contained sub-task the sub-agent must accomplish |
| `model` | string | no | **parent session's model** | the model id to run the sub-agent on. When omitted, inherit the spawning (parent) session's primary model. When given, it **must resolve on the host `ProviderRouter`** |
| `system` | string | no | none | extra system/role guidance prepended to the sub-agent's prompt |
| ~~`persist`~~ | `"disk" \| "memory"` | — | — | **NOT IMPLEMENTED.** Durability is set by the `D: DocumentStore` / `M: MemoryStore` type parameters the integrator picks when constructing `AgentTool<D, M>`, matching this doc's own "the delegator just hands the sub-agent whatever store it is configured with" — the child inherits those. A per-call override was not built: switching store *type* per call is not expressible through the generic parameters, and no caller has needed it. Revisit if a caller needs one delegation throwaway and the next durable. |
| `keep_session` | bool | no | `false` | keep the sub-agent session after `result` (audit / later resume) instead of deleting it |
| `max_iterations` | u32 | no | a sensible built-in cap | hard cap on the sub-agent's agent-loop iterations (runaway guard); overridable per call |

**Preconditions / errors** (all surface as `ToolError`, never a panic):

- `task` missing or empty → `InvalidArguments`.
- `model` given but does not resolve on the host router → `Execution` ("unknown
  model … "). When `model` is omitted the parent session's model is used (which is
  already known to resolve), so the common path can't fail here.
- depth ≥ `max_depth` → `Execution` ("delegation depth cap reached").

**What it does** (non-blocking): builds the sub-agent on the host router pointed at
`model` (at `depth + 1`), with a `persist`-backed session and the `write`/`read`
tools over the configured FS base; assigns an `output_location` and instructs the
sub-agent (task/system) to write its result there and finish with a short summary;
creates fresh `stop`/`pause` flags; schedules the driver task via
`valtron::execute()`; stores the `AgentRun`; **returns immediately.**

**Returns** (a small JSON object — the caller keeps `id` to drive the rest):

| Field | Type | Meaning |
|-------|------|---------|
| `id` | string | delegation id — pass to `check` / `result` / `stop` / `pause` |
| `session_id` | string | the sub-agent's session id — for resume/resurrect |
| `output_location` | string | where the result will be written (local path or remote key) |
| `status` | `"running"` | the sub-agent has been scheduled |

### Commands

- **`start`** → see the explicit contract above.
- **`check(id)`** → non-blocking: drain the stream for the latest `AgentStatus`;
  report `working { iteration }` / `paused` / `failed`, or `done` once
  `AgentStatus::Done` (then `None`) is seen. Accumulates nothing.
- **`result(id)`** → when done, return the **`location` + the 1–2 line `summary`**
  captured from `AgentStatus::Done` (the main agent `read`s the location for the
  full result; small files may be inlined). Then, unless `keep_session`, **delete
  the sub-agent's session** (cleanup) and drop the run. If not done, report
  still-running (a poll, not a block).
- **`pause(id)`** → set the `pause` flag; the task parks via `AntiBoolEventReadiness`
  at its next boundary and `check` then reports `paused`. Idempotent.
- **`resume(id)`** → clear the `pause` flag; the `AntiBoolEventReadiness` fires and
  the task wakes and continues. A no-op (with a note) if the run isn't paused.
- **`stop(id)`** → set the `stop` flag; the task aborts the sub-agent at its next
  boundary; mark `Stopped` (a late completion must not overwrite `Stopped`); apply
  the same cleanup rule as `result`.

### Cleanup

By default, once a delegation is `Completed` (or `Stopped`) **and its result has
been retrieved**, the delegator **deletes the sub-agent's session** (and may prune
the output file per policy) to reclaim disk. Pass `keep_session` to retain it for
audit or a later resume.

### Depth cap

The sub-agent is built at `depth + 1`; the delegator refuses `start` at
`max_depth`, bounding recursion.

## Testing

- `start` returns immediately with an id + session id (a slow/looping mock
  sub-agent does not block `start`).
- Poll `check` until it reports `done` (driven by `AgentStatus::Done` then `None`);
  `result` returns the file `location` + `summary`, and the file (written via the
  `write` tool) holds the full output — even though earlier `check` calls already
  drained the status stream (nothing was captured in the delegator).
- The sub-agent's session is disk-backed and its `SessionId` round-trips (resume);
  after `result` the session is deleted unless `keep_session` was set.
- A large sub-agent output does not grow the delegator's memory (result stays in
  the file; the stream only carried `AgentStatus`).
- `pause` parks the task and `check` reports `paused`; unpausing resumes to
  completion.
- `stop` sets the abort flag; the sub-agent is aborted and status flips to
  `stopped`; a later drain does not revert it.
- Depth cap: an agent at `max_depth` refuses `start`.
- Unknown ids error on `check` / `result` / `stop` / `pause`.
- The recorded `session_id` matches the sub-agent's session (resume handle).
- Registration yields one `Tool::MultiCommands("agent", …)` in `ToolShed.tools`.

## Done when

`start` never blocks and returns an id + sub-agent session id + output location;
the `stop`/`pause` `AtomicBool` handles actually abort/suspend the task; the task's
`AgentStatus` stream drives `check` (and confirms completion via `Done` → `None`);
the sub-agent runs isolated on a disk-backed session on the host router with the
requested model (or the parent's when omitted), writes its result to a file via
`write`, and `result` returns
the location + summary; the session is cleaned up unless `keep_session`; the
delegator's memory stays flat regardless of output size; and it registers as one
`agent` `MultiCommands` tool — tests covering each. Built only on the public
valtron + `AgentSession` + VFS-tool APIs; no bespoke machinery.
