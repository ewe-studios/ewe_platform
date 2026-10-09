# Getting Started: Tools & Presets

How to give an agent tools — the built-in **file tools** (`read`, `write`,
`edit`) and **shell** (`bash`), the **`ToolPreset`** helpers that bundle them,
the **`ToolShed`** that hands them to a session, and how the same presets
provision a sub-agent for background delegation.

For writing your *own* tools, see **Doc 10 — Building Custom Tools**. This guide
is about wiring the ones that ship with the crate.

---

## 1. The shape of a tool

Every tool is a `ToolImpl`. Three things connect it to an agent:

- The **`ToolShed`** — the one explicit list of tools a session can call. You
  build it and pass it to `with_toolshed(..)`; the session builder has no
  per-tool methods.
- The **`ToolCallManager`** on the session — the registry `build()` fills from
  the shed, which executes the calls.
- The **`shed` meta-tool** — the only tool the model is offered up front. The
  model asks `shed` for what it needs; every tool `shed` returns becomes
  *active* and is declared on later requests.

```rust
use foundation_ai::agentic::{AgentSession, ToolShed};

let agent = AgentSession::builder(router)
    .with_model("my-model")
    .with_toolshed(ToolShed::new().tool(my_tool))
    .build()?;

// What the next request declares: `shed` + the tools it has returned so far.
let declared = agent.tool_manager().offered_tools();
// Every tool the model can reach through `shed`:
let all = agent.tool_manager().all_declarations();
```

`ToolPreset` is the shortcut for several built-in tools at once.

---

## 2. The file tools: `read`, `write`, `edit`

The file tools work over a **VFS** (`AsyncVfsFileSystem`), not `std::fs`
directly. That makes the filesystem base swappable — an in-memory FS for tests,
a real disk for production, an overlay, or a remote FS — and keeps the tools
usable on wasm.

### Pick a filesystem

```rust
use std::sync::Arc;

// In-memory — great for tests and sandboxes. Nothing touches the real disk.
use foundation_nativeapis::shared::vfs::MemoryFs;
let fs = Arc::new(MemoryFs::new());

// Or a real directory on disk (native only). Everything is rooted here, so the
// agent cannot escape the sandbox by writing an absolute path.
use foundation_nativeapis::native::vfs::NativeFs;
let fs = Arc::new(NativeFs::new("/srv/agent-workspace")?);
```

### What each tool does

| Tool | Required args | Optional | Result |
|---|---|---|---|
| `read` | `path` | `offset` (1-indexed start line), `limit` (max lines) | file text (or the selected line range) |
| `write` | `path`, `content` | — | `wrote N bytes to <path>` (creates or overwrites) |
| `edit` | `path`, `old_string`, `new_string` | `replace_all` (default false) | `replaced N occurrence(s) in <path>` |

`edit` mirrors the editor contract: `old_string` must be **unique** in the file
unless `replace_all` is set, otherwise it errors rather than guessing which
occurrence you meant. Reading a non-UTF-8 file is a clean error, not a panic.

### Give them to a session

```rust
use foundation_ai::agentic::tools::files::{EditTool, ReadTool, WriteTool};

let tools = ToolShed::new()
    .tool(ReadTool::new(Arc::clone(&fs) as Arc<_>))
    .tool(WriteTool::new(Arc::clone(&fs) as Arc<_>))
    .tool(EditTool::new(Arc::clone(&fs)));
```

…or, more commonly, through a preset (section 4).

---

## 3. The shell tool: `bash`

```rust
use foundation_ai::agentic::tools::files::BashTool;

let tools = ToolShed::new().tool(BashTool::new()); // adds `bash`
```

`bash` runs a shell command and captures stdout/stderr/exit. It is **native
only** — on wasm the tool returns a clear "unsupported on this target" error
rather than silently doing nothing. Command execution is intentionally separate
from the VFS: file tools swap the FS base, but running a process only makes sense
on a native host.

---

## 4. `ToolPreset` — bundle tools in one call

A `ToolPreset` is a list of tool constructors built by named constructors and
composed with `merge()` (or `+`). It is an iterator of constructors, so it goes
straight into `ToolShed::tools(..)`.

```rust
use foundation_ai::harness::ToolPreset;

// read + write + edit + bash
let tools = ToolShed::new()
    .tools(ToolPreset::files(Arc::clone(&fs)))
    .tools(ToolPreset::shell());
```

### The constructors

| Constructor | Tools it bundles |
|---|---|
| `ToolPreset::files(fs)` | `read`, `write`, `edit` |
| `ToolPreset::shell()` | `bash` |
| `ToolPreset::memory(hierarchy)` | `memory add` / `remove` / `replace` over the hierarchy you pass |
| `ToolPreset::session_memory()` | `memory`, over the session's own hierarchy (built inside `build()`) |
| `ToolPreset::search_context()` | `search_context`, over the session's own stores and embedder (built inside `build()`) |
| `ToolPreset::agent(...)` | `agent` (background sub-agent delegation) |
| `ToolPreset::minimal_sub_agent(fs)` | `files` + `shell` |
| `ToolPreset::standard(fs)` | `files` + `shell` + `session_memory` |

