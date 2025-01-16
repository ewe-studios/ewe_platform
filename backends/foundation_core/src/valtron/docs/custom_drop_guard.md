# Drop Guards

You can implement custom checks and balances to deal with drops of async types

```rust

      /// A guard that closes the task if polling its future panics.
      struct Guard<F, T, S, M>(RawTask<F, T, S, M>)
      where
          F: Future<Output = T>,
          S: Schedule<M>;

      impl<F, T, S, M> Drop for Guard<F, T, S, M>
      where
          F: Future<Output = T>,
          S: Schedule<M>,
      {
          fn drop(&mut self) {
              let raw = self.0;
              let ptr = raw.header as *const ();

              unsafe {
                  let mut state = (*raw.header).state.load(Ordering::Acquire);

                  loop {
                      // If the task was closed while running, then unschedule it, drop its
                      // future, and drop the task reference.
                      if state & CLOSED != 0 {
                          // The thread that closed the task didn't drop the future because it
                          // was running so now it's our responsibility to do so.
                          RawTask::<F, T, S, M>::drop_future(ptr);

                          // Mark the task as not running and not scheduled.
                          (*raw.header)
                              .state
                              .fetch_and(!RUNNING & !SCHEDULED, Ordering::AcqRel);

                          // Take the awaiter out.
                          let mut awaiter = None;
                          if state & AWAITER != 0 {
                              awaiter = (*raw.header).take(None);
                          }

                          // Drop the task reference.
                          RawTask::<F, T, S, M>::drop_ref(ptr);

                          // Notify the awaiter that the future has been dropped.
                          if let Some(w) = awaiter {
                              abort_on_panic(|| w.wake());
                          }
                          break;
                      }

                      // Mark the task as not running, not scheduled, and closed.
                      match (*raw.header).state.compare_exchange_weak(
                          state,
                          (state & !RUNNING & !SCHEDULED) | CLOSED,
                          Ordering::AcqRel,
                          Ordering::Acquire,
                      ) {
                          Ok(state) => {
                              // Drop the future because the task is now closed.
                              RawTask::<F, T, S, M>::drop_future(ptr);

                              // Take the awaiter out.
                              let mut awaiter = None;
                              if state & AWAITER != 0 {
                                  awaiter = (*raw.header).take(None);
                              }

                              // Drop the task reference.
                              RawTask::<F, T, S, M>::drop_ref(ptr);

                              // Notify the awaiter that the future has been dropped.
                              if let Some(w) = awaiter {
                                  abort_on_panic(|| w.wake());
                              }
                              break;
                          }
                          Err(s) => state = s,
                      }
                  }
              }
          }
      }
  }
}
```
