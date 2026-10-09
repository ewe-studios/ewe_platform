use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

/// Zero-cost wrapper that asserts `Send` for a `!Send` future on single-threaded
/// wasm targets. On native/emscripten (real threads), `Send` auto-derives from
/// the inner type — wrapping a `!Send` future is a compile error, as intended.
///
/// # Safety (wasm-only `Send` impl)
///
/// Sound **only** on single-threaded wasm (`target_family = "wasm"` without
/// `atomics` target feature and not `emscripten`). There are no other threads,
/// so the `Send` assertion is vacuously true.
#[repr(transparent)]
pub struct SendWrapper<T>(T);

#[cfg(all(
    target_family = "wasm",
    not(target_os = "emscripten"),
    not(target_feature = "atomics")
))]
// SAFETY: single-threaded wasm — no concurrent threads exist.
unsafe impl<T> Send for SendWrapper<T> {}

#[cfg(all(
    target_family = "wasm",
    not(target_os = "emscripten"),
    not(target_feature = "atomics")
))]
// SAFETY: single-threaded wasm — no concurrent threads exist.
unsafe impl<T> Sync for SendWrapper<T> {}

impl<T> SendWrapper<T> {
    #[inline]
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<F: Future> Future for SendWrapper<F> {
    type Output = F::Output;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: SendWrapper is #[repr(transparent)], so Pin projection is sound.
        let inner = unsafe { self.map_unchecked_mut(|s| &mut s.0) };
        inner.poll(cx)
    }
}
