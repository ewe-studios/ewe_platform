# Archived Decisions

These files have been merged into consolidated decision documents.
They are preserved here for reference but are no longer canonical.

## Current canonical docs (10 files, sequentially numbered)

| # | File | Source |
|---|---|---|
| 01 | `01-platform-and-crates.md` | 01 + 17 |
| 02 | `02-route-policy-model.md` | (rewritten in-place) |
| 03 | `03-session-backbone-transport.md` | 03 + 04 + 15 + 06 + 07 + 08 |
| 04 | `04-deployment-surfaces.md` | 09 + 10 + 11 + 12 + 13 + 16 |
| 05 | `05-offline-and-sync.md` | 05 + 22 |
| 06 | `06-webview-profiles.md` | 14 (renumbered) |
| 07 | `07-native-capability-contract.md` | 18 (renumbered) |
| 08 | `08-security-model.md` | 19 (renumbered) |
| 09 | `09-testing-strategy.md` | 20 (renumbered) |
| 10 | `10-multi-webview-stack.md` | 21 (renumbered) |

## Round 1 merges

### Merged into 02-route-policy-model.md (rewritten in-place)
(None directly — 02 was rewritten, absorbing execution contract from 03/04/15)

### Merged into 03-session-backbone-transport.md
- `03-session-backbone.md` — session coordination model
- `04-transport-lanes.md` — protocol vs transport, all 7 lanes
- `15-custom-protocol-model.md` — ewe:// URI scheme, transport adapter

### Merged into 04-deployment-surfaces.md
- `09-api-surface-bundled-wasm-webview.md` — bundled WASM in WebView
- `10-api-surface-native-static-lib.md` — native static library
- `11-api-surface-shell-wasm-runtime.md` — shell + WASM runtime
- `12-api-surface-remote-server-driven.md` — remote server-driven
- `13-api-surface-cached-offline-replay.md` — cached offline replay

## Round 2 merges

### Merged into 01-platform-and-crates.md
- `01-platform-architecture.md` — platform architecture (stub)
- `17-crate-boundaries.md` — crate dependency graph, type residency

### Merged into 05-offline-and-sync.md
- `05-offline-model.md` — two-tier offline model
- `22-background-sync.md` — background sync tiers, mutation queue

### Folded into 03-session-backbone-transport.md
- `06-state-and-communication-model.md` — state ownership, communication model
- `07-native-integration-model.md` — native UI tiers, web↔native sync
- `08-arrow-and-data-model.md` — Arrow vs Columnar distinction

### Folded into 04-deployment-surfaces.md
- `16-entrypoint-model.md` — entrypoint attributes, build pipeline

### Renumbered (no content change)
- `14-webview-profiles.md` → `06-webview-profiles.md`
- `18-native-capability-contract.md` → `07-native-capability-contract.md`
- `19-remote-ui-security-red-team.md` → `08-security-model.md`
- `20-testing-strategy.md` → `09-testing-strategy.md`
- `21-multi-webview-stack.md` → `10-multi-webview-stack.md`
