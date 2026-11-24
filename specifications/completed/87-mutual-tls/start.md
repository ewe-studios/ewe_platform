---
workspace_name: "ewe_platform"
spec_directory: "specifications/completed/87-mutual-tls"
this_file: "specifications/completed/87-mutual-tls/start.md"
created: 2026-11-15
---

# Start: Mutual TLS

## Agent Workflow

1. Read `requirements.md` in full. Section 4 is the design, section 8 is the task list.
2. Read `mtls.md` in full. It is the authority for the types and the algorithms. The Revision 2
   table at its head records every substitution against the old spec 81 text, including the
   time source (`ctx.clock()`) and the task numbers.
3. Read `../86-auth-primitives/requirements.md` section 3.5 and 3.6. The providers implement
   `ClientAuth` and return `AuthInfo` with an `AuthMethod`; this specification does not define
   those types.
4. Read `../84-context-injection/requirements.md` section 3. The verifier and
   `TrustedClients` take the time from `Ctx`, and never from the system clock.
5. Read `../80-platform-prerequisites/features/00-transport-prerequisites/feature.md`. It
   makes `foundation_http` fill `ConnectionContext::tls` after every TLS handshake. `MtlsAuth`
   reads the peer chain from there.
6. Read the code that moves before T1 and T6: `backends/foundation_agent_rpc/src/auth.rs`
   (the CA and `MtlsConfig`), `src/trust.rs`, `src/secret.rs` (the CA part),
   `backends/foundation_netio/src/shared/context.rs` (`TlsInfo`).
7. Read `.agents/skills/rust-clean-code/skill.md` before T1.
8. Read `.agents/AGENTS.md` and the agent file for `foundation_auth`.
9. Work on one task at a time, in the order of the next section.
10. Report to the Main Agent. Do not commit.
11. After the commit, update `progress.md` and `LEARNINGS.md`.

**Order:** T1 → T2 → T3 → T4 → T5 → T6.

**Owned crates:** `foundation_auth` for T1 to T5. T6 also touches `foundation_agent_rpc`, and
only to remove code that moved.

Do not change `foundation_proxy`, `foundation_connectrpc` or `foundation_platform_sandbox`:
their specifications record the dependency and do their own change.

**Version:** `foundation_auth` moves one minor version. `foundation_agent_rpc` moves one minor
version in T6. Every crate that takes the new requirement moves in the same change (root rule:
version bump on new public API).

**Dependencies:** `rustls`, `rcgen` and `x509-parser` move with the code. Declare them once in
`[workspace.dependencies]` (root rule 17).

**Workflow:** Requirements → Select task → Implement → Report → Verify → Commit → Update
progress

---

_Created: 2026-11-15_
