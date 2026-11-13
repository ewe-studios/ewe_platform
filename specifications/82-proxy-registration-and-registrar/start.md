---
workspace_name: "ewe_platform"
spec_directory: "specifications/82-proxy-registration-and-registrar"
this_file: "specifications/82-proxy-registration-and-registrar/start.md"
created: 2026-11-14
---

# Start: Proxy Registration and Registrar

## Agent Workflow

1. Read `requirements.md` (mission, order, the rotation rule, success criteria).
2. Read `../78-manifest-contract/requirements.md` (the `proxy` and `probes` blocks).
3. Read `../81-foundation-auth/requirements.md` (enrollment, CA, mTLS).
4. Read `LEARNINGS.md`.
5. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
6. Read the existing proxy code before you design an extension: `config.rs`, `router.rs`,
   `runtime.rs`, `health.rs`, `control.rs`, `persistence.rs`, `tls.rs`.
7. Pick a feature from the Feature Index. Feature 00 runs before feature 01. The sidecar's
   end-to-end task also needs spec 79.
8. Navigate to `features/[feature]/start.md`.
9. Update `progress.md` and `LEARNINGS.md` after each task.

**Order:** spec 78 and spec 81 first. Spec 63 is complete; do not re-open it.

**Workflow:** Requirements → 78 + 81 → Existing proxy → Select feature → Feature start.md → Implement

---

_Created: 2026-11-14_
