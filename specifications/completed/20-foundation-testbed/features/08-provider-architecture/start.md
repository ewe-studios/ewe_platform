# Provider Architecture — Agent Workflow

## Steps

1. **Read `feature.md`** — understand the full architecture refactor
2. **Read parent `requirements.md` and `LEARNINGS.md`** — understand all past lessons
3. **Phase 1: Infrastructure** — Provider trait, move qemu/ under providers/, create common/
4. **Phase 2: CLI Extraction** — move CLI logic to foundation_testbed, create standalone bin
5. **Phase 3: UTM Provider** — AppleScript wrapper, .utm bundle management
6. **Phase 4: Integration** — root mise.toml reference, cfg(target_os) selection
7. **Update LEARNINGS.md** after each phase
8. **Report to Main Agent** when complete

## Critical Design Decisions to Validate Before Starting

- Provider trait should be a `trait` or an `enum` with match dispatch?
  - **Decision**: `trait` — allows adding future providers (libvirt, Docker, etc.)
    without modifying existing code (Open/Closed principle).
- Should clap be an optional dependency or always present?
  - **Decision**: Optional behind `cli` feature flag — the library can be used
    programmatically without pulling in clap.
- Should QEMU and UTM providers coexist on the same platform?
  - **Decision**: No — `#[cfg(target_os = "linux")]` gates QEMU,
    `#[cfg(target_os = "macos")]` gates UTM. No `all-providers` build needed
    in practice.

## Implementation Order

### Phase 1 (Infrastructure Refactor)
1. Update `Cargo.toml` with feature flags
2. Create `src/providers/mod.rs` — Provider trait + VmHandle + factory
3. Create `src/common/` — move shared modules
4. Move `src/qemu/*` → `src/providers/qemu/*`
5. Update all imports, verify `cargo check --features qemu`

### Phase 2 (CLI Extraction)
6. Create `src/cli/mod.rs` — build_command() + dispatch()
7. Create `src/cli/handlers.rs` — move from bin/platform/src/testbed/cli.rs
8. Create `src/bin/testbed.rs` — standalone binary
9. Simplify `bin/platform/src/testbed/` to 20 lines
10. Verify both `cargo run --bin testbed` and `cargo run -p ewe_platform -- testbed`

### Phase 3 (UTM Provider)
11. Create `src/providers/utm/applescript.rs`
12. Create `src/providers/utm/bundle.rs`
13. Create `src/providers/utm/mod.rs`
14. Add macOS-relevant profiles to config.rs
15. Verify `cargo check --features utm` on macOS

### Phase 4 (Integration)
16. Create `backends/foundation_testbed/mise.toml`
17. Update root `mise.toml` to reference it
18. Update README.md with cross-platform docs