`shed` is built into every session, so there is no preset for it; give the
session an embedder (`with_embedder`) and `shed` searches by embedding.

`standard` is the general-purpose set. It deliberately omits `agent` — add that
explicitly only when you want delegation, so an agent never gains the ability to
spawn sub-agents by accident.

```rust
let agent = AgentSession::builder(router)
    .with_model("my-model")
    .with_toolshed(ToolShed::new().tools(ToolPreset::standard(Arc::clone(&fs))))
    .build()?;
```

### Composing

`merge()` and `+` both combine presets:

```rust
let preset = ToolPreset::files(Arc::clone(&fs))
    + ToolPreset::shell()
    + ToolPreset::search_context();
```

A name added twice — two presets that both carry `read`, say — fails
`build()` with `ToolShedError::DuplicateTool("read")`.

### Without a session

Presets of ready-made tools also work on their own; a session-dependent tool
(`search_context`, `session_memory`) makes these return
`ToolShedError::NeedsSession`:

```rust
let manager = ToolPreset::files(Arc::clone(&fs)).into_manager(session_id)?; // fresh manager
ToolPreset::shell().register_all(agent.tool_manager())?;                    // add to a running session
let child_tools = ToolPreset::minimal_sub_agent(fs).as_child_tools()?;      // Vec<Arc<dyn ToolImpl>>
```

---

## 5. Background delegation: the `agent` tool

The `agent` tool lets a parent agent spawn a **sub-agent** to handle a
self-contained sub-task in the background — it returns a handle immediately and
the work runs on the pool. The sub-agent needs its own tools to produce output,
and that is exactly what `as_child_tools()` provides.

```rust
use foundation_ai::harness::ToolPreset;

// Tools the sub-agent gets: read/write/edit/bash, but NO memory or agent tool —
// which prevents an unbounded chain of sub-agents spawning sub-agents.
let child_tools = ToolPreset::minimal_sub_agent(Arc::clone(&fs)).as_child_tools()?;

// The agent tool itself, provisioned with those child tools.
// Doc / Mem: the sub-agents\' store types, e.g. MemoryDocumentStore and
// KvMemoryStore<MemoryStorage>.
let delegation = ToolPreset::agent::<Doc, Mem>(
    router.clone(),      // same ProviderRouter the session uses
    0,                   // current depth
    2,                   // max delegation depth (a hard cap on chains)
    default_model,       // model the sub-agent runs on
    "/srv/agent-out",    // where sub-agents write their results
    user,                // UserId for access accounting
    child_tools,
);

// Give the parent everything plus delegation.
let agent = AgentSession::builder(router)
    .with_model("my-model")
    .with_toolshed(ToolShed::new().tools(ToolPreset::standard(fs)).tools(delegation))
    .build()?;
```

`max_depth` is the safety rail: a sub-agent at the cap cannot spawn further
sub-agents, so delegation chains are bounded no matter what the model requests.

---

## 6. End to end

```rust
use std::sync::Arc;
use foundation_ai::agentic::ToolShed;
use foundation_ai::harness::{self, ToolPreset};
use foundation_ai::types::SessionId;
use foundation_nativeapis::shared::vfs::MemoryFs;

# fn demo() -> Result<(), Box<dyn std::error::Error>> {
let api_key = std::env::var("ANTHROPIC_API_KEY")?;
let fs = Arc::new(MemoryFs::new());

// 1. Model preset → session, with read/write/edit + bash.
let agent = harness::claude_session(SessionId::new(), &api_key)?
    .with_system_prompt("You are a coding assistant. Use the file tools.")
    .with_toolshed(
        ToolShed::new()
            .tools(ToolPreset::files(Arc::clone(&fs)))
            .tools(ToolPreset::shell()),
    )
    .build()?;

// 2. Confirm what the model can reach through `shed`.
let all = agent.tool_manager().all_declarations();
println!("agent has {} tool(s)", all.tools.len());

// 3. Run a turn: the model asks `shed` for file tools, then uses them.
let turn = agent.run_turn("Write 'hello' to notes.txt, then read it back.")?;
for (_, name, _) in turn.tool_calls() {
    println!("→ {name}");
}
println!("{}", turn.text());
if let Some(error) = turn.failure() {
    eprintln!("turn ended early: {error}");
}
agent.end()?;
# Ok(())
# }
```

Runnable version: `cargo run -p foundation_ai --example agent_with_tools
--features agentic`.

---

## See also

- **Doc 04 — Tools** — `ToolShed`, `shed` and activation in detail.
- **Doc 10 — Building Custom Tools** — write your own `ToolImpl`.
- **Doc 12 — Harness Presets** — the model-side presets (providers + router).
- **Doc 11 — Memory System** — the `memory` tool and `MemoryHierarchy`.
- **Getting Started: Agent Harness** — the full session lifecycle.
