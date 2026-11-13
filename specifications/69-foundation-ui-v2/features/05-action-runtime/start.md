---
workspace_name: "ewe_platform"
spec_directory: "specifications/69-foundation-ui-v2"
feature_directory: "specifications/69-foundation-ui-v2/features/05-action-runtime"
this_file: "specifications/69-foundation-ui-v2/features/05-action-runtime/start.md"
created: 2026-11-13
---

# Start: feature 05-action-runtime

_Written in ASD-STE100 Simplified Technical English (STE)._

## Agent workflow

1. Read [`../../start.md`](../../start.md) and the files that it lists.
2. Read [`feature.md`](./feature.md) completely. Its frontmatter names the model, the
   crates that you own and the features that must be complete first.
3. Read the language skills for the code that you change:
   `.agents/skills/rust-clean-code/skill.md` and `.agents/skills/rust-valtron-usage/skill.md`
   for Rust; `.agents/skills/test-driven-development/skill.md` for every language. If a
   language of your feature (JavaScript, Swift, Kotlin, Python) has no skill in
   `.agents/skills/`, follow the style of the files that the feature names and say so in
   your report.
4. Read `.agents/agents/implementation.md`.
5. Check the "Measured facts" of the feature against the code. If a fact changed, stop and
   report it.
6. Work on one task at a time, in the order of the "Tasks" section. Write the test first.
   A test goes in the `tests/` directory of its crate (`CLAUDE.md` rule 7).
7. Run the commands of the "Verify" section. A green `cargo check` is not a pass; run the
   tests and keep the output.
8. Update [`../../progress.md`](../../progress.md) and [`../../learnings.md`](../../learnings.md).
9. Report to the main session: the files that you changed, the commands and their results,
   each "Done when" item with its evidence, and each bug or limit that you found (the main
   session opens the GitHub issues). Do not commit.

---

_Created: 2026-11-13_
