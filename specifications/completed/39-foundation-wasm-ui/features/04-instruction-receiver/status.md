# Feature 04 — Status: COMPLETE (2026-06-12)

## What shipped

The `InstructionReceiver` core (queue/flush/ack over `ProtocolMethods`, owned vs
GLOBAL arena, the encode-under-lock/ship-outside-lock split) had landed with the
F01/F17 plumbing. This feature completed the spec surface:

- **`SendResult`** now carries the spec's diagnostics: `memory_id` + `op_count`
  + `encoded_bytes` (payload size excluding envelope). All protocol impls and
  the shared `write_framed` helper thread them through; `BatchInstructionsV1`'s
  generic `write_message` reports 0 and the `Vec<DomOp>` leg overwrites it.
- **Receiver**: initial queue capacity 64; flush drains via
  `mem::replace(.., Vec::with_capacity(64))` so capacity survives flush cycles
  (G22 — a bare `mem::take` would zero it); `flush_count()` (non-empty flushes
  only) + `pending_count()` alias.
- **`MockProtocol`** (`protocol/mock.rs`): byte 255, records batches + ACKs via
  `Rc<RefCell<..>>` recorder handles tests clone before boxing.
- **`runtime.rs`**: `wasm_ui::Runtime` + `RuntimeBuilder`
  (`protocol(..)`, `memory(..)`, `global_arena()`; panics "protocol is
  required"/"memory is required" per spec), `SharedInstructionReceiver`
  (cloneable `Rc<RefCell<..>>` handle — the spec's "receiver is Clone"),
  `Runtime::attach(signals)` registering the flush-on-stabilize
  `NotificationManager` (spec section 6 loop), and **`DomSignalBinding`** —
  the decision-004 closure-captures-receiver bridge from the F02 spec.
- **New dependency**: `foundation_wasm_ui` → `foundation_signals` (the
  decision-012 diagram's edge).
- **`foundation_signals` fix surfaced by the loop e2e**: dirty buckets are now
  FIFO (`VecDeque::pop_front`) — same-height effects run in creation/marking
  order, keeping order-dependent DomOp streams deterministic (two bindings on
  one signal previously flushed in LIFO order).

## Verification

20 tests in `tests/receiver_tests.rs` mapping the spec's 29-test plan
(consolidated where rows test one behavior): queue-order batches, empty/double
flush no-ops, per-cycle batching + flush_count, 5000-op batches, no-dedup,
SendResult fields for Arrow, byte-0 single-slot `[texts_off][texts_len]`
layout, JSON payload parses via serde_json, ack-frees + stale-id/double-ack
safety, protocol bytes (1/0/2/255), exact slot bytes vs encoder output,
generation-protected recycling, 10-cycle leak check (live slots == 0), builder
happy/panic paths, builder-routed ops, mock batch+ACK recording, and the full
signals loop: two `DomSignalBinding`s → `set()` → `stabilize()` → ONE
auto-flushed batch in effect order; quiet stabilize ships nothing. Plus the 11
pre-existing wasm_ui tests and all 21 foundation_signals tests still green.
Zero clippy warnings (`--all-targets`, uat) on both crates.

## Spec deviations (justified)

| Spec says | Shipped | Why |
|-----------|---------|-----|
| `HandleResult` enum `Ok{op_count}/DecodeError/StaleMemory` | `Result<Vec<DomOp>, DecodeError>` | The decoded ops are strictly more useful than a count; stale-memory is the arena's generation check (`memory.get` errs), not a decode outcome. |
| `flush()` returns `()` | `Option<SendResult>` | Host-side error paths and tests need the `memory_id` to ACK; `None` = empty no-op. |
| `ack(&self, ids: &[MemoryId], ..)` (mock sketch) | `ack(memory_id, ..)` per slot | One slot per message (the spec's own invariant) — a slice API has nothing to batch. |
| `MockProtocol` fields public inline | `Rc` recorder accessors | The receiver consumes the boxed mock; tests need handles that survive the move. |
| Receiver field `flush_count` on every flush | non-empty flushes only | Spec test 6 pins this exact semantic. |

## For downstream features

- F03 (`html!`): components receive a `SharedInstructionReceiver` clone;
  bindings via `DomSignalBinding::bind` (text) — attribute/event binding
  variants land with the macro.
- F06/F07: mount/morph ops queue through the same receiver.
- Live JS loop: `Runtime::builder().protocol(..).global_arena().build()`,
  then `attach(&signals)`.
