# Feature 18: Valtron Signals Bridge (cross-thread signals without losing the graph)

**Crate path:** `backends/foundation_signals/src/hub/` (feature `valtron`, native-only, OFF by default)
**Depends on:** F02 (foundation_signals), foundation_core::valtron (multi executor, queues)
**Origin:** the RefCell-vs-Mutex review question (see `features/02-signal-system/status.md`
Q&A, 2026-06-12). Scheduled LAST — design reviewed first, built after F03-F11.

## The problem this solves

The reactive graph is single-threaded by design (G15): `RefCell`/`Rc`, no `Send`
bounds on closures, glitch-free stabilize. Native consumers running under
valtron's multi pool still need to:

1. **write** signals from worker threads (a download task updating progress),
2. **read** signal values from worker threads (a job checking a config signal),
3. **react** to signal changes on worker threads (a logger streaming changes),
4. **register** effects/signals that live on the signal thread, from elsewhere.

The wrong answer is `Mutex<Graph>` + viral `Send` bounds (see the F02 Q&A: you
pay atomics everywhere to either serialize on one big lock or lose determinism).
The right answer is an **actor seam**: the graph stays on ONE thread; other
threads talk to it through Send-safe handles over valtron primitives.

**The property that makes this worth building:** remote readers only ever
observe **post-stabilize snapshots**. Glitch-freedom — no observer sees a
half-propagated diamond — EXTENDS across threads. A lock-everything design
cannot give this (it exposes mid-stabilize intermediate states to whoever grabs
the lock between node evaluations).

---

## 1. Architecture

```
 signal thread (owns Runtime/Contexts)          worker threads (valtron multi pool)
┌──────────────────────────────────┐           ┌─────────────────────────────────┐
│  Runtime + Context (unchanged)   │           │  RemoteSetter<T>::set(v) ───┐   │
│                                  │           │  RemoteSetter<T>::update(f) │   │
│  SignalHub                       │  commands │  RemoteGetter<T>::snapshot()│   │
│   ├─ command queue (MPSC, Send)◄─┼───────────┤  RemoteGetter<T>::changes() │   │
│   ├─ exposure registry           │           │  HubHandle::run_on_hub(f) ──┘   │
│   │   (RemoteId -> apply fn)     │           │                                 │
│   └─ snapshot publishers         │ snapshots │  SignalStream<T>                │
│       (effects writing           ├──────────►│   (TaskIterator — drive it on   │
│        watch cells + streams)    │           │    YOUR executor)               │
│                                  │           │                                 │
│  HubDriver (valtron TaskIterator)│           └─────────────────────────────────┘
│   pump() -> stabilize() -> publish
└──────────────────────────────────┘
```

- **Commands flow IN** through one MPSC queue; each command is applied on the
  signal thread, then one `stabilize()` coalesces everything that arrived in
  the cycle (multiple remote `set`s batch exactly like local ones).
- **Values flow OUT** through per-signal watch cells (latest snapshot) and
  optional change streams — published by ordinary EFFECTS the hub installs, so
  publication is itself part of the graph and inherits all its guarantees.
- **No locks are held across user code** on either side; the signal thread
  never blocks on worker threads.

## 2. Types & API

### 2.1 SignalHub (signal thread)

