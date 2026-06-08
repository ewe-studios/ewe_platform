# Feature 02: Signal System (foundation_signals)

## Description

Create `foundation_signals` — a standalone reactive signal crate with no DOM coupling. Pure Rust, works anywhere. Uses R3-style architecture: height-based topological ordering, bucket queue, version-based stale detection, diamond dependency safety, explicit disposal, batch coalescing via `stabilize()`.

Signals return `(getter, setter)` tuples for ergonomic function-style components. Two-way bindings (DOM → WASM) use compile-time registered callbacks with numeric IDs — no string keys.

**Decisions:** 002, 003, 004, 011, 029

## Crate

`crates/foundation_signals/` (depends on `foundation_ui_traits`)

## Types

### Runtime

```rust
pub struct Runtime { /* bucket queue, version counter, active tracking */ }

impl Runtime {
    pub fn new() -> Self;
    pub fn context(&self) -> Context;
}
```

One global Runtime per app. Unified graph across all contexts.

### Context

```rust
pub struct Context { /* sub-graph for ownership */ }

impl Context {
    pub fn signal<T>(&self, initial: T) -> (SignalGetter<T>, SignalSetter<T>);
    pub fn computed<T>(&self, f: impl Fn() -> T) -> ComputedGetter<T>;
    pub fn effect(&self, f: impl FnMut());
    pub fn child(&self) -> Context;
}
```

Contexts are sub-graphs within the unified graph. Drop a Context → cascade disposal.

### Getter / Setter

```rust
/// Read-only access to a signal's value. Subscribes to deps when called inside effect/computed.
pub struct SignalGetter<T> { /* Arc reference */ }

impl<T: Clone> SignalGetter<T> {
    pub fn get(&self) -> T;
}

/// Write access to a signal's value. Triggers stabilize().
pub struct SignalSetter<T> { /* Arc reference */ }

impl<T> SignalSetter<T> {
    pub fn set(&self, value: T);
    pub fn update<F: FnOnce(&mut T)>(&self, f: F);
    pub fn callback_id(&self) -> u64;  // for compile-time cross-boundary registration
}
```

Signals have no string keys — identity is the Arc reference. Setters carry a numeric ID for compile-time event binding registration.

### Computed

```rust
pub struct ComputedGetter<T> { /* lazy, cached, height-based ordering */ }

impl<T: Clone> ComputedGetter<T> {
    pub fn get(&self) -> T;
}
```

Height = max(dep heights) + 1. Diamond-safe. Version-based cache invalidation.

### Effect

Effects are nodes in the signal graph — same height ordering as computeds. On creation: assigned height, linked into dependency graph. On Drop: deferred removal after stabilize loop.

### Callback Registry

```rust
/// Compile-time registered two-way bindings.
/// Each setter gets a unique u64 ID. JS calls this ID to trigger updates.
pub struct CallbackRegistry { /* monotonic counter, never reused */ }

impl CallbackRegistry {
    pub fn register(&mut self, f: impl FnMut(serde_json::Value)) -> u64;
    pub fn invoke(&mut self, id: u64, data: serde_json::Value);
    pub fn unregister(&mut self, id: u64);
}
```

Monotonic IDs — never reused. Stale call → silently dropped.

## Dependencies

- `foundation_ui_traits`
- `serde_json` (for event data)

## Testing

- Signal get/set → value changes, subscribers notified
- Setter.update → closure applied atomically
- Computed from two signals → updates when either dep changes
- Diamond dependency → computed runs once per stabilize
- Context drop → all owned signals disposed
- Effect → queues DomOp on signal change
- Callback registry → monotonic IDs, stale calls dropped
- stabilize() → single flush after multiple sets
