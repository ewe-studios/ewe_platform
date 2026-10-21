---
purpose: Backward compatibility file for Claude AI agents
description: This file ensures compatibility with older workflows that reference CLAUDE.md. It redirects to the current AGENTS.md configuration file.
version: 1.0.0
---

# Claude AI Agent Configuration

## ⚠️ Important Notice

This file is maintained for **backward compatibility only**.

The primary configuration file for all AI agents (including Claude) has been moved to `AGENTS.md`.

---

## Primary Rule

**MANDATORY:** Before performing any tasks or operations on this project, you **MUST**:

1. **Load `AGENTS.md`** - This is the central configuration file containing all agent rules and guidelines
2. **Follow all instructions** in `.agents/AGENTS.md`.
3. **How to work**: stuff should not be left stubbed till another feature, if you are working on it now then its dependencies should have been met else we need to properly update the feature to ensure that. Dont ignore errors, dont ignore return values that are important either
4. **Stream bodies, never buffer them**: HTTP, RPC, Docker and any other I/O bodies are consumed as streams (`Stream`/`StreamIterator`/`FutureStream`), never read whole into memory. Check the status before touching the body; on error, read only a bounded amount for the message. Reading a full body is only acceptable in tests, or when the body is small and its contents are needed for the next step (e.g. a small JSON response that is parsed and acted on).
5. **Use the generated types**: the docker, cloud-provider and other platform crates have generated request/response types. Use them instead of hand-written structs or untyped JSON. If a generated type is missing or wrong, fix the generator and regenerate — don't work around it.
6. **Async all the way; valtron composes with it**: write async logic as native Rust `async fn`s all the way down. Valtron hooks into async rather than replacing it — it can drive a future (`execute`), and its streams/tasks work inside async by becoming a future or future stream (`into_ready_future`, `into_collect_future`, `into_future_stream`, `into_next_stream`). Valtron's existing rules on when a task may cross into async still apply (`Send + 'static`, the `!Send` constraint, blocking only at boundaries); see `.agents/skills/rust-valtron-usage`. **Public APIs that wait are async by default** (`async fn`, or a returned `'static` future / async `Stream`) — write them once for the async world, with no sync twin. A caller that needs sync wraps it with valtron (`block_on_future(fut)`, `execute(from_stream(stream))`). Never consume a valtron stream with a synchronous loop that `continue`s on `Wait`: on wasm (one thread) that spins forever; park instead (`.await`, `park_on`, `into_future_stream`).
7. **Tests live in `tests/`, never inline**: every test goes in the crate's `tests/` directory (registered in its `tests/**/mod.rs`). Never add a `#[cfg(test)] mod tests` to a source file. If you touch a source file that has one, move its tests into `tests/` in the same change. Only public API is tested from there; if something needs testing and isn't reachable, make the right thing public rather than inlining the test.
8. **Limits are configuration, not constants**: size caps, timeouts, buffer and batch sizes are fields on the relevant config (e.g. `ClientConfig`, `FrameLimits`) with builder methods and documented defaults. Never hard-code one as a `const` in the code path. Pick defaults from what established servers use (nginx, Caddy/Go `net/http`) and say which in the doc comment.
9. **Every bug found gets tracked and scheduled**: any bug or limitation you notice, in code you changed or not, and however old, goes on the task list and is scheduled for a fix. "It predates this change" is never a reason to leave it unrecorded. Record it in [`docs/bug-backlog.md`](./docs/bug-backlog.md) (grouped by crate, with file, problem and likely fix) in the same change, and delete its entry in the PR that fixes it.
10. **Keep the spec's `learnings.md` and `progress.md` current**: before stopping any work, and after finishing any work, update the relevant spec's `learnings.md` (what was learned, including dead ends and root causes) and `progress.md` (what landed, what is in progress, what is next). Create them if the spec doesn't have them yet.

---

## Migration Notice

If you're referencing `CLAUDE.md`:

- Please update your workflows to reference `.agents/AGENTS.md` instead
- `CLAUDE.md` may be deprecated in future versions
- All new rules and updates will be added to `.agents/AGENTS.md`.

---

## Quick Start

👉 **Go to [`AGENTS.md`](./.agents/AGENTS.md)** to get started.

---

_This file redirects to .agents/AGENTS.md for current agent configuration_
