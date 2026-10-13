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
6. **Native async inside, valtron at the edges**: write native Rust `async fn`s where async is needed. Valtron (executors, `StreamIterator`, task pools) is used either at the edges — entry points, adapters and boundaries — or to orchestrate native valtron tasks (`TaskIterator` state machines); it is not threaded through otherwise-async core logic.

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
