# Lifetimes

When you want to make a type ensure it stays longer than the lifetime of a non-static argument, this way
with self being a static reference, you can indicate a argument that is not 'static can life similar
or will not exists longer than `self`

```rust

    /// Spawns a task onto the executor.
    ///
    /// Note: unlike [`Executor::spawn`], this function requires being called with a `'static`
    /// borrow on the executor.
    ///
    /// # Examples
    ///
    /// `
    /// use async_executor::StaticExecutor;
    ///
    /// static EXECUTOR: StaticExecutor = StaticExecutor::new();
    ///
    /// let task = EXECUTOR.spawn(async {
    ///     println!("Hello world");
    /// });
    /// `
    pub fn spawn<T: Send + 'static>(
        &'static self,
        future: impl Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        let (runnable, task) = Builder::new()
            .propagate_panic(true)
            .spawn(|()| future, self.schedule());
        runnable.schedule();
        task
    }

    /// Spawns a non-`'static` task onto the executor.
    ///
    /// ## Safety
    ///
    /// The caller must ensure that the returned task terminates
    /// or is cancelled before the end of 'a.
    pub unsafe fn spawn_scoped<'a, T: Send + 'a>(
        &'static self,
        future: impl Future<Output = T> + Send + 'a,
    ) -> Task<T> {
        // SAFETY:
        //
        // - `future` is `Send`
        // - `future` is not `'static`, but the caller guarantees that the
        //    task, and thus its `Runnable` must not live longer than `'a`.
        // - `self.schedule()` is `Send`, `Sync` and `'static`, as checked below.
        //    Therefore we do not need to worry about what is done with the
        //    `Waker`.
        let (runnable, task) = unsafe {
            Builder::new()
                .propagate_panic(true)
                .spawn_unchecked(|()| future, self.schedule())
        };
        runnable.schedule();
        task
    }

    pub fn spawn<T: 'static>(&'static self, future: impl Future<Output = T> + 'static) -> Task<T> {
        let (runnable, task) = Builder::new()
            .propagate_panic(true)
            .spawn_local(|()| future, self.schedule());
        runnable.schedule();
        task
    }

    /// Spawns a non-`'static` task onto the executor.
    ///
    /// ## Safety
    ///
    /// The caller must ensure that the returned task terminates
    /// or is cancelled before the end of 'a.
    pub unsafe fn spawn_scoped<'a, T: 'a>(
        &'static self,
        future: impl Future<Output = T> + 'a,
    ) -> Task<T> {
        // SAFETY:
        //
        // - `future` is not `Send` but `StaticLocalExecutor` is `!Sync`,
        //   `try_tick`, `tick` and `run` can only be called from the origin
        //    thread of the `StaticLocalExecutor`. Similarly, `spawn_scoped` can only
        //    be called from the origin thread, ensuring that `future` and the executor
        //    share the same origin thread. The `Runnable` can be scheduled from other
        //    threads, but because of the above `Runnable` can only be called or
        //    dropped on the origin thread.
        // - `future` is not `'static`, but the caller guarantees that the
        //    task, and thus its `Runnable` must not live longer than `'a`.
        // - `self.schedule()` is `Send`, `Sync` and `'static`, as checked below.
        //    Therefore we do not need to worry about what is done with the
        //    `Waker`.
        let (runnable, task) = unsafe {
            Builder::new()
                .propagate_panic(true)
                .spawn_unchecked(|()| future, self.schedule())
        };
        runnable.schedule();
        task
    }
```
