---
name: feedback-macros-location
description: All proc macros go in foundation_macros — no companion proc-macro crates
metadata:
  type: feedback
---

All proc macros go in `backends/foundation_macros/`. No companion proc-macro crates (e.g. no `foundation_arrow_macros`). One crate for all derive/attribute macros.

**Why:** User wants a single, clear location for all macros. Keeps the workspace lean.

**How to apply:** When adding any new derive or attribute macro (ArrowSerialize, etc.), add it as a new module in `foundation_macros/src/` and wire it into `lib.rs`. Never create a separate `*_macros` crate.
