use alloc::{boxed::Box, vec::Vec};

use crate::TickState;
use foundation_nostd::spin::Mutex;

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub trait IntervalCallback {
    fn perform(&self) -> TickState;
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub trait IntervalCallback: Send + Sync {
    fn perform(&self) -> TickState;
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct FnIntervalCallback(Box<dyn Fn() -> TickState>);

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct FnIntervalCallback(Mutex<Box<dyn Fn() -> TickState + Send + 'static>>);

impl FnIntervalCallback {
    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn() -> TickState + Send + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn() -> TickState + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn new(elem: Box<dyn Fn() -> TickState + Send + 'static>) -> Self {
        Self(Mutex::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn new(elem: Box<dyn Fn() -> TickState>) -> Self {
        Self(elem)
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Sync for FnIntervalCallback {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for FnIntervalCallback {}

impl IntervalCallback for FnIntervalCallback {
    fn perform(&self) -> TickState {
        #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
        {
            (self.0.lock())()
        }

        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        {
            (self.0)()
        }
    }
}

pub struct IntervalCallbackList {
    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    items: Vec<Option<Box<dyn IntervalCallback + Send + Sync + 'static>>>,

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    items: Vec<Option<Box<dyn IntervalCallback + 'static>>>,
}

impl IntervalCallbackList {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
}

impl Default for IntervalCallbackList {
    fn default() -> Self {
        Self::new()
    }
}

impl IntervalCallbackList {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn add(&mut self, handler: Box<dyn IntervalCallback + 'static>) {
        self.items.push(Some(handler));
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn add(&mut self, handler: Box<dyn IntervalCallback + Send + Sync + 'static>) {
        self.items.push(Some(handler));
    }

    pub fn call(&mut self) -> TickState {
        let total = self.items.len();
        let mut to_keep: Vec<usize> = Vec::with_capacity(total);

        for (index, item) in self.items.iter().enumerate() {
            if let Some(elem) = item {
                if TickState::REQUEUE == elem.perform() {
                    to_keep.push(index);
                }
            }
        }

        if to_keep.is_empty() {
            self.items.clear();
            return TickState::STOP;
        }

        let mut place_index = 0;
        for index in to_keep {
            self.items.swap(place_index, index);
            place_index += 1;
        }

        // truncate to only what is kept
        self.items.truncate(place_index);

        TickState::REQUEUE
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Sync for IntervalCallbackList {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for IntervalCallbackList {}

#[cfg(test)]
mod test_interval_registry {
    extern crate std;

    use alloc::boxed::Box;
    use alloc::sync::Arc;

    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_add_when_requeued() {
        let mut registry = IntervalCallbackList::new();

        let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        let copy_value = value.clone();
        let handle = Box::new(FnIntervalCallback::from(move || {
            let mut item = copy_value.lock().unwrap();
            *item = 2;
            TickState::REQUEUE
        }));

        assert_eq!(registry.len(), 0);
        registry.add(handle);

        assert_eq!(registry.len(), 1);
        assert_eq!(*value.lock().unwrap(), 0);

        registry.call();

        assert_eq!(*value.lock().unwrap(), 2);

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_add_when_stopping() {
        let mut registry = IntervalCallbackList::new();

        let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        let copy_value = value.clone();
        let handle = Box::new(FnIntervalCallback::from(move || {
            let mut item = copy_value.lock().unwrap();
            *item = 2;
            TickState::STOP
        }));

        assert_eq!(registry.len(), 0);
        registry.add(handle);

        assert_eq!(registry.len(), 1);
        assert_eq!(*value.lock().unwrap(), 0);

        registry.call();

        assert_eq!(*value.lock().unwrap(), 2);

        assert_eq!(registry.len(), 0);
    }
}
