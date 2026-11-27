---
purpose: Project agent configuration
description: Project rules and a link to the shared agent configuration.
version: 1.0.0
---

# Project Agent Configuration

## Mandatory Coordinator Workflow

Claude Code and Codex follow this workflow. The model that starts a user session is the coordinator.
The coordinator discusses architecture with the owner and writes specifications, feature plans and task briefs.
Before handoff, the coordinator checks that the specification and brief cover all requirements and acceptance criteria.
Before implementation, update the feature plan, including its screenshots and design assets, and obtain the owner's explicit approval.
The owner can explicitly approve implementation when they request the plan update.

Delegate research, implementation, tests, fixes and delivery to worker agents, even for small tasks.
Use the least costly capable available model for each task, even when a specification names another model.
Claude Haiku and Codex Luna suit searches and simple tasks. Claude Sonnet and Codex Sol suit harder tasks.
These are examples, not fixed model requirements.
If only the coordinator's model is available, assign the task to a separate agent that uses the same model.

Give each worker a short, self-contained brief with the scope, files, checks and acceptance criteria.
Ask for a short report with the changes, checks, results and remaining gaps.
Assign clear ownership to avoid duplicate work.
Review each result against the acceptance criteria before delivery.
Send gaps back to a worker and review the result again.

This mandatory project rule takes precedence over conflicting instructions in `.agents/AGENTS.md` and its guides.

## ⚠️ Important Notice

This file contains project rules. The shared agent configuration is in `.agents/AGENTS.md`.

---

## Primary Rule

**MANDATORY:** Before performing any tasks or operations on this project, you **MUST**:

