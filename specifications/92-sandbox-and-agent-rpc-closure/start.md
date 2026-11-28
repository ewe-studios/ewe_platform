---
workspace_name: "ewe_platform"
spec_directory: "specifications/92-sandbox-and-agent-rpc-closure"
this_file: "specifications/92-sandbox-and-agent-rpc-closure/start.md"
created: 2026-11-27
---

# Start: closing specs 77 and 83

Read `requirements.md` first, then `progress.md` and `learnings.md`.

The order is fixed: feature 00 (live proof) first, because a red `sandbox-cloudflare` job
makes every claim about the Cloudflare path uncertain. Feature 01 (DST fidelity) second,
because it changes what the simulation tests prove. Feature 02 (follow-ups) last, in
severity order.

Each task names an issue. Close the issue in the change that fixes it, and say in the closing
comment which test proves the fix.

Never put a secret in the repository, a log, a plan or a test fixture. Read it from the
environment. When a credential is absent, the test prints the reason and returns; a release
workflow fails instead.

After each feature, update `progress.md` and `learnings.md`.

---

_Created: 2026-11-27_