```rust
/// Owner-side hub. Lives on the thread that owns the Runtime. NOT Send.
pub struct SignalHub {
    runtime: Rc<Runtime>,
    /// Scoped owner of hub-created nodes (publisher effects, remote effects).
    ctx: Context,
    commands: HubReceiver,            // Send MPSC receiver (foundation_core queue)
    registry: RefCell<ExposureRegistry>,
}

impl SignalHub {
    /// Create the hub + its Send handle factory.
    pub fn new(runtime: &Rc<Runtime>) -> SignalHub;

    /// The Send+Sync+Clone handle workers use. Cheap to clone.
    pub fn handle(&self) -> HubHandle;

    /// Drain queued commands, apply them, then stabilize ONCE and publish
    /// snapshots. Returns what happened (for drivers/diagnostics).
    /// This is the ONLY place remote writes touch the graph.
    pub fn pump(&self) -> PumpReport;

    /// Expose a signal for remote READING: installs a publisher effect that
    /// clones each post-stabilize value into a watch cell (+ fan-out streams).
    pub fn expose<T>(&self, getter: &SignalGetter<T>) -> RemoteGetter<T>
    where T: Clone + Send + Sync + 'static;

    /// Expose a setter for remote WRITING: registers an apply-fn keyed by a
    /// Send RemoteId; only VALUES cross threads, never the Rc-based setter.
    pub fn expose_setter<T>(&self, setter: &SignalSetter<T>) -> RemoteSetter<T>
    where T: Clone + PartialEq + Send + 'static;

    /// Both directions at once (the common case).
    pub fn expose_pair<T>(
        &self, getter: &SignalGetter<T>, setter: &SignalSetter<T>,
    ) -> (RemoteGetter<T>, RemoteSetter<T>)
    where T: Clone + PartialEq + Send + Sync + 'static;

    /// A valtron task that drives this hub: each iteration pumps, then yields
    /// Pending (re-armed by queue notification). Schedule it on the SIGNAL
    /// thread's executor (single::spawn or the multi pool's owning thread).
    pub fn driver(self) -> HubDriver;
}

pub struct PumpReport {
    pub commands_applied: usize,
    pub stabilized: bool,       // false when zero commands arrived
}
```

### 2.2 HubHandle (any thread) — `Send + Sync + Clone`

```rust
pub struct HubHandle { tx: HubSender }

impl HubHandle {
    /// Run arbitrary code ON the signal thread, with a Context borrowed from
    /// the hub's scope. THE escape hatch — create signals, effects, computeds
    /// remotely. The closure must be Send (it crosses threads ONCE, then runs
    /// single-threaded and may capture-by-move Send data freely).
    pub fn run_on_hub(&self, f: impl FnOnce(&HubScope) + Send + 'static)
        -> Result<(), HubGone>;

    /// Fire an interop callback remotely (worker-side event sources).
    pub fn invoke_callback(&self, id: u64, data: EventData) -> Result<(), HubGone>;
}

/// What `run_on_hub` closures receive: scoped creation + handle minting.
pub struct HubScope<'hub> { /* &SignalHub internals */ }
impl HubScope<'_> {
    pub fn context(&self) -> &Context;                       // hub-owned scope
    pub fn expose<T>(&self, getter: &SignalGetter<T>) -> RemoteGetter<T> /* … */;
    pub fn expose_setter<T>(&self, setter: &SignalSetter<T>) -> RemoteSetter<T> /* … */;
    /// Send the minted handle back to the caller (pairs with a oneshot the
    /// caller polls — see §3.4 remote signal creation).
}
```

### 2.3 RemoteSetter\<T\> — `Send + Sync + Clone`

```rust
pub struct RemoteSetter<T> { tx: HubSender, id: RemoteId, _t: PhantomData<fn(T)> }

impl<T: Clone + PartialEq + Send + 'static> RemoteSetter<T> {
    /// Enqueue a write. Returns immediately; the value lands at the next pump.
    /// PartialEq dedup happens ON the signal thread (same as local set).
    pub fn set(&self, value: T) -> Result<(), HubGone>;

    /// Enqueue an in-place update (closure crosses once, runs on hub).
    pub fn update(&self, f: impl FnOnce(&mut T) + Send + 'static) -> Result<(), HubGone>;
}
```

`RemoteId` is a `u64` into the hub's exposure registry — the `Rc`-based
`SignalSetter` NEVER crosses a thread boundary; only `RemoteId` + the boxed
value do. A disposed signal's registry entry is removed; commands targeting it
are dropped silently (same contract as stale `callback_id`s).

### 2.4 RemoteGetter\<T\> — `Send + Sync + Clone`

