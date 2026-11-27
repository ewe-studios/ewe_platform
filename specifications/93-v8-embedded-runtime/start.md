---
workspace_name: "ewe_platform"
spec_directory: "specifications/93-v8-embedded-runtime"
this_file: "specifications/93-v8-embedded-runtime/start.md"
created: 2026-11-27
---

# Start: the embedded V8 runtime

Read `requirements.md` first, then `progress.md` and `learnings.md`.

**Nothing is implemented.** This specification is for the owner to review and decide. Do not
write code until the owner settles decisions D1 to D8 in `requirements.md`.

## Order

1. **Feature 00, the spike.** One isolate, one ESM from disk, one op and one timer on a
   valtron loop. If the spike fails, stop and report; the move is not ready.
2. **Feature 01, the core.** Isolate, loader, ops, event loop, public API and CI.
3. **Feature 02, the Web APIs.** The globals that the host ABI reads. The 105 non-network
   cases run here, because the harness JavaScript needs them.
4. **Feature 03, fetch and WebSocket.** The network path on `foundation_netio`.
5. **Feature 04, the cutover.** V8 becomes the default and Deno is deleted.

Do not start a feature until the gate of the previous feature is green.

## Rules

- Keep `foundation-wasm.js` unchanged. It is the code under test.
- Keep the public names `run_module`, `run_staged_harness` and `HarnessReport`, so the
  callers change only the import path.
- Do not delete a network test. Keep it on the Deno feature until feature 03 passes it on V8.
- Do not copy Deno's JavaScript. It uses `Deno.core` internals and primordials (issue #139).
- Every size, count and timeout is configuration with a documented default (root rule 8).
- Every claim in a feature file cites `path:line` or the issue.
- Update `progress.md` and `learnings.md` after each feature.
- Every bug found becomes a GitHub issue with a severity (root rule 9).

## Before implementation

The owner answers D1 to D8 in `requirements.md`. In particular:

- D1: start this move now, or bump the Deno set to 0.413 as one group and keep #139 as the
  contingency.
- D2: the feature name and the cutover timing.
- D8: the CI cost of two V8 feature sets.

---

_Created: 2026-11-27_
