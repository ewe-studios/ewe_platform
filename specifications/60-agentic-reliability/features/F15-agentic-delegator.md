---
feature: "F15 — Agentic delegator (background LLM delegation)"
status: "in-progress"
priority: "high"
depends_on: ["F14", "F19"]
source_spec: "specifications/completed/36-agentic-api (F14)"
supersedes: "the reverted run-to-completion delegate.rs"
references: [".agents/skills/rust-valtron-usage/skill.md"]
---

# F15 — Agentic delegator

## What it is

A **real background-LLM delegation tool**: the caller hands off a sub-task to an
LLM the user *explicitly names*, run **in the background as a valtron task** using
the **existing providers/router** the current agent already has. The tool returns
a handle immediately; the work streams to completion on the pool; the handle can
be **closed (aborted) easily** at any time.

It is a `MultiCommands` tool (F19) — one logical `delegate` tool exposing the
commands `start` / `stop` / `pause` / `check` / `result`. There is no special
`DelegationTool` struct or `ToolShed.delegate` slot; it is just another
`Tool::MultiCommands` in `ToolShed.tools`.

## Relationship to the generic slot

The generic delegate protocol ("kick off async work, get an id, poll it") is
capability-agnostic — an integrator could back it with a remote service. This
feature ships the **agentic** implementation of that protocol: the async work is
an LLM turn on the local router.

## Why the first attempt was wrong (valtron model)

The reverted version called `child.run_turn(...)` — a blocking drive-to-completion
inside the tool's `async execute`. Per `rust-valtron-usage`: **schedule with
`execute()`, keep the returned stream, block only at boundaries.** `execute()`
(which `AgentSession::run_turn_stream` calls internally) schedules the task on the
pool and returns a `DrivenStreamIterator` *immediately*; the work runs on the pool
while the stream carries progress (`Pending` = in flight, `Next(v)` = value,
end = closed). Cancellation is the **agent's** hook — `AgentSession::abort()`
trips the `AgentLoop`'s shared cancel signal — not a valtron primitive.

## Design

### Which LLM

`start` takes an explicit `model` argument (and optional `system`); the delegator
resolves it against the **current agent's `ProviderRouter`** (all existing
providers are reusable). So delegation reuses the same OpenAI/Anthropic/llama.cpp/
candle providers already configured — no new provider wiring.

### The background task = a child turn's stream

`start`:
1. depth-cap check (a child at `max_depth` cannot delegate further);
2. build a child `AgentSession` via an injected `spawner(SessionId, u32, model,
   system)` that clones the router and points the child at the requested `model`;
3. `stream = child.run_turn_stream(user_msg(task))` — schedules the child
   `AgentLoop` on the pool, returns at once;
4. store `DelegationRun { stream, control: child.clone(), records: [], Running }`
   under a fresh id;
5. return the id **immediately** (never blocks the caller).

`DrivenStreamIterator` is `Send`, so the run lives in
`Arc<Mutex<HashMap<String, DelegationRun<D, M>>>>`.

### Commands

- **`start(task, model, [system])`** → schedule + return id (`Running`).
- **`check(id)`** → non-blocking drain: pull ready items (`is_empty()`-guarded),
  accumulate `Next` `SessionRecord`s, mark `Completed` on stream end (or `Failed`
  on a terminal `FailedAction`); report status. Does not park.
- **`result(id)`** → drain like check; return the child's assistant text if
  `Completed`, else report still-running (a poll, not a block).
- **`stop(id)`** → `control.abort()` (closes the background task at the loop's next
  boundary); drain final records; mark `Stopped`; a late completion must not
  overwrite `Stopped`.
- **`pause(id)`** → an agent turn has no suspend point; report that honestly.

### Depth cap

`spawner` builds children at `depth + 1`; the delegator refuses `start` at
`max_depth`, bounding recursion.

## Execution note

We launch **one valtron task** — the child turn, via `run_turn_stream` →
`execute(agent_loop)` — into the valtron loop, and hold the returned stream as
the handle. We are not spawning workers or managing threads; valtron owns how the
task is driven. The delegator's job is only to (a) launch the task, (b) observe
it through the stream (`check`/`result` pull ready values and detect completion),
and (c) close it (`stop` → `abort()`).

## Testing

- `start` returns immediately with `Running` + id (a slow/looping mock child does
  not block `start`).
- Poll `check` until `completed`; `result` returns the mock child's output.
- Depth cap: a delegator at `max_depth` refuses `start`.
- `stop` aborts the child and flips to `stopped`; a later drain does not revert it.
- Unknown ids error on `check`/`result`/`stop`.
- Registration yields a single `delegate` `MultiCommands` tool with the five
  commands, present in `ToolShed.tools`.

## Done when

`start` never blocks, the commands reflect real stream/agent state, `stop`
actually closes the background task, the depth cap holds, delegation runs on the
existing router with a user-named model, and it registers as one `MultiCommands`
`delegate` tool — tests covering each.
