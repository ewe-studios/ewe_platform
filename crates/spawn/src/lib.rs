use cfg_if::cfg_if;
use futures;

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

struct Delay<T: Default + 'static> {
    when: Instant,
    phantom: PhantomData<T>,
}

impl<T: Default + 'static> Delay<T> {
    #[allow(dead_code)]
    pub fn new(d: Instant) -> Self {
        Self {
            when: d,
            phantom: PhantomData::default(),
        }
    }

    #[allow(dead_code)]
    pub fn from(d: Duration) -> Self {
        Self {
            when: Instant::now() + d,
            phantom: PhantomData::default(),
        }
    }
}

impl<T: Default + 'static> Future for Delay<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        if Instant::now() >= self.when {
            Poll::Ready(T::default())
        } else {
            // Ignore this line for now.
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// Spawns and runs a thread-local [`Future`] in a platform-independent way.
///
/// This can be used to interface with any `async` code by spawning a task
/// to run a `Future`.
///
/// ## Limitations
///
/// in WASM:
/// 	This uses Promises underneath and hence will be async.
///
/// in Test:
///  	This blocks current thread until the future completes.
///
///	In Arm, X86, X86/64, Windows:
/// 	When in normal runtime: this blocks current thread and schedules
/// 	future on current thrad for completion until the future completion,
/// 	which means if you call it in a different thread then it runs the
/// 	future in that thread till completion.
///
#[tracing::instrument(skip(future))]
pub fn spawn_local<F>(future: F)
where
    F: futures::Future<Output = ()> + 'static,
{
    cfg_if! {
        if #[cfg(target_arch="wasm32")] {
            wasm_bindgen_futures::spawn_local(future);
        } else if #[cfg(any(test, doctest))] {
            tokio_test::block_on(future);
        } else if #[cfg(feature ="server")] {
            tokio::task::spawn_local(async move {
                future.await;
            })
        } else {
            futures::executor::block_on(future)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::{spawn_local, Delay};

    struct Empty;

    impl Default for Empty {
        fn default() -> Self {
            Self {}
        }
    }

    #[test]
    fn test_spawn_local_using_from() {
        spawn_local(async move {
            let current = Instant::now();
            Delay::<Empty>::from(Duration::from_millis(10)).await;
            println!("Finished After: {:?}", Instant::now() - current);
        });
    }

    #[test]
    fn test_spawn_local_using_new() {
        spawn_local(async move {
            let current = Instant::now();
            Delay::<Empty>::new(Instant::now() + Duration::from_millis(10)).await;
            println!("Finished After: {:?}", Instant::now() - current);
        });
    }
}
