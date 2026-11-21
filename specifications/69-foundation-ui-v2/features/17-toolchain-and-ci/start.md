---
workspace_name: "ewe_platform"
spec_directory: "specifications/69-foundation-ui-v2"
feature_directory: "specifications/69-foundation-ui-v2/features/17-toolchain-and-ci"
this_file: "specifications/69-foundation-ui-v2/features/17-toolchain-and-ci/start.md"
created: 2026-11-14
---

# Start: feature 17-toolchain-and-ci

_Written in ASD-STE100 Simplified Technical English (STE)._

## Agent workflow

1. Read [`../../start.md`](../../start.md) and the files that it lists.
2. Read [`feature.md`](./feature.md) completely. It names the pins, the scripts and the CI
   steps. This feature has no crate; do not add one.
3. Read `.agents/skills/main-agent-orchestration/skill.md` for the CI rules, and
   `.agents/skills/code-verification/skill.md` for the check and test rules.
4. Read the current root `mise.toml`, `mise.lock` and `ci/platform-images/mise/mise.toml`
   before you change a pin.
5. Check the "Measured facts" of section 6.1. Run the commands again. If a version or an
   output differs, stop and report it.
6. Work on one task at a time, in the order of the "Tasks" section. Write the test of a
   script before the script. A shell test asserts an exit status and an output line.
7. Run the commands of the "Verify" section. A green `mise install` is not a pass; run the
   check, the shell tests and, on macOS, the proof.
8. Update [`../../progress.md`](../../progress.md) and [`../../learnings.md`](../../learnings.md).
   Record the versions that the proof printed.
9. Report to the main session: the files that you changed, the commands and their results,
   each "Done when" item with its evidence, and each bug or limit that you found (the main
   session opens the GitHub issues). Do not commit.

## Notes

- The Android SDK packages are large. Check `df -h` before `mise run toolchain:android`.
- Xcode is host-managed. Do not add it to `mise.toml`, and do not install it.
- A pin change also changes `mise.lock`. Change both in one commit.

---

_Created: 2026-11-14_
