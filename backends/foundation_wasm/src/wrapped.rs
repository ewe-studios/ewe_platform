#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct WrappedItem<T>(pub alloc::sync::Arc<foundation_nostd::comp::basic::Mutex<T>>);

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct WrappedItem<T>(pub alloc::rc::Rc<T>);

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl<T> Sync for WrappedItem<T> {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl<T> Send for WrappedItem<T> {}

impl<T> WrappedItem<T> {
    pub fn new(f: T) -> Self {
        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        {
            Self(alloc::rc::Rc::new(f))
        }

        #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
        {
            Self(alloc::sync::Arc::new(
                foundation_nostd::comp::basic::Mutex::new(f),
            ))
        }
    }
}

impl<T: Clone> Clone for WrappedItem<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
