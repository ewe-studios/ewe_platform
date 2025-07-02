use alloc::{boxed::Box, collections::btree_map::BTreeMap};

use crate::{InternalPointer, WrappedItem};
use foundation_nostd::spin::Mutex;

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub trait DoTask {
    fn perform(&self);
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub trait DoTask: Send + Sync {
    fn perform(&self);
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct FnDoTask(Box<dyn Fn()>);

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct FnDoTask(Mutex<Box<dyn Fn() + Send + 'static>>);

impl FnDoTask {
    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn() + Send + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn() + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn new(elem: Box<dyn Fn() + Send + 'static>) -> Self {
        Self(Mutex::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn new(elem: Box<dyn Fn()>) -> Self {
        Self(elem)
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Sync for FnDoTask {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for FnDoTask {}

impl DoTask for FnDoTask {
    fn perform(&self) {
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

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct ScheduleRegistry {
    tree: BTreeMap<InternalPointer, WrappedItem<Box<dyn DoTask + 'static>>>,
    id: u64,
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct ScheduleRegistry {
    tree: BTreeMap<InternalPointer, WrappedItem<Box<dyn DoTask + Sync + Send + 'static>>>,
    id: u64,
}

// -- Constructors

#[allow(unused)]
impl ScheduleRegistry {
    pub(crate) fn new() -> Self {
        Self {
            id: 0,
            tree: BTreeMap::new(),
        }
    }
}

impl ScheduleRegistry {
    pub const fn create() -> Mutex<Self> {
        Mutex::new(Self {
            id: 0,
            tree: BTreeMap::new(),
        })
    }
}

// -- Methods

impl ScheduleRegistry {
    pub fn delete(&mut self, id: InternalPointer) -> Option<()> {
        self.tree.remove(&id).map(|_| ())
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
impl ScheduleRegistry {
    pub fn call(&self, id: InternalPointer) -> Option<()> {
        if let Some(callback) = self.tree.get(&id) {
            callback.0.perform();
            return Some(());
        }
        None
    }

    pub fn add(&mut self, callback: Box<dyn DoTask + 'static>) -> InternalPointer {
        self.id += 1;
        let id = self.id;
        let wrapped = WrappedItem::new(callback);
        self.tree.insert(InternalPointer::from(id), wrapped);
        InternalPointer::from(id)
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
impl ScheduleRegistry {
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn call(&self, id: InternalPointer) -> Option<()> {
        if let Some(callback) = self.tree.get(&id) {
            callback.0.lock().perform();
            return Some(());
        }
        None
    }

    pub fn add(&mut self, callback: Box<dyn DoTask + Send + Sync + 'static>) -> InternalPointer {
        self.id += 1;
        let id = self.id;
        let wrapped = WrappedItem::new(callback);
        self.tree.insert(InternalPointer::from(id), wrapped);
        InternalPointer::from(id)
    }
}

#[cfg(test)]
mod test_schedule_registry {
    extern crate std;

    use alloc::boxed::Box;
    use alloc::sync::Arc;

    use super::FnDoTask;
    use super::ScheduleRegistry;
    use std::sync::Mutex;

    #[test]
    fn test_add() {
        let mut registry = ScheduleRegistry::new();

        let value: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        let copy_value = value.clone();
        let handle = Box::new(FnDoTask::from(move || {
            let mut item = copy_value.lock().unwrap();
            *item = 2;
        }));

        let id = registry.add(handle);

        assert_eq!(*value.lock().unwrap(), 0);

        registry.call(id);

        assert_eq!(*value.lock().unwrap(), 2);
    }
}
