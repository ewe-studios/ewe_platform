# macOS VM Support — Agent Workflow

## Steps

1. **Read `feature.md`** — understand the full feature scope and requirements
2. **Read parent `requirements.md`** and `LEARNINGS.md` in spec root
3. **Implement Phase 1** — add `GuestOs::MacOS`, profile, QEMU args
4. **Implement Phase 2** — native IPSW download + BaseSystem extraction for image creation
5. **Implement Phase 3** — macOS bootstrap and SSH
6. **Implement Phase 4** — build pipeline for Apple targets
7. **Update LEARNINGS.md** in spec root after each phase
8. **Report to Main Agent** when complete

## Prerequisites

- Feature 01 (QEMU Backend) must be complete — we extend `build_qemu_args`
- Feature 02 (VM Communication) must be complete — we use SSH for macOS
- `qemu-img` and `unzip` on host (for image creation)

## Implementation Order

1. Add `GuestOs::MacOS` to `config.rs`, update all `match` arms
2. Create `macos-build` profile in `config.rs`
3. Add macOS branch to `build_qemu_args` in `qemu/mod.rs`
4. Create `src/import/macos.rs` for native IPSW download + BaseSystem extraction
5. Bundle OpenCore EFI image at `resources/opencore/`
6. Create `src/bootstrap/macos.rs` for macOS-specific bootstrap
7. Wire into CLI (`testbed import/start/build macos-build`)

## Notes

- macOS uses SATA for boot disk, not virtio — unlike Windows/Linux
- OpenCore EFI must be present on a separate EFI partition or qcow2
- The OSK string is a well-known constant (not a secret)
- No WinRM equivalent — SSH is the only remote access method
