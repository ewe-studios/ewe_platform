# 64-valtron-stream-task — Spec Start

Spec-level workflow entry point.

1. Read `requirements.md` for the overview, feature index, and recorded decisions.
2. Identify the feature to work on from the Feature Index.
3. Navigate to `features/<NN-name>/start.md` and follow its workflow.
4. Work features in order:
   `00-cleanup` → `01-migrate-recv-iterator` →
   `02-wakeable-stream` → `03-bridge-park` →
   `04-ctx` → `05-thread-ctx` → `06-state-task`.

The three former specs are folded in sequence:

- **Stream cleanup** (was 65): `00-cleanup`, `01-migrate-recv-iterator`.
- **Stream wake** (was 66): `02-wakeable-stream`, `03-bridge-park`.
- **State task** (was 64): `04-ctx`, `05-thread-ctx`, `06-state-task`.

Read `.agents/AGENTS.md` and the `rust-valtron-usage` / `rust-valtron-iterator`
skills before implementing any feature.

Two cross-cutting constraints to keep in mind:

- The `StreamIterator` blanket impl is **kept** — `WakeableStream` is opt-in and
  must have no blanket impl of its own (E0119 with the waking terminal's
  `Successful` impl).
- `Ctx` is passed **by value** (an `Arc` clone) through the task traits; never
  `&Ctx`/`&mut Ctx`.
