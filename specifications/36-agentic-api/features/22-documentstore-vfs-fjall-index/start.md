---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/22-documentstore-vfs-fjall-index"
this_file: "specifications/36-agentic-api/features/22-documentstore-vfs-fjall-index/start.md"
created: 2026-06-14
---

# Start: DocumentStore VFS + fjall offset index

## Agent Workflow

1. Read `feature.md` + Decision 13 (VFS) + Decision 03 (offset-index TODO).
2. **Stack:** Rust + VFS + fjall. Read `.agents/skills/rust-clean-code/skill.md`. Read the existing
   VFS (`foundation_nativeapis/src/native/vfs/native_fs.rs`, offset I/O) and the `vfs-fjall` feature
   + `fjall_vfs` test.
3. Confirm F06 landed (the `DocumentStore` trait with `scan_from` + doc_id = scru128 ordering).
4. Read `../../LEARNINGS.md`. Follow `feedback_target_gate_native_tooling` (native-only, target-gated).
   Resolve OD-22-1..5 before coding.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** append+index → scan_from seek → crash-rebuild → delete/compaction.
7. Report; verify; update `../../LEARNINGS.md`; move to Feature 23.

---

**Workflow:** feature.md → Decisions 13/03 → VFS+fjall grounding → Resolve OD-22 → Compact → ONE ITEM → Report → Verify → 06

---

_Created: 2026-06-14_
