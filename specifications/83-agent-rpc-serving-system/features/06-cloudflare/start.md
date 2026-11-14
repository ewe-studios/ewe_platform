---
workspace_name: "ewe_platform"
spec_directory: "specifications/83-agent-rpc-serving-system"
feature_directory: "specifications/83-agent-rpc-serving-system/features/06-cloudflare"
this_file: "specifications/83-agent-rpc-serving-system/features/06-cloudflare/start.md"
created: 2026-10-19
last_updated: 2026-11-14
---

# Start: Cloudflare Host Feature

## Read first

1. `feature.md`: the requirements, the architecture, the "Dependency on spec 77" section,
   the tasks and the "Done when" criteria.
2. Spec 77, which owns the substrate this feature runs on:
   - `../../../77-foundation-platform-sandbox/requirements.md` section 6.6 (container
     control).
   - `../../../77-foundation-platform-sandbox/features/00-contract/feature.md` section 6.13
     (`ContainerControl`, `ContainerHandle`, `ContainerSpec`).
   - `../../../77-foundation-platform-sandbox/features/07-cloudflare-control-plane/feature.md`
     section 6.4.1 (the Cloudflare container control).
   - `../../../77-foundation-platform-sandbox/features/08-cloudflare-worker/feature.md`
     section 6.8 (`DeploymentPlan` and the two deployers).
3. The features this one builds on: `../00-foundation/feature.md` (the wasm readiness),
   `../01-agent-rpc/feature.md` (the model server that is deployed) and
   `../05-worker-host-and-web-client/feature.md` (the Worker host and the `cf-agent-rpc`
   example).
4. `../../LEARNINGS.md`: the "Cloudflare moved to spec 77" note first, then the rest.
5. `.agents/AGENTS.md` and the skills it names, then your agent file in `.agents/agents/`.

The installer (spec 79) is **not** a dependency. Cloudflare deployment is not its concern.

## Agent Workflow

1. Generate `compacted.md` with `.agents/skills/context-compaction/skill.md`.
2. Work on one task at a time. Write the test first.
3. Use the spec 77 types. Do not write a deployer, a container API call or a
   `wrangler.toml` in this feature.
4. Stream every body. Keep every limit in the configuration.
5. Run the tests and verify commands that `feature.md` names.
6. Report to the Main Agent: the changed files, the commands you ran and their results.
7. After the commit, update `../../progress.md` and `../../LEARNINGS.md`.

**Order:** Feature 05, then spec 77 features 07 and 08, then this feature. Start with T4,
which builds on feature 05 T21.

---

_Created: 2026-10-19_
