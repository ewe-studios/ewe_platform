# Rules for specification work

These rules apply to every spec in this directory. They add to the root `CLAUDE.md` and
`.agents/AGENTS.md`; they do not replace them. The full text is in the `.agents` skills
`main-agent-orchestration` (delegation, models, limits) and `code-verification` (checks, CI,
real services).

## 1. Research first, then hand off (owner rule 2026-11-04)

- The main session researches and designs each feature **before** it hands it off. The
  feature file names the files, the measured cause, the fix, the tests that prove it (one
  must fail on the old code) and the exact verify commands.
- The feature frontmatter's `model:` picks the agent: **haiku** for mechanical work (dead
  code, lint and warning debt, moving tests, small CI or config edits, bookkeeping);
  **sonnet** for diagnosis or design (concurrency, async, protocols, security, generators,
  anything measured).
- **One fresh agent per feature, on that model**, with a self-contained brief (feature file,
  the spec's rules section, branch, owned crates, report format). Do not resume one
  long-lived agent across features; resume only to finish the feature it is on.
- The main session reviews each report against the feature's "Done when" before the next
  feature starts, and turns every new finding into a backlog entry and a scheduled feature.

## 2. Focused local checks, full CI on GitHub (owner rule 2026-11-04)

- Locally: format, clippy and tests of the crates you changed, plus the tests of the
  workspace crates that depend on them and use the changed API
  (`cargo tree -i <crate> --workspace --depth 1`). Do not run `ci.py run` for the whole set.
- The full run is the pull request's GitHub CI. The main session reads it and sends failures
  to the owning agent. A run cancelled by a newer push is not a failure.

## 3. Test against the real service, not a stub (owner rule 2026-11-03)

When a local tool runs the real thing, tests use it. Cloudflare: `wrangler dev --local` runs
real D1, R2 and KV bindings and serves the REST API at `/cdn-cgi/local/explorer/api`. Use a
stub only when no local form exists, and say so in the test.

## 4. Shared machine limits

- One lane builds at a time when the disk cannot hold every lane's `target`. Check `df`
  before big builds; free space with `cargo clean -p <finished crate>` in your own worktree.
- Stop a process by the PID you saved, never with `pkill -f <pattern>`.

## 5. Scope decisions are recorded

When the owner drops a feature, mark it `status: "dropped"` with a dated note, remove it from
the lane order, and delete its backlog entries (do not keep them as paused) unless the owner
keeps a part of it, which then becomes its own scheduled feature.
