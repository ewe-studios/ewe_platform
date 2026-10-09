---
feature: "ReadinessSource Reactor Seam"
description: "foundation_core defines a reactor plug-in trait over the wake queue — no reactor dependency pulled in; in-tree test reactor proves it"
status: "pending"
priority: "medium"
depends_on: ["01-waker-queue-bridge"]
estimated_effort: "medium"
created: 2026-06-17
---

# Feature 02: ReadinessSource Reactor Seam

## Description

Level 1 (F01) parks futures correctly when they wake via the context waker. But
a leaf future blocked on a real OS resource (socket, timer) only progresses if
*something* fires its waker when the resource is ready. On wasm the browser
event loop does this (`JsFuture`). Native needs a reactor — and
`foundation_core` must NOT depend on `mio`/`polling`/tokio.

This feature adds the **seam**: a trait in `foundation_core` that an external
reactor (living in a platform crate) implements and registers. The wake path is
the SAME `ConcurrentQueue<WakeToken>` from F01.

## Traits (foundation_core — NO new dependency)

```rust
/// A source that arranges to push a WakeToken onto a task's wake queue when an
/// external resource becomes ready. The implementation lives OUTSIDE
/// foundation_core (e.g. foundation_netio); foundation_core owns only the trait.
pub trait ReadinessSource: Send + Sync {
    /// Register interest. When `interest` becomes ready, the source pushes
    /// `token` onto `wake` (the future's F01 wake queue). The returned handle
    /// deregisters on drop (RAII).
    fn register(
        &self,
        interest: Interest,
        wake: Arc<ConcurrentQueue<WakeToken>>,
        token: WakeToken,
    ) -> Box<dyn ReadinessRegistration>;
}

/// RAII handle — dropping it deregisters the interest.
pub trait ReadinessRegistration: Send + Sync {}

/// What a task waits for. Abstract so foundation_core needs no OS types.
#[non_exhaustive]
pub enum Interest {
    Readable,
    Writable,
    Timer(core::time::Duration),
    /// Opaque platform-defined interest (e.g. a specific event id).
    Custom(u64),
}
```

## Global registration slot (optional, same pattern as pool singletons)

```rust
// foundation_core::valtron — a place for a platform reactor to register itself
// once at startup. None on wasm (browser drives wakers). OnceLock, not a dep.
static READINESS_SOURCE: OnceLock<Arc<dyn ReadinessSource>> = OnceLock::new();

pub fn set_readiness_source(src: Arc<dyn ReadinessSource>) -> Result<(), ...>;
pub fn readiness_source() -> Option<Arc<dyn ReadinessSource>>;
```

## How a leaf future uses it

The leaf future (which knows its fd/timer) — NOT foundation_core — calls
`register` in its own `poll` when returning `Pending`, handing over the wake
queue it received via the context waker:

```rust
// inside some socket future's poll(), in a PLATFORM crate:
if let Some(src) = foundation_core::valtron::readiness_source() {
    // obtain the F01 wake queue (see Open Question 1 in requirements) and a token
    self.reg = Some(src.register(Interest::Readable, wake_queue, token));
}
Poll::Pending
```

When the reactor sees the fd ready, it pushes the token → F01's `QueueReadiness`
becomes ready → the executor re-runs the task. `foundation_core` never sees an
fd.

## What foundation_core owns vs. what it doesn't

| Owns (foundation_core) | Does NOT own |
|---|---|
| `ReadinessSource` / `ReadinessRegistration` traits | Any reactor implementation |
| `Interest` enum | `mio` / `polling` / epoll / kqueue |
| `READINESS_SOURCE` registration slot | OS fd / socket / timer types |
| `WakeToken` + the wake queue (from F01) | The thread that polls the OS |

The reactor PLUGS IN; it is not PULLED IN.

## Proof in this feature: an in-tree TEST reactor only

To validate the seam without a real reactor or new dependency, implement a tiny
test `ReadinessSource` inside foundation_core's tests:

```rust
/// Test reactor: spawns a thread that pushes the token after `Interest::Timer`.
struct TimerTestReactor;
impl ReadinessSource for TimerTestReactor {
    fn register(&self, interest, wake, token) -> Box<dyn ReadinessRegistration> {
        if let Interest::Timer(d) = interest {
            std::thread::spawn(move || { std::thread::sleep(d); let _ = wake.push(token); });
        }
        Box::new(NoopReg)
    }
}
```

A future that registers `Timer(50ms)` and returns `Pending` must be driven to
completion by the executor purely via the pushed token — proving park→wake works
end-to-end through the seam. This stays in tests; **no production reactor ships
in this spec.**

## Scope boundary

- **In scope:** the traits, the registration slot, and the in-tree test reactor.
- **Out of scope:** a native `polling`/`mio` reactor (separate spec, lives in
  `foundation_netio` or `foundation_nativeapis`), and any wasm reactor (the
  browser already drives `JsFuture` wakers — F01 suffices there).

## Module changes

- `backends/foundation_core/src/valtron/` — new `readiness_source.rs` (traits,
  `Interest`, registration slot); re-export from `valtron` module root
- Tests only: `TimerTestReactor` proving the seam

## Testing (use `#[valtron_test]`)

- Register `TimerTestReactor`; a future that depends on `Timer(50ms)` completes
  via the pushed token, not via busy re-poll.
- Dropping the `ReadinessRegistration` deregisters (test reactor records it).
- With NO `ReadinessSource` registered, behaviour falls back to F01 (context
  waker only) — confirm a self-waking future still completes.
