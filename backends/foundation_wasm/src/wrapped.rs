#[cfg(not(target_family = "wasm"))]
pub struct WrappedItem<T>(pub alloc::sync::Arc<foundation_nostd::comp::basic::Mutex<T>>);

#[cfg(target_family = "wasm")]
pub struct WrappedItem<T>(pub alloc::rc::Rc<T>);

#[cfg(target_family = "wasm")]
unsafe impl<T> Sync for WrappedItem<T> {}

#[cfg(target_family = "wasm")]
unsafe impl<T> Send for WrappedItem<T> {}

impl<T> WrappedItem<T> {
    pub fn new(f: T) -> Self {
        #[cfg(target_family = "wasm")]
        {
            Self(alloc::rc::Rc::new(f))
        }

        #[cfg(not(target_family = "wasm"))]
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
