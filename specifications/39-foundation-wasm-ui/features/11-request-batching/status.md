# Feature 11 — Status: COMPLETE (2026-06-12)

## What shipped (foundation-wasm-ui.js)

- `probeBatching()` — `HEAD /primal/messages`, 200 enables (404/network error
  disable). `RequestQueue.probe()` delegates to it.
- `RequestQueue` upgraded to the §4 batch format: queued requests become
  `{id, url, method, headers, body}` with monotonic ids; one
  `POST /primal/messages` per microtask tick; a SINGLE queued request skips
  the wrapper and ships as itself (test 5); disabled queues pass through;
  empty flushes make no call.
- `parseBatchEntry` (§5): routes each `{id, status, headers, body}` response
  entry by its content type into the same result objects the protocol
  handlers emit — `Patcher.route` works on batch members unchanged.
- `WSBatchQueue` (§6): one frame per microtask; single message unwrapped,
  several as `{batch: [...]}`.
- `WorkerBatchQueue` (§7): one `postMessage({batch})` per tick, transferables
  forwarded (not copied).
- WASM-local batching was already the architecture: the InstructionReceiver
  flushes once per `stabilize()` (F04); SSE is server→client and exempt (§1).

## Verification

7 tests: probe 200/404/throw (1-3), three-request id'd batch with order
(4), single-request passthrough (5), disabled passthrough + silent empty
flush (6-7), per-content-type batch-entry routing incl. morph wrapper (8),
WS single/multi/per-tick framing (9-11), worker batch + transferable
forwarding (12-13). Test 14 (worker receives and processes) is the worker-
host runtime's side — exercised when a worker app adopts F10's wrappers.
Full wasm_ui JS suite 67/67.
