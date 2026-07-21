---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F20-test-extraction-and-pub"
this_file: "specifications/52-tauri-foundation-platform/features/F20-test-extraction-and-pub/feature.md"

status: completed
priority: high
created: 2026-07-18

depends_on:
  - "F00-crate-skeleton"
  - "F01-session-backbone"

tasks:
  completed: 10
  uncompleted: 0
  total: 10
  completion_percentage: 100%
---
# F20 — Test extraction to `tests/` + pub internals

## Overview

Inline `#[cfg(test)] mod tests { ... }` blocks scattered across
`foundation_platform/src/*.rs` have been extracted to standalone
test files in `tests/`. Necessary internal types were made `pub`
or `pub(crate)` for test access.

House rule: tests go in `tests/`. Private-field/function/proc-macro/wasm
tests stay inline. Public API tests + integration tests live in `tests/`.

## Results

### Files created (2026-07-18)

| Test file | Source | Test count |
|---|---|---|
| `tests/cache_suite.rs` | `cache.rs` | 8 |
| `tests/profiles_suite.rs` | `profiles.rs` | 8 |
| `tests/stack_suite.rs` | `stack.rs` | 18 |
| `tests/pattern_suite.rs` | `pattern.rs` | 12 |
| `tests/session_suite.rs` | `session.rs` | 17 |
| `tests/mutation_suite.rs` | `mutation.rs` | 10 |
| `tests/backend_suite.rs` | `backend.rs` | 6 |
| `tests/capability_suite.rs` | `capability.rs` | 5 |
| `tests/ewe_suite.rs` | `ewe.rs` | 18 |
| `tests/codegen_suite.rs` | `codegen.rs` | 3 |

### Kept inline

| Module | Reason |
|---|---|
| `ewe::mode_page_tests` | Tests private rendering functions (`html_escape`, `build_mode_page`, `mode_badge_for`) |

### Types made pub for test access

| File | Type | Visibility |
|---|---|---|
| `backend.rs` | `TestTransport` | `pub` |
| `capability.rs` | `TestCap` | `pub` |
| `capability.rs` | `test_registry()` | `pub` |
| `ewe.rs` | `EweUrl` struct + fields | `pub` |
| `ewe.rs` | `EweUrl::parse()` | `pub` |
| `ewe.rs` | `protocol_from_hint()` | `pub` |
| `ewe.rs` | `protocol_from_query()` | `pub` |
| `ewe.rs` | `method_from_request()` | `pub` |
| `codegen.rs` | `Annotation`, `AnnotationKind` | `pub` |
| `codegen.rs` | `scan_for_annotations()` | `pub` |
| `mutation.rs` | `MutationQueue::pending_items()` | `pub` |
| `lib.rs` | `mod backend`, `mod capability` | `pub mod` |
| `lib.rs` | `SessionEvent` | re-exported |

## Verification

```bash
cargo test --package foundation_platform  # 165 passed, 0 failed
```
