# Bug and limitation backlog (retired)

This file was the single list of known bugs and limitations until 2026-11-06.

It is retired. The tracker is now the GitHub issue tracker. Every entry was audited on
2026-11-06 and moved to an issue, so this file holds no open entry.

What the audit did:

- Deleted 11 entries that are already fixed.
- Added 22 entries to the existing per-crate issues.
- Created 22 new issues, one per remaining defect or decision, each with a
  `severity:` label. Clippy debt across crates is one tracking issue.
- Merged duplicates: the wireguard and reactor clippy entries each became one item.

## Where the old sections live now

| Old section | Issue |
| --- | --- |
| 1 foundation_connectrpc | #99 |
| 2 foundation_netio | #100 |
| 3 foundation_http | #101 |
| 4 foundation_core / valtron | #102 |
| 5 foundation_nostd / wasm / iogate | #103 |
| 5 foundation_nativeapis (signal handler) | #189 |
| 5b foundation_reactor | #200 |
| 8 foundation_openapi / codegentools | #106 |
| 8 mobile declaration | #196 |
| 9 foundation_proxy | #110 |
| 9 pool-driven panic | #102 |
| 10 foundation_ai | #190, #191, #200 |
| 11 foundation_db | #192, #193 |
| 12 foundation_auth | closed: all entries fixed |
| 16 workspace / process | #107, #194, #195, #196, #200 |
| 22 foundation_keychain | #197 |
| 26 foundation_platform | #200 |
| 27 foundation_packager | #200 |
| 28 infrastructure_llama_cpp | #107 |
| 29 foundation_jsonschema | #198 |
| 30 proc-macro crates | #199, #200 |
| 31 lane B clippy debt | #200 |
| 32 foundation_toolings | #201, #202, #203 |
| 35 foundation_nativeapis | #204, #205, #206, #207, #208, #209, #210 |

The umbrella issue is #98. Each per-crate issue is listed there.

## How to use it now

- New finding: open a GitHub issue. Give it a `severity:` label and, for a defect, the `bug`
  label. Name the file, the problem and the likely fix.
- Fixed finding: close the issue in the change that fixes it.
- Do not add an entry to this file.

The root rules that govern this are rule 9 in `CLAUDE.md` and rule 1 in
`specifications/CLAUDE.md`.
