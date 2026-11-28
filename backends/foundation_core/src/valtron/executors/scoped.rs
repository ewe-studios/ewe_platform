//! The scoped sync boundary: drive a borrowed future on the calling thread.

/// Drive a borrowed, non-`Send` future on the calling thread.
///
/// [`block_on_future`](crate::valtron::block_on_future) needs `Send + 'static`,
/// because it hands the future to a pool worker. A test often borrows local
/// data instead. This function pins the future on the caller's stack and parks
/// the thread until it is ready. It is the scoped sync boundary, and it uses
/// no second executor.
///
/// # Panics
/// Panics on a target without threads, such as wasm32 without a worker.
#[cfg(not(target_arch = "wasm32"))]
pub fn block_on_future_scoped<F: core::future::Future>(future: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::park();
    }
}
