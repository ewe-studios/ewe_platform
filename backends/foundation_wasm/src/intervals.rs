use alloc::{boxed::Box, vec::Vec};

use crate::InternalPointer;
use foundation_nostd::spin::Mutex;

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub trait DoTask {
    fn do(&self);
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub trait DoTask: Send + Sync {
    fn do(&self);
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct FnDoTask(Box<dyn Fn()>);

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct FnDoTask(Mutex<Box<dyn Fn()  + Send + 'static>>);

impl FnDoTask {
    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn()  + Send + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn(f64) -> TickState + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn new(elem: Box<dyn Fn(f64) -> TickState + Send + 'static>) -> Self {
        Self(Mutex::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn new(elem: Box<dyn Fn(f64) -> TickState>) -> Self {
        Self(elem)
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Sync for FnDoTask {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for FnDoTask {}

impl DoTask for FnDoTask {
    fn do(&self, value: f64) -> TickState {
        #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
        {
            (self.0.lock())(value)
        }

        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        {
            (self.0)(value)
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
    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn add(&mut self, handler: Box<dyn IntervalCallback + 'static>) -> InternalPointer {
        let next_index = self.items.len();
        self.items.push(Some(handler));
        InternalPointer::from(next_index as u64)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn add(
        &mut self,
        handler: Box<dyn IntervalCallback + Send + Sync + 'static>,
    ) -> InternalPointer {
        let next_index = self.items.len();
        self.items.push(Some(handler));
        InternalPointer::from(next_index as u64)
    }

    pub fn call(&mut self, value: f64) -> TickState {
        let total = self.items.len();
        let mut to_keep: Vec<usize> = Vec::with_capacity(total);

        for (index, item) in self.items.iter().enumerate() {
            if let Some(elem) = item {
                if TickState::REQUEUE == elem.tick(value) {
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
