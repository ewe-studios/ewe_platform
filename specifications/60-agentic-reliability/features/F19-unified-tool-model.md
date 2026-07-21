---
feature: "F19 — Unified tool model (Tool enum + single ToolDefinition)"
status: "in-progress"
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