```rust
pub struct RemoteGetter<T> { cell: Arc<WatchCell<T>>, tx: HubSender, id: RemoteId }

impl<T: Clone + Send + Sync + 'static> RemoteGetter<T> {
    /// The latest POST-STABILIZE value. Never blocks the graph; never shows a
    /// mid-stabilize intermediate. (tokio::watch semantics, valtron-native.)
    pub fn snapshot(&self) -> T;

    /// Monotonic publish counter — equality means "nothing changed since".
    pub fn version(&self) -> u64;

    /// A change stream: yields each post-stabilize value (latest-wins if the
    /// consumer lags — it's a state stream, not an event log; see §5).
    pub fn changes(&self) -> SignalStream<T>;
}

/// WatchCell: RwLock<(u64, T)> internally. Writers (the publisher effect, on
/// the signal thread) take the write lock for a clone-in; readers clone out.
/// Reads/writes are short and never overlap user code.
```

### 2.5 SignalStream\<T\> — the valtron consumption surface

```rust
/// TaskIterator over published values. Drive it on ANY executor.
pub struct SignalStream<T> { /* NotifyQueue receiver + last-seen version */ }

impl<T: Clone + Send + 'static> TaskIterator for SignalStream<T> {
    type Ready = T;          // each newly published value
    type Pending = ();       // queue empty — executor re-polls / sleeps
    type Spawner = NoSpawner;
    fn next_status(&mut self) -> Option<TaskStatus<T, (), NoSpawner>>;
    // NEVER loops/blocks in next_status (project rule): empty queue =>
    // Some(Pending), closed hub => None.
}

// usage on a worker:
spawn().with_task(remote_count.changes())
    .with_resolver(Box::new(FnReady::new(|v, _| log_progress(v))))
    .schedule()?;
```

### 2.6 HubDriver — pumping as a first-class valtron task

```rust
/// TaskIterator: Ready(PumpReport) per non-empty pump, Pending while idle.
/// Schedule on the signal thread's executor; the command queue's notify wakes
/// it (no busy spin — Delayed backoff while idle).
pub struct HubDriver { hub: SignalHub }
```

For wasm / manual loops, skip the driver and call `hub.pump()` before/after
your existing `stabilize()` site — `pump` is just a function.

## 3. Semantics (the contract reviewers should poke at)

### 3.1 Ordering
- Commands are FIFO end-to-end: one MPSC queue, applied in arrival order, ONE
  stabilize after the batch. Two `set`s from the same worker land in order; a
  worker's `set` then `run_on_hub` observe each other in order.
- Cross-worker ordering is arrival order (queue is the serialization point) —
  same as any actor.

### 3.2 Visibility / glitch-freedom across threads
- Publisher effects run DURING stabilize (they're ordinary effects at their
  height); the watch-cell write is the last step of each one. A remote
  `snapshot()` therefore returns either the previous stable value or the new
  stable value — never an intermediate. Diamond test: remote observers of D
  can never see "B updated, C not yet".
- `version()` increments once per ACTUAL value change (PartialEq on the signal
  thread), not per pump.

### 3.3 Backpressure (decision 009 stance, adapted)
- Command queue: unbounded by default (writes are tiny; the graph drains every
  pump). `SignalHub::with_capacity(n)` opts into bounded + `set` returning
  `Err(HubBusy)` for callers that prefer shedding to growth.
- Change streams: bounded ring of 1 per subscriber (LATEST-WINS). Signals are
  STATE, not events — a lagging consumer skips intermediate states, exactly
  like a local effect that runs after three `set`s sees only the last value.
  Consumers needing every transition should ship events through a valtron
  channel instead of a signal (document this loudly).

### 3.4 Remote signal creation (pattern, not new API)
```rust
// worker thread: create a signal + get both handles back, via run_on_hub +
// a oneshot built from the same queue primitives.
let (reply_tx, reply_rx) = hub_oneshot::<(RemoteGetter<u64>, RemoteSetter<u64>)>();
handle.run_on_hub(move |scope| {
    let (g, s) = scope.context().signal(0u64);
    let pair = (scope.expose(&g), scope.expose_setter(&s));
    reply_tx.send(pair);
})?;
let (remote_get, remote_set) = reply_rx.recv_via_task()?;  // TaskIterator-friendly
```

