# Static Initing
Ways to statically initialize a instance (static instance)

See [source](https://github.com/smol-rs/async-io/blob/9d1ca6f8158ce39026c5029a8640dc6f92dee41e/src/reactor.rs#L93-L108)

```rust
impl Reactor {
    /// Returns a reference to the reactor.
    pub(crate) fn get() -> &'static Reactor {
        static REACTOR: OnceCell<Reactor> = OnceCell::new();

        REACTOR.get_or_init_blocking(|| {
            crate::driver::init();
            Reactor {
                poller: Poller::new().expect("cannot initialize I/O event notification"),
                ticker: AtomicUsize::new(0),
                sources: Mutex::new(Slab::new()),
                events: Mutex::new(Events::new()),
                timers: Mutex::new(BTreeMap::new()),
                timer_ops: ConcurrentQueue::bounded(TIMER_QUEUE_SIZE),
            }
        })
    }

...
```