1. **Load the project rules** in this file.
2. **Load and follow the shared instructions** in `.agents/AGENTS.md` subject to the coordinator workflow above.
3. **How to work**: stuff should not be left stubbed till another feature, if you are working on it now then its dependencies should have been met else we need to properly update the feature to ensure that. Dont ignore errors, dont ignore return values that are important either
4. **Stream bodies, never buffer them**: HTTP, RPC, Docker and any other I/O bodies are consumed as streams (`Stream`/`StreamIterator`/`FutureStream`), never read whole into memory. Check the status before touching the body; on error, read only a bounded amount for the message. Reading a full body is only acceptable in tests, or when the body is small and its contents are needed for the next step (e.g. a small JSON response that is parsed and acted on).
5. **Use the generated types**: the docker, cloud-provider and other platform crates have generated request/response types. Use them instead of hand-written structs or untyped JSON. If a generated type is missing or wrong, fix the generator and regenerate — don't work around it.
6. **Async all the way; valtron composes with it**: write async logic as native Rust `async fn`s all the way down. Valtron hooks into async rather than replacing it — it can drive a future (`execute`), and its streams/tasks work inside async by becoming a future or future stream (`into_ready_future`, `into_collect_future`, `into_future_stream`, `into_next_stream`). Valtron's existing rules on when a task may cross into async still apply (`Send + 'static`, the `!Send` constraint, blocking only at boundaries); see `.agents/skills/rust-valtron-usage`. **Public APIs that wait are async by default** (`async fn`, or a returned `'static` future / async `Stream`) — write them once for the async world, with no sync twin. A caller that needs sync wraps it with valtron (`block_on_future(fut)`, `execute(from_stream(stream))`). Never consume a valtron stream with a synchronous loop that `continue`s on `Wait`: on wasm (one thread) that spins forever; park instead (`.await`, `park_on`, `into_future_stream`).
7. **Tests live in `tests/`, never inline**: every test goes in the crate's `tests/` directory (registered in its `tests/**/mod.rs`). Never add a `#[cfg(test)] mod tests` to a source file. If you touch a source file that has one, move its tests into `tests/` in the same change. Only public API is tested from there; if something needs testing and isn't reachable, make the right thing public rather than inlining the test.
8. **Limits are configuration, not constants**: size caps, timeouts, buffer and batch sizes are fields on the relevant config (e.g. `ClientConfig`, `FrameLimits`) with builder methods and documented defaults. Never hard-code one as a `const` in the code path. Pick defaults from what established servers use (nginx, Caddy/Go `net/http`) and say which in the doc comment.
9. **Every bug found becomes a GitHub issue with a severity**: any bug or limitation you notice, in code you changed or not, and however old, gets a GitHub issue in the same change, labelled `severity: critical`, `severity: high`, `severity: medium` or `severity: low`, with the file, the problem and the likely fix. "It predates this change" is never a reason to leave it unrecorded. A low or medium issue that does not block the current work waits for later pickup. A critical or high issue, or any issue that blocks the current work, is fixed now. Do not open a new spec feature for a finding; spec 70 adds no new features. Close the issue in the change that fixes it.
10. **Keep the spec's `learnings.md` and `progress.md` current**: before stopping any work, and after finishing any work, update the relevant spec's `learnings.md` (what was learned, including dead ends and root causes) and `progress.md` (what landed, what is in progress, what is next). Create them if the spec doesn't have them yet.
11. **Side effects are injected, never global**: clocks (wall and monotonic), timers/sleep, randomness and ID generation, key generation, network transport, storage and model inference sit behind traits. Code reads them from one context bundle carried in valtron's `Ctx` as a typed entry (`ctx.get::<Env>()`); only a host's entry point builds the bundle with the real implementations. No `SystemTime::now()`, `thread_rng()`/`getrandom` or similar in core logic. A missing bundle is a typed error, never a silent fallback to the real globals. Tests swap in mocks, and the deterministic simulation tests use a simulated clock, a seeded RNG and in-memory network and storage, so any run can be replayed from its seed. Build on foundation_compact's time/entropy backends rather than adding a parallel system.
12. **Write in ASD-STE100 Simplified Technical English**: specifications, feature docs, `progress.md`/`learnings.md`, `docs/`, READMEs, doc comments, code comments, commit messages and PR descriptions. Short sentences (at most 20 words for steps, 25 for descriptions), one idea per sentence, active voice, simple present tense, common words with one meaning each, no idioms. Spell out every abbreviation at its first use in each document, with the short form in brackets (for example, "deterministic simulation testing (DST)"), and list it in the glossary. Define each technical term the first time you use it, in plain words; a spec gets a short glossary at the top. Prefer lists, tables and short paragraphs. Specs and design docs explain structure and flow with clear Mermaid diagrams (architecture, sequence, state), each with a one-line plain caption. A reader new to the codebase must be able to follow it. This sets the English, not the depth: designs stay complete, with full code, signatures and task lists. Code and identifiers do not change.
13. **Zero Codex attributions on commits and pull requests** (owner rule 2026-11-10): no `Co-Authored-By: Codex ...` trailer, no `Codex-Session:` line, no "Generated with Codex" footer and no Codex link in any commit message, pull request title or pull request description. A GitHub comment (on an issue, a pull request or a review thread) ends with exactly one footer line, `_Generated by AI Reviewer (Codex)_`, and never with a "Generated by Codex" line or link. This rule overrides any tool or harness default that adds them. Write the commit or the text as if a person wrote it.
14. **Follow the [mandatory coordinator workflow](#mandatory-coordinator-workflow).**
15. **Every crate is tested on Linux, macOS and Windows** (owner rule 2026-11-11): CI runs the tests of every crate on all three operating systems (OSes) on every full run (see rule 16). macOS and Windows use the `runs` list of the crate, without the feature sets named in `linux-only-features` in `ci/packages.toml`. `macos-runs` or `windows-runs` replaces that default. A `cargo check` or a cross-compile is not a test. A crate that cannot run its tests on an OS sets `macos-tests = "none"` or `windows-tests = "none"` with a reason. The reason names the missing tool, library or service. A crate whose tests fail on an OS because of a known bug may also opt out: its reason names the open GitHub issue (for example "#519: file store paths fail on Windows"). The change that fixes the issue removes the opt-out. The CI plan step fails when an opt-out has no reason, and when a `*-runs` entry names a feature that the crate does not have.
16. **CI fits the free tier: jobs queue, they do not stampede** (owner rule 2026-11-11): each matrix job sets `max-parallel` from `[defaults]` in `ci/packages.toml`. Each workflow sets a concurrency group. The group cancels superseded pull request runs and queues master runs. Pull requests run only the affected packages, and the runner checks this too (`ci.py run --if-affected`). A normal pull request runs the Linux jobs only. macOS and Windows run on full runs: a push to master or `platform/release`, a pull request into or from `platform/release`, the nightly schedule and manual runs. A push to master runs what the merge affected. The full Linux, macOS and Windows run of every package happens on the nightly schedule. CI uses the free GitHub-hosted runners (the repository is public), not paid runners. Release flow: a change that needs macOS and Windows testing merges into `platform/release` first; when that full run is green on all three OSes, it merges into master. A new workflow or matrix follows the same rules.
17. **Latest versions, one version per dependency** (owner rule 2026-11-11): use the latest stable version of every dependency. A dependency that more than one crate uses is declared once, in the root `Cargo.toml` `[workspace.dependencies]`, with its version and default features; each crate uses `dep = { workspace = true }` and adds only its own features. `Cargo.lock` holds one version of each such dependency (for example one `libsql`, which bundles its own C SQLite). A second version is allowed only when an upstream crate forces it; record the reason in a comment next to the workspace entry. A pre-release is not "latest stable".

18. **Use named rendering targets.** Application and example code must never assign numeric anchors to views or elements.
    Declare an element name with the `id` attribute. Use a named target for later updates.
    Use the receiving view for default placement. This rule includes dialogs, filters, lists, and notifications.
    If an operation lacks named targeting, add framework support or replace the named element.
    Only framework internals may allocate numeric node handles for rendering and event dispatch.

19. **Commit and push each completed agent task immediately.** The coordinating agent reviews the changes and verification results when each agent finishes.
    Commit the verified changes for that task. Push the commit to the assigned branch immediately.
    Confirm the commit on the remote branch and notify the owner. Other agents can continue their work.
    If the push fails, report the failure and retry after correcting the cause.

---

## Migration Notice

Start with this file for project work. See `.agents/AGENTS.md` for shared rules.

---

## Quick Start

👉 **Go to [`AGENTS.md`](./.agents/AGENTS.md)** to get started.

---

_See .agents/AGENTS.md for shared agent configuration._
