# Getting Started: Tools & Presets

How to give an agent tools — the built-in **file tools** (`read`, `write`,
`edit`) and **shell** (`bash`), the **`ToolPreset`** helpers that bundle them
into one call, and how the same preset provisions a sub-agent for background
delegation.

For writing your *own* tools, see **Doc 10 — Building Custom Tools**. This guide
is about wiring the ones that ship with the crate.

---

## 1. The shape of a tool

Every tool is an `Arc<dyn ToolImpl>`. Two things connect it to an agent:

- The **`ToolCallManager`** on a session — where a tool is *registered* and later
  *executed*.
- The **`ToolShed`** — the declaration the model sees, built from the registered
  tools via `tool_manager().build_toolshed()`.

You register tools after building the session, then the model discovers them
through the shed:

```rust
let agent = builder.build()?;

// register → the manager can execute it
agent.tool_manager().register(my_tool);

// build_toolshed → what the model is told exists
let shed = agent.tool_manager().build_toolshed();
```

`ToolPreset` is the shortcut for registering several built-in tools at once.

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

### Register them

Individually:

```rust
use foundation_ai::agentic::tools::files::register_file_tools;

register_file_tools(agent.tool_manager(), Arc::clone(&fs));
```

…or, more commonly, through a preset (next section).

---

## 3. The shell tool: `bash`

```rust
use foundation_ai::agentic::tools::files::register_shell_tool;

register_shell_tool(agent.tool_manager()); // adds `bash`
```

`bash` runs a shell command and captures stdout/stderr/exit. It is **native
only** — on wasm the tool returns a clear "unsupported on this target" error
rather than silently doing nothing. Command execution is intentionally separate
from the VFS: file tools swap the FS base, but running a process only makes sense
on a native host.

---

## 4. `ToolPreset` — bundle tools in one call

Wiring each `register_*` by hand is boilerplate. A `ToolPreset` is a collection
of tools built by named constructors, composed with `merge()` (or `+`), and
stamped onto a manager with `register_all()`.

```rust
use foundation_ai::harness::ToolPreset;

// read + write + edit + bash
let preset = ToolPreset::files(Arc::clone(&fs))
    .merge(ToolPreset::shell());

preset.register_all(agent.tool_manager());
```

### The constructors

| Constructor | Tools it bundles |
|---|---|
| `ToolPreset::files(fs)` | `read`, `write`, `edit` |
| `ToolPreset::shell()` | `bash` |
| `ToolPreset::memory(hierarchy)` | `memory add` / `remove` / `replace` |
| `ToolPreset::shed(discovery)` | `shed` (tool discovery) |
| `ToolPreset::agent(...)` | `agent` (background sub-agent delegation) |
| `ToolPreset::minimal_sub_agent(fs)` | `files` + `shell` |
| `ToolPreset::standard(fs, hierarchy, discovery)` | `files` + `shell` + `memory` + `shed` |

`standard` is the general-purpose set. It deliberately omits `agent` — add that
explicitly only when you want delegation, so an agent never gains the ability to
spawn sub-agents by accident.

```rust
use foundation_ai::harness::ToolPreset;

let preset = ToolPreset::standard(
    Arc::clone(&fs),
    Arc::clone(&memory_hierarchy),
    Arc::clone(&tool_discovery),
);
preset.register_all(agent.tool_manager());
```

### Composing

`merge()` and `+` both combine presets:

```rust
let preset = ToolPreset::files(Arc::clone(&fs))
    + ToolPreset::shell()
    + ToolPreset::memory(Arc::clone(&hierarchy));
```

### Building a manager directly

If you want a `ToolCallManager` populated from a preset without going through a
session first:

```rust
let manager = ToolPreset::files(fs).into_manager(session_id);
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
let child_tools = ToolPreset::minimal_sub_agent(Arc::clone(&fs)).as_child_tools();

// The agent tool itself, provisioned with those child tools.
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
let preset = ToolPreset::standard(fs, hierarchy, discovery).merge(delegation);
preset.register_all(agent.tool_manager());
```

`max_depth` is the safety rail: a sub-agent at the cap cannot spawn further
sub-agents, so delegation chains are bounded no matter what the model requests.

---

## 6. End to end

```rust
use std::sync::Arc;
use foundation_ai::harness::{self, ToolPreset};
use foundation_ai::agentic::KvMemoryStore;
use foundation_ai::types::{
    MessageRole, Messages, SessionId, TextContent, UserModelContent,
};
use foundation_compact::ids::new_scru128;
use foundation_db::{MemoryDocumentStore, MemoryStorage};
use foundation_nativeapis::shared::vfs::MemoryFs;

type Doc = MemoryDocumentStore;
type Mem = KvMemoryStore<MemoryStorage>;

# fn demo() -> Result<(), Box<dyn std::error::Error>> {
let api_key = std::env::var("ANTHROPIC_API_KEY")?;
let fs = Arc::new(MemoryFs::new());

// 1. Model preset → session.
let agent = harness::claude_session::<Doc, Mem>(SessionId::new(), &api_key)?
    .with_system_prompt("You are a coding assistant. Use the file tools.")
    .build()?;

// 2. Tools: read/write/edit + bash.
ToolPreset::files(Arc::clone(&fs))
    .merge(ToolPreset::shell())
    .register_all(agent.tool_manager());

// 3. Confirm what the model will see.
let shed = agent.tool_manager().build_toolshed();
println!("agent has {} tool(s)", shed.tools.len());

// 4. Run a turn.
let prompt = Messages::User {
    id: new_scru128(),
    role: MessageRole::User,
    content: UserModelContent::Text(TextContent {
        content: "Write 'hello' to notes.txt, then read it back.".into(),
        signature: None,
    }),
    signature: None,
};
let records = agent.run_turn(prompt)?;
for record in &records {
    println!("{record:?}");
}
agent.end()?;
# Ok(())
# }
```

Runnable version: `cargo run -p foundation_ai --example agent_with_tools
--features agentic`.

---

## See also

- **Doc 10 — Building Custom Tools** — write your own `ToolImpl`.
- **Doc 12 — Harness Presets** — the model-side presets (providers + router).
- **Doc 11 — Memory System** — the `memory` tool and `MemoryHierarchy`.
- **Getting Started: Agent Harness** — the full session lifecycle.
