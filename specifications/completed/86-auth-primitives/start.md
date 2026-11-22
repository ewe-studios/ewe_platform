---
workspace_name: "ewe_platform"
spec_directory: "specifications/completed/86-auth-primitives"
this_file: "specifications/completed/86-auth-primitives/start.md"
created: 2026-11-15
---

# Start: Auth Primitives

## Agent Workflow

1. Read `requirements.md` in full. Section 3 is the design and section 7 is the task list.
2. Read `../../81-foundation-auth/requirements.md` sections 4 and 7. Spec 81 keeps the signed
   secrets, the key files and the provider set; it does not own the blocks of this spec.
3. Read `../87-mutual-tls/requirements.md` section 4. The mTLS provider implements the
   `ClientAuth` trait of this specification.
4. Read `../../77-foundation-platform-sandbox/features/00-contract/feature.md` section 6.10 and
   `features/02-control-plane-core/feature.md` section 6.10. They are the two places that
   delete their own secret type and bearer parser.
5. Read `backends/foundation_auth/src/shared/types.rs`, `shared/middleware.rs`,
   `shared/two_factor.rs` and `foundation_connectrpc/src/shared/auth.rs` before T1 and T4.
6. Read `.agents/skills/rust-clean-code/skill.md` before T1.
7. Read `.agents/AGENTS.md` and the agent file for `foundation_auth`.
8. Work on one task at a time, in the order of the next section.
9. Report to the Main Agent. Do not commit.
10. After the commit, update `progress.md` and `LEARNINGS.md`, and close the two issues named
    in `progress.md`.

**Order:** T2 → T1 → T3 → T4 → T5 → T6.

**Owned crates:** `foundation_auth` and the `auth` module of `foundation_connectrpc`. Do not
change `foundation_platform_sandbox`, `foundation_agent_rpc` or `foundation_proxy`: their specs
record the dependency and do their own change.

**Version:** `foundation_auth` moves one minor version. Every crate that takes the new
requirement moves in the same change (root rule: version bump on new public API).

**Workflow:** Requirements → Select task → Implement → Report → Verify → Commit → Update
progress

---

_Created: 2026-11-15_
