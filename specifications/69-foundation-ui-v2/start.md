---
workspace_name: "ewe_platform"
spec_directory: "specifications/69-foundation-ui-v2"
this_file: "specifications/69-foundation-ui-v2/start.md"
created: 2026-11-13
---

# Start: Spec 69, foundation_ui v2

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** user interface (UI); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

You implement **one feature** of this spec. The main session gave you its number.

## Read first, in this order

1. `CLAUDE.md`, `specifications/CLAUDE.md` and `.agents/AGENTS.md`.
2. [`requirements.md`](./requirements.md): sections 0 (glossary), 2 (decisions), 4 (context
   map), 5 (architecture and limits), 6 (rules) and 8 (verification). They apply to every
   feature.
3. [`research/README.md`](./research/README.md) section 3: the owner decisions D1 to D40.
   Read an idea file only when your feature names it.
4. Your feature: `features/NN-<slug>/start.md`, then its `feature.md`. Read all of it before
   you change code.
5. [`progress.md`](./progress.md) and [`learnings.md`](./learnings.md).

## Rules

- Do the features in the order of the Feature Index. Do not start a feature whose
  dependencies are not complete.
- Do not start feature 13 before the phase gate of `requirements.md` section 7 is open.
- You do not commit. You report to the main session with the diff summary, the commands
  that you ran and their output.
- If the feature file does not say what to do in a case, stop and report the gap. Do not
  invent an API.

---

_Created: 2026-11-13_
