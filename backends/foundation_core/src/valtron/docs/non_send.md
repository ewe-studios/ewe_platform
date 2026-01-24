# NonSend Types

### Making an Executor non-send

See [source](https://github.com/smol-rs/async-executor/blob/master/src/lib.rs#L449-L455).

Additional [rustonomicon for send](https://doc.rust-lang.org/nomicon/send-and-sync.html).

```rust
/// A thread-local executor.
///
/// The executor can only be run on the thread that created it.
///
/// # Examples
///
/// ```
/// use async_executor::LocalExecutor;
/// use futures_lite::future;
///
/// let local_ex = LocalExecutor::new();
///
/// future::block_on(local_ex.run(async {
///     println!("Hello world!");
/// }));
/// ```
pub struct LocalExecutor<'a> {
    /// The inner executor.
    inner: Executor<'a>,

    /// Makes the type `!Send` and `!Sync`.
    _marker: PhantomData<Rc<()>>,
}
```
