# 65-valtron-stream-cleanup — Spec Start

Spec-level workflow entry point.

1. Read `requirements.md` for the overview, feature index, and recorded decisions.
2. Identify the feature to work on from the Feature Index.
3. Navigate to `features/<NN-name>/start.md` and follow its workflow.
4. Work features in dependency order: `00-cleanup`, then `01-migrate-recv-iterator`.

Read `.agents/AGENTS.md` and the `rust-valtron-usage` / `rust-valtron-iterator`
skills before implementing any feature.

This spec is intended to be completed **before** `64-valtron-state-task`.
