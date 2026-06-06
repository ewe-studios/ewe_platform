# Feature 02: Signal System

**TODO**: Are we taking into considerations the optimizations, fixes and capabilties things like specifications/39-foundation-wasm-ui/learnings/r3-vs-datastar-signals.md is teaching us when we think about all these different signals based systems work (documented in here and in specifications/39-foundation-wasm-ui/learnings/signal.md, specifications/39-foundation-wasm-ui/learnings/datastar-signals-sse.md)

I would like us to talk about it, present to me cleary, ask questions, lets get this right, and adopt the best parts of all these.

## Description

Implement a fine-grained reactive signal system that works on both the Rust/WASM side and the DOM side, with a bidirectional bridge connecting them. No virtual DOM — signals target specific DOM nodes directly.

Inspired by datastar's reactive signals and lit's reactive property system.

## Module

`backends/foundation_wasm_ui/src/shared/signal.rs`

## API Surface

### Rust Signals

```rust
/// A reactive signal holding a value of type T.
/// When the value changes, all subscribers are notified.
pub struct Signal<T: Clone> {
    inner: Arc<SignalInner<T>>,
}

struct SignalInner<T: Clone> {
    value: Mutex<T>,
    subscribers: Mutex<Vec<SubscriberId>>,
    version: AtomicU64,
}

impl<T: Clone> Signal<T> {
    /// Create a new signal with an initial value.
    pub fn new(value: T) -> Self;

    /// Get the current value (cloned).
    pub fn get(&self) -> T;

    /// Set a new value. If the value changed (Eq), notify subscribers.
    pub fn set(&self, value: T);

    /// Update the value using a closure.
    pub fn update<F: FnOnce(&mut T)>(&self, f: F);

    /// Subscribe to changes. Returns a subscription ID for unsubscription.
    pub fn subscribe<F: Fn(&T) + 'static>(&self, callback: F) -> SubscriberId;

    /// Unsubscribe by ID.
    pub fn unsubscribe(&self, id: SubscriberId);

    /// Get the current version number (increments on each change).
    pub fn version(&self) -> u64;
}

/// Computed signal — derived from other signals, lazy evaluation.
pub struct Computed<T: Clone> {
    compute: Box<dyn Fn() -> T + Send + Sync>, /// *TODO*: Question: must this be Send + Sync ?
    cache: Mutex<Option<(u64, T)>>,
    dependencies: Vec<WeakSignalRef>,
}

impl<T: Clone + PartialEq> Computed<T> {
    /// Create a computed signal from a closure that reads other signals.
    pub fn new<F: Fn() -> T + 'static>(compute: F) -> Self;

    /// Get the current computed value (cached if dependencies unchanged).
    pub fn get(&self) -> T;

    /// Convert to a read-only Signal.
    pub fn as_signal(&self) -> Signal<T>;
}
```

### DOM Signal Bridge

**TODO**: I would like to create a true rust signal crate e.g foundation_signals that has no relation at all to anything DOM, which we then bring in and build on in foundation-wasm-ui

```rust
/// Bridge that connects a Rust signal to a DOM element.
/// When the signal changes, the DOM element is updated.
pub struct DomSignalBinding {
    signal_id: u64,
    dom_node: ExternalPointer,
    attribute: Option<String>,   // "textContent", "value", "class", etc.
    transform: Option<Box<dyn Fn(&serde_json::Value) -> String + Send + Sync>>,
    subscription: SubscriberId,
}

*TODO*: how exactly does this binding work ? This is so vague and i cant figure it out.
    
impl DomSignalBinding {
    /// Bind a signal to a DOM element's text content.
    pub fn bind_text<T: Clone + ToString>(
        signal: &Signal<T>,
        node: ExternalPointer,
    ) -> Self;

    /// Bind a signal to a DOM element's attribute.
    pub fn bind_attribute<T: Clone>(
        signal: &Signal<T>,
        node: ExternalPointer,
        attribute: &str,
        transform: impl Fn(&T) -> String + 'static,
    ) -> Self;

    /// Remove the binding (unsubscribe from signal).
    pub fn unbind(self);
}
```

### Event Signal Bridge (DOM → Rust)

*TODO*: how exactly does this binding work ? This is so vague and i cant figure it out.  What exactly is the difference between this and `DomSignalBinding`.

```rust
/// Bridge that connects a DOM event to a Rust signal.
/// When the DOM event fires, the signal is updated.
pub struct EventSignalBinding {
    dom_node: ExternalPointer,
    event_name: String,
    handler_id: u64,  // registered host function
}

impl EventSignalBinding {
    /// Bind a DOM input event to a string signal.
    pub fn bind_input(
        node: ExternalPointer,
        signal: &Signal<String>,
    ) -> Self;

    /// Bind a DOM change event to a bool signal.
    pub fn bind_change(
        node: ExternalPointer,
        signal: &Signal<bool>,
    ) -> Self;

    /// Bind a custom DOM event with a data extractor.
    pub fn bind_custom<T: Clone>(
        node: ExternalPointer,
        event: &str,
        signal: &Signal<T>,
        extract: impl Fn(&str) -> T + 'static,
    ) -> Self;

    /// Remove the binding (unregister host event listener).
    pub fn unbind(self);
}
```

## Implementation Details

### Signal change detection
- Uses `Eq` for equality check — only notify if value actually changed
- Version number increments on each successful change
- Subscribers called synchronously within `set()` (no batching at signal level)
- Computed signals track dependency versions — only recompute if any dep version changed

### Subscriber execution
- Subscribers are called in registration order
- No async — subscribers must be `Fn(&T)` (synchronous)
- For async work, subscriber spawns a valtron task

### DOM signal bindings
- On signal change: queues a DOM update operation into the Arrow batch
- Batch is flushed at the end of the current animation frame (coalescing)
- Multiple signal changes in one frame → single batch apply

### Event signal bindings

**TODO**: Do we really need this when the host runtime provides methods or should provide methods that are module level and do the needed binding via it's js runtime side without an actual struct for this ?

I mean e.g we could bind to a parent, bind to the body and let events bubble up, capture it and send to the right place to wasm, do we even need a struct here on the rust side, especially two structs ? I can understand DOM signal struct, but we need another for event?

- Registers a JS event listener on the DOM node
- Event handler extracts data, sends to WASM via `host_invoke_async_function`
- WASM callback updates the signal
- Signal change may trigger DOM updates (feedback loop prevented by change detection)

### Memory management
- `Signal<T>` uses `Arc` for shared ownership
- `WeakSignalRef` for computed signal dependency tracking (prevents cycles)
- Subscriber IDs are `u64` — simple incrementing counter
- Unsubscribe removes the callback from the list

### Error handling
- No errors in signal system — `set()` is infallible
- Panics only on mutex poisoning (indicates thread panic elsewhere)

## Dependencies

- `foundation_wasm` (for ExternalPointer, host_invoke, callbacks)
- `foundation_nostd` (for Mutex)
- `serde_json` (for event data extraction)

## Testing

- Signal::new + get → returns initial value
- Signal::set with different value → subscriber called
- Signal::set with same value → subscriber NOT called
- Signal::update → value changed, subscriber called
- Computed from two signals → updates when either dep changes
- Computed caching → only recomputes when dep version changes
- DomSignalBinding → signal change queues DOM update
- EventSignalBinding → DOM event updates signal
- Unsubscribe → subscriber no longer called
- Multiple signals in one frame → single batch apply
