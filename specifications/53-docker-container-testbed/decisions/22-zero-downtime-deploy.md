# 22 — Zero-downtime deployment + canary rollout (Stage 3)

**Date:** 2026-07-10
**Status:** Resolved

## Decision

`foundation_proxy` supports deploying new backends without dropping a single
in-flight request. The deployment sequence: register new backends at zero weight,
gradually shift traffic (canary), drain old backends, remove them. The atomic
swap (`replace_backends`) is the fast path for instant cutover.

kamal-proxy does this via `kamal-proxy deploy --target host:port` which is a
blocking command that health-checks, swaps, and drains. We split this into
fine-grained RPC commands so the orchestrator (`foundation_deployment_platform`)
can drive the sequence and handle failures at each step.

## Why

The current proxy has `BackendState::Active/Draining/Paused` and in-flight
refcounting — the building blocks are there. But there's no orchestrated
deployment sequence. This decision wires the building blocks together into a
complete deploy:

1. Zero-downtime: in-flight requests complete before old backend is removed
2. Canary: traffic shifts gradually, observable at each step
3. Atomic: instant cutover when canary isn't needed

## Deploy sequence

```
deployment_platform                     foundation_proxy (RPC)
      │                                        │
      │  start new container                   │
      │  health-check new container             │
      │                                        │
      │── deploy(app, url=new, weight=0) ──→   │ register backend (0% traffic)
      │                                        │
      │── set_weight(app, new, 10) ────────→   │ 10% → new, 90% → old
      │   (observe metrics)                    │
      │── set_weight(app, new, 50) ────────→   │ 50% → new, 50% → old
      │   (observe metrics)                    │
      │── set_weight(app, new, 100) ───────→   │ 100% → new
      │                                        │
      │── replace_backends(app, [new]) ────→   │ atomically: old backends → Draining
      │                                        │   wait for in-flight → 0
      │                                        │   remove old backends
      │                                        │
      │  stop old container                     │
```

## Atomic swap (`replace_backends`)

```rust
impl ServiceRuntime {
    /// Atomically replace all backends for this service.
    ///
    /// 1. Mark current backends as `Draining` (no new traffic).
    /// 2. Insert new backends at their configured weights.
    /// 3. Wait for in-flight count on old backends to reach zero
    ///    (up to `drain_timeout`).
    /// 4. Remove old backends.
    /// 5. Persist to DB.
    pub fn replace_backends(
        &self,
        new_backends: Vec<BackendRuntime>,
        drain_timeout: Duration,
    ) -> Result<ReplaceResult, ProxyError>;
}
```

## Weighted canary rollout

```rust
/// Gradually shift traffic by adjusting a backend's weight.
/// Called by the orchestrator at each canary step.
///
/// # Example
/// canary from 0→10→50→100, checking metrics at each step:
///   rpc.set_weight("app", backend_id, 10)?;
///   sleep(canary_interval);
///   rpc.set_weight("app", backend_id, 50)?;
///   sleep(canary_interval);
///   rpc.set_weight("app", backend_id, 100)?;
pub fn set_weight(&self, service: &str, backend_id: &str, weight: u32) -> Result<(), ProxyError>;
```

Weight changes take effect on the next call to `ServiceRuntime::pick()` — which
is the next incoming request. No buffering, no delay. The orchestrator owns
the timing between weight steps.

## Drain

When a backend is set to `Draining`:
1. `is_eligible()` returns `false` — `pick()` skips it
2. Existing in-flight requests complete normally
3. `BackendLease::drop` decrements the in-flight counter
4. The drainer polls `inflight() == 0` (with timeout)
5. Once drained, the backend is removed from the service and DB

## Integration with existing code

All the primitives exist and are tested:
- `BackendState::Draining` — `is_eligible()` already excludes it
- `BackendLease` — RAII in-flight counter already works
- `ServiceRuntime::pick()` — smooth WRB with eligibility gating
- `BackendRuntime::set_state()` — mutex-protected state transitions

This decision adds the **orchestration layer** that sequences these primitives
into a deploy, plus the RPC commands that expose them.

## Verification

1. Start proxy with 2 backends (A and B), all traffic hits both.
2. Call `replace_backends(app, [C])` — traffic shifts to C, A and B drain,
   removed when in-flight=0.
3. No requests return 502 during the swap (zero-downtime).
4. Canary: set C weight=0→10→50→100, observe traffic distribution at each step.
5. Drain timeout: if a backend never drains (stuck connection), the swap
   completes after timeout with a warning.
