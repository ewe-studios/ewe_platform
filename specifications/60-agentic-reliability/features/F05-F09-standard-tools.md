---
feature: "F05–F09 — Standard agentic tools (read/write/edit/bash) on the VFS"
status: "not-started"
priority: "high"
depends_on: ["09-toolimpl-registry (spec36)", "F02"]
---

# F05–F09 — Standard tools: read / write / edit / bash

## Problem

The `ToolShed` (F10) declares `read`, `edit`, `write`, `shell` slots, and the
agent loop can execute tools — but **none of these tools are implemented**. The
only `ToolImpl`s that exist are `ShedTool`, `SearchContextTool`, `SearchFileTool`.
So the agent cannot read a file, write a file, edit a file, or run a command.

## Design

All file tools go through the **VFS** (`foundation_nativeapis::…::VfsFileSystem`)
so the underlying FS base is swappable (real FS, in-memory, overlay, remote).
The tools take a VFS handle (injected capability), never `std::fs` directly.
`bash` is native-only (target-gated `cfg(not(target_family = "wasm"))`); on wasm
it is absent or returns a clear "unsupported on this target" error.

Each tool is a `ToolImpl` (async `execute`), lives in
`backends/foundation_ai/src/agentic/tools/`, and fills its `ToolShed` slot.

### F05 — `read`

- Args: `path` (required), optional `offset`/`limit` (line range) or `bytes` range.
- Reads via VFS; returns text (with a clear error on binary/oversize).
- Rejects path escapes / enforces the VFS root (no `..` outside root).

### F06 — `write`

- Args: `path` (required), `content` (required).
- Creates or overwrites via VFS; creates parent dirs if the VFS supports it.
- Returns bytes written; enforces root.

### F07 — `edit`

- Args: `path`, `old_string`, `new_string`, optional `replace_all`.
- Reads via VFS, performs an exact-match replacement (unique unless
  `replace_all`), writes back. Errors clearly when `old_string` is absent or
  non-unique — mirrors the editor contract.

### F08 — `bash`

- Args: `command` (required), optional `cwd`, `timeout_ms`.
- Native: spawns via the platform shell, captures stdout/stderr/exit; enforces
  a timeout. Uses `tracing`, never prints. wasm: unsupported error.
- Returns `{stdout, stderr, exit_code}`.

### F09 — Defaults wiring

- `ToolCallManager` gains a way to register the standard tools given a VFS
  handle (+ exec capability). `build_toolshed` fills the `read`/`edit`/`write`/
  `shell` slots when registered.
- The tools are opt-in via capability injection (no ambient FS/exec) — an agent
  with no VFS handle gets none of them (safe default), matching how the phantom
  `shed` tool was made conditional.

## Tasks

- [ ] F05 `ReadTool` (VFS) + tests (read whole/range, missing file, root escape, binary).
- [ ] F06 `WriteTool` (VFS) + tests (create, overwrite, root escape, bytes returned).
- [ ] F07 `EditTool` (VFS) + tests (unique replace, replace_all, absent, non-unique).
- [ ] F08 `BashTool` (native, target-gated) + tests (stdout/stderr/exit, timeout, wasm-unsupported).
- [ ] F09 register standard tools with a VFS/exec capability; `build_toolshed`
      fills the slots; agent-loop integration test that the LLM can read→edit→write.
- [ ] Tests use an in-memory VFS so they run offline; a native-FS test proves the swap.

## Done when

read/write/edit/bash exist as `ToolImpl`s over the VFS, fill their `ToolShed`
slots when a VFS/exec capability is injected, are fully tested (in-memory VFS
offline + a native-FS proof), and an agent turn can drive a read→edit→write
sequence through the mock provider.
