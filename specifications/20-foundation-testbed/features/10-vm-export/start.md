# VM Export & Distribution — Agent Workflow

## Steps

1. **Read `feature.md`** — understand the full feature scope and requirements
2. **Read parent `requirements.md`** and `LEARNINGS.md` in spec root
3. **Implement Phase 1** — local export: copy disk, generate manifest, SHA256
4. **Implement Phase 2** — upload to S3/R2/GitHub Releases
5. **Implement Phase 3** — extend import to read manifests
6. **Update LEARNINGS.md** in spec root after each phase
7. **Report to Main Agent** when complete

## Prerequisites

- Feature 05 (CLI & State Management) must be complete — we extend CLI and read state
- Feature 08 (Provider Architecture) must be complete — we need disk path resolution
- `qemu-img` on host (for disk conversion/compression)

## Implementation Order

1. Create `src/export/mod.rs` — orchestrator with stop/copy/manifest flow
2. Create `src/export/manifest.rs` — manifest struct, JSON serialization, tool detection via SSH
3. Create `src/export/shrink.rs` — fstrim, zero-fill, qemu-img compress
4. Create `src/export/upload.rs` — S3 SigV4 signing + PUT via simple_http
5. Create `src/export/destination.rs` — parse r2://, s3://, gh-release:// URIs
6. Wire into CLI (`testbed export <name> --out/--upload`)
7. Extend `import::download` to fetch and display manifest.json

## Notes

- Export should be idempotent — running twice produces the same output
- SHA256 is computed on the qcow2 file (not the raw data)
- S3 SigV4 signing is stateless — no AWS SDK needed, just HMAC-SHA256
- GitHub Releases upload uses `gh` CLI (already authenticated via `gh auth`)
- The `--shrink` flag requires the VM to be running (for fstrim) — if VM is stopped, skip fstrim step
