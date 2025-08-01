use alloc::{boxed::Box, vec::Vec};

#[derive(PartialEq, Eq, Debug, Clone)]
#[repr(u8)]
pub enum TickState {
    REQUEUE = 1,
    STOP = 2,
}

impl TickState {
    pub fn into_u8(self) -> u8 {
        self as u8
    }
}

impl From<TickState> for u8 {
    fn from(val: TickState) -> Self {
        val as u8
    }
}

impl From<u8> for TickState {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::REQUEUE,
            2 => Self::STOP,
            _ => unreachable!("No other u8 state is supported"),
        }
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub trait FrameCallback {
    fn tick(&self, value: f64) -> TickState;
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub trait FrameCallback: Send + Sync {
    fn tick(&self, value: f64) -> TickState;
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct FnFrameCallback(Box<dyn Fn(f64) -> TickState>);

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct FnFrameCallback(
    foundation_nostd::spin::Mutex<Box<dyn Fn(f64) -> TickState + Send + 'static>>,
);

impl FnFrameCallback {
    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn(f64) -> TickState + Send + 'static,
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
        Self(foundation_nostd::spin::Mutex::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn new(elem: Box<dyn Fn(f64) -> TickState>) -> Self {
        Self(elem)
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Sync for FnFrameCallback {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for FnFrameCallback {}

impl FrameCallback for FnFrameCallback {
    fn tick(&self, value: f64) -> TickState {
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

pub struct FrameCallbackList {
    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    items: Vec<Option<Box<dyn FrameCallback + Send + Sync + 'static>>>,

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    items: Vec<Option<Box<dyn FrameCallback + 'static>>>,
}

impl FrameCallbackList {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub const fn create() -> Self {
        Self { items: Vec::new() }
    }
}

impl Default for FrameCallbackList {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCallbackList {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn add(&mut self, handler: Box<dyn FrameCallback + 'static>) {
        self.items.push(Some(handler));
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn add(&mut self, handler: Box<dyn FrameCallback + Send + Sync + 'static>) {
        self.items.push(Some(handler));
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
unsafe impl Sync for FrameCallbackList {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for FrameCallbackList {}

#[cfg(test)]
mod test_frame_callback_registry {
    extern crate std;

    use alloc::boxed::Box;
    use alloc::sync::Arc;

    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_add_when_requeued() {
        let mut registry = FrameCallbackList::new();

        let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        let copy_value = value.clone();
        let handle = Box::new(FnFrameCallback::from(move |value| {
            let mut item = copy_value.lock().unwrap();
            *item = value as usize;
            TickState::REQUEUE
        }));

        assert_eq!(registry.len(), 0);
        registry.add(handle);

        assert_eq!(registry.len(), 1);
        assert_eq!(*value.lock().unwrap(), 0);

        registry.call(2.0);

        assert_eq!(*value.lock().unwrap(), 2);

        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_add_when_stopping() {
        let mut registry = FrameCallbackList::new();

        let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        let copy_value = value.clone();
        let handle = Box::new(FnFrameCallback::from(move |value| {
            let mut item = copy_value.lock().unwrap();
            *item = value as usize;
            TickState::STOP
        }));

        assert_eq!(registry.len(), 0);
        registry.add(handle);

        assert_eq!(registry.len(), 1);
        assert_eq!(*value.lock().unwrap(), 0);

        registry.call(2.0);

        assert_eq!(*value.lock().unwrap(), 2);

        assert_eq!(registry.len(), 0);
    }
}
