---
workspace_name: "ewe_platform"
spec_directory: "specifications/85-wasm-fetch-transport"
this_file: "specifications/85-wasm-fetch-transport/start.md"
created: 2026-11-15
---

# Start: Wasm HTTP Streaming

## Agent Workflow

1. Read `requirements.md` in full. Sections 4 and 5 are the design and the rules.
2. Read `fetch.md` for the shared reader, the exchange task, the response builder, the sink,
   the DO stub client and the error table.
3. Read `../84-context-injection/requirements.md` sections 5.1 and 5.4. The clock and the
   timer come from `Ctx`.
4. Read `../83-agent-rpc-serving-system/features/05-worker-host-and-web-client/worker-and-web.md`
   sections 5.3 to 5.5 and 6.2. They show how a Worker serves through `foundation_http` and
   calls its DOs. `fetch.md` is the authority when the two disagree.
5. Read `../77-foundation-platform-sandbox/features/07-cloudflare-control-plane/feature.md`
   section 6.5 and `features/08-cloudflare-worker/feature.md` section 6.2. They are the two
   sandbox consumers.
6. Read `.agents/skills/rust-valtron-usage/skill.md` before T4. Its test section is older than
   the code: a wasm test is `#[wasm_bindgen_test]` or `#[valtron_bindgen]`, and a native test is
   `#[valtron_test]`, which holds the pool guard (`foundation_macros/src/lib.rs:675`). A task
   written there is a `#[valtron::task]` async transition, not a hand-written `impl
   TaskIterator`. Read `.agents/skills/rust-clean-code/skill.md` before T1.
7. Read `.agents/AGENTS.md` and the agent file for `foundation_connectrpc`.
8. Work on one task at a time, in the order of the next section.
9. Report to the Main Agent. Do not commit.
10. After the commit, update `progress.md` and `LEARNINGS.md`.

**Order:** T6 → T1 → T2 → T3 → T4 → T5. T6 first: it gives T1, T2 and T4 a wasm test
executor, which does not exist for these crates today. T4 needs T3; T5 needs T2.

**Owned crates:** `foundation_netio` (the wasm client, the shared reader, the response
builders, the DO stub client), `foundation_http` (the two wasm bridges), `foundation_connectrpc`
(the sink and the capability constants), and the test wiring: the existing
`foundation_netio/tests/wasm/`, a new `foundation_connectrpc/integration/wasm/` crate
(outside the workspace, as `foundation_db/integration/wasm_cf` shows), and the tasks in
`mise.wasm.toml` and `scripts/ci/ci.py`.

**Test executor:** the `wasm-testbed test bindgen-deno <crate>` mode runs `#[valtron_bindgen]`
under the pinned Deno. It passes `--no-default-features` on wasm32 (`build.rs:273-276`), so
`foundation_netio` needs no new crate. `foundation_connectrpc` does: `foundation_core` with
`multi` is a hard error on wasm32 (`foundation_core/src/lib.rs:20`). The suite ran on
2026-11-15: five tests found, four pass, one hangs (#565). Do not touch `foundation_platform_sandbox` or `foundation_agent_rpc`: their
specs record the dependency and do their own change.

**Root rules that decide the hard cases:** rule 4 (stream, do not buffer), rule 6 (async all
the way), rule 7 (tests in `tests/`), rule 8 (limits are configuration), rule 15 (Linux, macOS
and Windows).

**Workflow:** Requirements → Select task → Implement → Report → Verify → Commit → Update
progress

---

_Created: 2026-11-15_
