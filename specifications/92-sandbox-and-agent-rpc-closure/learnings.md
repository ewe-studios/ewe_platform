# Spec 92 Learnings

## Why this spec exists (2026-11-27)

Specs 77 and 83 merged with three kinds of debt:

1. **Proof debt.** The real Cloudflare container path was red, and no live run had used the
   owner's account. The real AWS and GCP KMS paths had never run.
2. **Fidelity debt.** The DST harness used a scripted registry, so the production registry
   code was not under simulation.
3. **Small correctness debt.** Eleven findings stayed open because each feature could land
   without them.

This spec exists to pay those debts. A feature of this spec is done when its issues are
closed, not when its code compiles.

## The Cloudflare create stall (2026-11-28)

- **The layer:** the Durable Object (DO), not the client. The client deadline
  (`rpc_timeout`, 5 s) only reported the stall.
- **The spin:** `ControlService::create` consumed the driving create iterator in a
  synchronous `futures::stream::poll_fn` loop. `Stream::Wait` means "the queue is empty;
  run the producer again", so the loop called `next()` again. A wasm host has one thread
  and one event loop: the loop never returned to it, so every promise (DO storage, the
  container fetch, the timers) stayed pending.
- **The wide effect:** the loop held the whole isolate. A raw request that the entry
  refuses with no DO also timed out. This ruled out a simple "slow create" and named a
  synchronous loop as the cause.
- **The fix:** park on the injected timer for `create_poll_interval` (4 ms) on `Wait`,
  then pull again. The event loop runs between the pulls. The remote create also uses the
  create deadline `boot_timeout` (10 s) for the whole stream; the unary `rpc_timeout`
  (5 s) was too short for a cold container start.
- **`fix/dst-wait-redrive` (#1116):** it changes the five parking stream-to-future
  adapters. It does not change this hand-written loop, so it would not fix this failure.
  It stays useful for the adapters. Its sibling issues #1128 and #1129 cover the
  nested-engine case and the `Pending` case.
- **The second cause:** the slave used `/.dockerenv` and `/run/.containerenv` to detect a
  container. The Cloudflare runtime writes neither, so the slave ran the virtual-machine
  boot duties in a container and died. It now also checks the `CLOUDFLARE_*` environment
  (issue #1133).
- **The next layer:** the create fix alone cannot make the job green. The end-to-end suite
  now exposes three independent defects: the streaming route reads a Connect envelope as
  JSON (#1140), the HTTP client does not decompress gzip (#1141), and the sandbox DO has no
  model configuration (#1142).
