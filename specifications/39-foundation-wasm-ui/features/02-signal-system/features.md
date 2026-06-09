# Feature 02: Signal System (foundation_signals)

## Description

Create `foundation_signals` — a standalone reactive signal crate with no DOM coupling. Pure Rust, works anywhere. Uses R3-style architecture: height-based topological ordering, bucket queue, version-based stale detection, diamond dependency safety, explicit disposal, batch coalescing via `stabilize()`. Signals return `(getter, setter)` tuples for ergonomic function-style components.

**Decisions:** 002, 003, 004, 011, 029

## Crate

`crates/foundation_signals/` (depends on `foundation_ui_traits`)

## Types

### Runtime

```rust
pub struct Runtime { /* bucket queue, version counter, active tracking */ }
```

### Context

```rust
impl Context {
    pub fn signal<T>(&self, initial: T) -> (SignalGetter<T>, SignalSetter<T>);
    pub fn computed<T>(&self, f: impl Fn() -> T) -> ComputedGetter<T>;
    pub fn effect(&self, f: impl FnMut());
    pub fn child(&self) -> Context;
}
```

### Getter / Setter

```rust
impl<T: Clone> SignalGetter<T> { pub fn get(&self) -> T; }
impl<T> SignalSetter<T> {
    pub fn set(&self, value: T);
    pub fn update<F: FnOnce(&mut T)>(&self, f: F);
    pub fn callback_id(&self) -> u64;
}
```

### Computed

Height = max(dep heights) + 1. Diamond-safe. Version-based cache invalidation.

### Callback Registry

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
- Callback registry → monotonic IDs, stale calls dropped
- stabilize() → single flush after multiple sets