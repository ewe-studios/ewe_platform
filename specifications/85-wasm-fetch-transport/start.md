---
workspace_name: "ewe_platform"
spec_directory: "specifications/85-wasm-fetch-transport"
this_file: "specifications/85-wasm-fetch-transport/start.md"
created: 2026-11-15
---

# Start: Wasm Fetch Transport

## Agent Workflow

1. Read `requirements.md` in full. Sections 4 and 5 are the design and the rules.
2. Read `fetch.md` for the exchange phases, the dispatch, the DO stub and the error table.
3. Read `../84-context-injection/requirements.md` sections 5.1 and 5.4. The clock and the timer
   come from `Ctx`, and `FetchServe::new` fails with `EnvError::Missing` without them.
4. Read `../83-agent-rpc-serving-system/features/05-worker-host-and-web-client/worker-and-web.md`
   sections 5.4 and 6.2. They are the original text of the moved design; `fetch.md` is the
   authority when the two disagree.
5. Read `../77-foundation-platform-sandbox/features/07-cloudflare-control-plane/feature.md`
   section 6.5 and `features/08-cloudflare-worker/feature.md` section 6.2. They are the two
   sandbox consumers.
6. Read `.agents/skills/rust-valtron-usage/skill.md` before T2 (the pump is a valtron task) and
   `.agents/skills/rust-clean-code/skill.md` before T1.
7. Read `.agents/AGENTS.md` and the agent file for `foundation_connectrpc`.
8. Work on one task at a time, in the order of the next section.
9. Report to the Main Agent. Do not commit.
10. After the commit, update `progress.md` and `LEARNINGS.md`.

**Order:** T4 → T1 → T3 → T2 → T5.

**Owned crates:** `foundation_connectrpc` (the `wasm/` module, the transport types, the
deadline read), `foundation_netio` (two response helpers), `foundation_compact` (a stream impl
on `SendWrapper`). Do not touch `foundation_platform_sandbox` or `foundation_agent_rpc`: their
specs record the dependency and do their own change.

**Root rules that decide the hard cases:** rule 4 (stream, do not buffer), rule 6 (async all
the way), rule 7 (tests in `tests/`), rule 8 (limits are configuration), rule 15 (Linux, macOS
and Windows).

**Workflow:** Requirements → Select task → Implement → Report → Verify → Commit → Update
progress

---

_Created: 2026-11-15_