### 3.5 Disposal & failure
| Scenario | Behavior |
|----------|----------|
| Hub dropped, worker calls `set`/`run_on_hub` | `Err(HubGone)` — queue closed. |
| Exposed signal's Context disposed | Registry entry removed at next pump; later commands for that `RemoteId` dropped silently (stale-callback contract). `RemoteGetter::snapshot` keeps returning the last published value; `changes()` ends (`None`). |
| Worker drops `SignalStream` | Subscriber slot freed at next publish (weak refs). |
| Closure panics inside `run_on_hub` | Panic on the signal thread (same as a local effect panicking) — NOT swallowed; hub integrity over silent corruption. |
| `update` closure on disposed signal | Dropped with the command, never runs. |

### 3.6 What this deliberately does NOT provide
- No remote `get()` that reads the live graph (would require locking it).
  `snapshot()` is the read; staleness window = one pump cycle.
- No `Send` effects running on worker threads with tracked dependencies —
  dependency tracking is a signal-thread concept. Workers consume STREAMS.
- No multi-hub federation; one hub per Runtime.

## 4. Implementation notes

- Queue: `foundation_core::valtron` NotifyQueue/mpp channel (Send, notify-on-
  push wakes the HubDriver) — no new dependencies.
- `WatchCell`: `RwLock<(u64, T)>` from std (this feature is native-only); the
  hub feature does NOT compile for wasm32 (`compile_error!` guard, mirroring
  the existing `single_threaded` stance).
- Feature flag: `valtron` (implies nothing about the core; zero cost when off).
  Cargo: `foundation_signals = { features = ["valtron"] }` pulls
  `foundation_core` as an optional dep.
- The publisher effect for `expose` is created in the hub's own child Context,
  so dropping the hub disposes all publishers cleanly.
- `#[valtron]`/`#[valtron_test]` (the new macros) are the natural test harness:
  the multi pool exercises real cross-thread traffic.

## 5. Testing plan

| # | Scenario | Verify |
|---|----------|--------|
| 1 | `set` from a worker, pump on signal thread | value lands; ONE stabilize; effect ran once |
| 2 | 100 `set`s from 4 workers, one pump | all applied FIFO; effects ran once; final value = last arrival |
| 3 | Diamond exposed remotely | `snapshot()` polled in a tight worker loop NEVER yields an inconsistent intermediate (B'+C mix) |
| 4 | `changes()` stream | yields each post-stabilize value; lagging consumer gets latest-wins, never a torn value |
| 5 | `version()` | bumps only on actual change (PartialEq dedup on hub) |
| 6 | `run_on_hub` creates signal + exposes (§3.4 pattern) | worker receives working handles |
| 7 | Context disposal | later remote `set` dropped silently; stream ends; snapshot frozen at last value |
| 8 | Hub drop | `set`/`run_on_hub` return `HubGone`; driver task completes |
| 9 | Bounded hub | `HubBusy` surfaces under flood; nothing lost below capacity |
| 10 | `update` closure ordering | `set(1)` then `update(+1)` from same worker = 2 |
| 11 | HubDriver under `#[valtron_test]` (multi pool) | end-to-end: worker task sets, driver pumps, subscriber task observes — no sleeps/spins in test code |
| 12 | Panic in `run_on_hub` closure | propagates on signal thread; queue intact for a fresh hub (process-level test) |

## 6. Review questions for the user (answer before build)

1. **Stream policy**: latest-wins ring-of-1 per subscriber (state semantics) —
   or should `changes()` offer an opt-in bounded N for short histories?
2. **`HubBusy` vs blocking**: bounded mode currently sheds. Should there be a
   `set_blocking` that parks the valtron task (Pending) instead?
3. **Naming**: `SignalHub`/`HubHandle`/`RemoteGetter`/`RemoteSetter` vs
   `SyncBridge`/`watch`-style naming?
4. **Crate placement**: in-crate `feature = "valtron"` (proposed) vs a separate
   `foundation_signals_valtron` crate? In-crate avoids a new workspace member;
   separate keeps foundation_signals dependency-free of foundation_core.
