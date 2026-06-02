---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/http11-compatibility-review"
this_file: "specifications/02-build-http-client/features/http11-compatibility-review/start.md"
created: 2026-03-11
---

# Start: HTTP/1.1 Compatibility Review

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks)
2. Read `../LEARNINGS.md` (past discoveries and mistakes)
3. Read `.agents/AGENTS.md` to identify your agent type
4. Read your agent file in `.agents/agents/[agent-name].md`
5. Read skills specified in your agent documentation
6. Review RFC 7230-7235 specifications (linked in feature.md)
7. **MANDATORY**: Generate `compacted.md` with all info using `.agents/skills/context-compaction/skill.md`
8. Clear context, reload from `compacted.md` only, start work
9. **Work on ONE item at a time** - one RFC section, one file, one finding at a time
10. Document findings in REPORT.md, SECURITY.md, GAPS.md
11. Report to Main Agent when done (DO NOT commit)
12. Wait for verification to pass
13. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to next task

---

**Workflow:** Feature.md → Learnings → AGENTS.md → Agent Doc → RFC Specs → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Audit → Document → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-03-11_
