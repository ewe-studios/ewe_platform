---
feature: "F19 — Unified tool model (Tool enum + single ToolDefinition)"
status: "complete"
priority: "high"
depends_on: ["F05-F09", "F14"]
blocks: ["F15"]
---

# F19 — Unified tool model

## Problem

Two types describe the same thing, drifted apart:

- `types::base_types::Tool { name, description, arguments: Option<Args>, returns:
  Option<Args> }` — the LLM-facing descriptor providers iterate.
- `agentic::tool_impl::ToolDefinition { name, description, arguments: Args,
  category }` — what every `ToolImpl::definition()` returns.

`build_toolshed` hand-copies `ToolDefinition` → `Tool` field by field. And
"a tool with several commands" is special-cased with bespoke structs
(`MemoryTool { add, replace, remove }`, `DelegationTool { start, stop, pause,
check, result }`) that each also get a **dedicated `ToolShed` field**. Every new
multi-command capability needs a new struct + a new `ToolShed` field + wiring in
`all_tools`/`build_toolshed`. That does not scale and privileges arbitrary
capabilities with named slots.

## Design

### One descriptor

```rust
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub category: String,
    pub description: String,
    pub arguments: Args,           // empty-object Args when a tool takes no args
    pub returns: Option<Args>,
}
```

Single source of truth in the types layer. `agentic::tool_impl` **re-exports**
it, so `ToolImpl::definition(&self) -> ToolDefinition` is unchanged. The old
`Tool` struct is replaced by:

### One tool, single- or multi-command

```rust
#[derive(From, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Tool {
    SingleCommand(ToolDefinition),
    MultiCommands(Vec<ToolDefinition>),
}
```

A leaf capability (`read`, `bash`) is `SingleCommand`. A capability with several
commands (`memory` → add/replace/remove; `delegate` → start/stop/pause/check/
result) is `MultiCommands` — **no bespoke struct, no dedicated slot**.

### ToolShed is just tools

```rust
pub struct ToolShed {
    pub shed: Option<Tool>,   // the discovery meta-tool
    pub tools: Vec<Tool>,     // everything else, single or multi
}
```

The `read`/`edit`/`write`/`search`/`search_files`/`shell`/`memory`/`delegate`
fields and the `MemoryTool`/`DelegationTool` structs are **deleted**. Category is
carried inside each `ToolDefinition`.

### No flattening — providers render the enum

`ToolShed::all_tools() -> Vec<Tool>` returns the **enum, un-flattened**. The
previous plan to flatten `MultiCommands` into loose `ToolDefinition`s in
`all_tools` is wrong: it throws away the grouping the enum exists to preserve,
before any provider can use it.

**Each provider turns `&[Tool]` into its own instruction format** and decides how
to express a multi-command tool:

- Function-calling providers (OpenAI chat, OpenAI responses, Anthropic) render
  each `ToolDefinition` as a function; a `MultiCommands` group may be expressed as
  several functions *or*, later, as one discriminated function — the provider owns
  that choice because it still has the whole group.
- Text/JSON formatters (llamacpp, candle `TextBasedFormatter`) describe the tool
  and its commands in the system-prompt instructions.

Dispatch is unchanged: each `ToolDefinition.name` is the `ToolCallManager`
registry key, so however a provider presents the commands, a chosen command name
routes to its `ToolImpl`.

### build_toolshed

Groups registered `ToolDefinition`s by `category`: a category with one def →
`Tool::SingleCommand`; a category with several → `Tool::MultiCommands` (sorted by
name for determinism). `memory` and `delegate` fall out of this automatically as
`MultiCommands`. Populates `ToolShed.tools`.

## Blast radius (mechanical, no behavior change)

- ~34 `ToolDefinition` sites, ~36 `Tool` field-accesses, 5 providers, ~23
  `ToolShed` slot accesses.
- Providers: openai_provider, openai_responses_provider, anthropic_messages_provider,
  candle/llamacpp `TextBasedFormatter`, `backend_utils::flatten_tools`.
- Delete `MemoryTool`, `DelegationTool`; update `MemoryTool`/`DelegationTool`
  assemblers in `build_toolshed` to emit `MultiCommands`.
- `agentic/tools/memory.rs` (F14) tools: category `memory` → assembled into a
  `MultiCommands` tool.

## Done when

One `ToolDefinition`, one `Tool` enum, `ToolShed { shed, tools }`; every provider
renders both `Tool` variants into correct instructions; no flattening in
`all_tools`; `MemoryTool`/`DelegationTool` and the special `ToolShed` fields are
gone; existing tests green + new coverage for multi-command rendering per
provider.

## Verified complete (2026-07-23)

Every `Done when` clause checked against the code, not against memory:

| Clause | Where |
|---|---|
| One `ToolDefinition` | `types/base_types.rs:1188` — the only definition; `agentic/tool_impl.rs` re-exports it, so `ToolImpl::definition()` is unchanged |
| One `Tool` enum | `types/base_types.rs:1234` — `SingleCommand(ToolDefinition)` / `MultiCommands(String, Vec<ToolDefinition>)` |
| `ToolShed { shed, tools }` | `types/base_types.rs:1393` — exactly two fields; the special slots are gone |
| No flattening in `all_tools` | `types/base_types.rs:1438` — chains `shed` + `tools`, returns `Vec<Tool>`, enum preserved |
| `MemoryTool`/`DelegationTool` gone | no descriptor structs remain in the types layer. `agentic::tools::memory::MemoryTool` still exists and is *supposed* to — it is the `ToolImpl`, which reports itself as `Tool::MultiCommands`, not a bespoke descriptor with a dedicated slot |
| Every provider renders both variants | cloud providers via the shared `Tool::function_spec()` (OpenAI, Anthropic, Responses); local backends (llama.cpp, candle) via `Tool::name()` + `Tool::arg_summary()`, both of which match on the enum |

**Deviation from the design above:** `MultiCommands` carries the group name —
`MultiCommands(String, Vec<ToolDefinition>)` rather than the
`MultiCommands(Vec<ToolDefinition>)` sketched in the Design section. The group
needs a name of its own to render as one function (`memory`), which cannot be
derived from its sub-commands.

Note `llamacpp.rs`'s `flatten_tools()` is a misleading name, not a violation:
it is a one-line passthrough to `shed.all_tools()` and returns `Vec<Tool>` with
the enum intact.

### The clause that was actually outstanding

*"new coverage for multi-command rendering per provider"* was the only unmet
one. `tests/tools/function_spec_tests.rs` covered the shared `function_spec()`
helper, but `tests/providers/` had **zero** multi-command coverage — nothing
asserted that a provider turns a `MultiCommands` into a payload its vendor
accepts. Each wraps it differently (OpenAI nests under `function`, Anthropic
uses `input_schema` and has no output-schema slot, the text formatter folds
returns into the description), so the shared helper being right does not imply
the providers are.

Closed by `tests/providers/multi_command_rendering_tests.rs` (6 tests). The
load-bearing one is `a_multi_command_tool_is_one_entry_for_every_provider`: it
guards against rendering one entry *per command*, which would show the model
`memory_add`/`memory_remove`/`memory_replace` as three unrelated tools — exactly
the flattened world F19 removed.

Verified the tests detect a regression rather than merely passing: removing the
`command` discriminator from `function_spec` fails the OpenAI and Anthropic
tests.
