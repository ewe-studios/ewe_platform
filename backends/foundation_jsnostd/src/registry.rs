use alloc::{boxed::Box, collections::btree_map::BTreeMap};

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
use alloc::rc::Rc;

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
use alloc::sync::Arc;

use foundation_nostd::spin::Mutex;

use crate::InternalPointer;

pub trait InternalCallback {
    fn receive(&self, start_pointer: *const u8, length: u64);
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct WrappedInternalCallback(Rc<Box<dyn InternalCallback + 'static>>);

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
impl Clone for WrappedInternalCallback {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Sync for WrappedInternalCallback {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for WrappedInternalCallback {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
impl InternalCallback for WrappedInternalCallback {
    fn receive(&self, start_pointer: *const u8, length: u64) {
        self.0.receive(start_pointer, length);
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
impl WrappedInternalCallback {
    pub fn new<F>(f: F) -> Self
    where
        F: InternalCallback + 'static,
    {
        Self(Rc::new(Box::new(f)))
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct WrappedInternalCallback(Arc<Mutex<Box<dyn InternalCallback + Send + Sync + 'static>>>);

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
impl Clone for WrappedInternalCallback {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
impl InternalCallback for WrappedInternalCallback {
    fn receive(&self, start_pointer: *const u8, length: u64) {
        self.0.lock().receive(start_pointer, length);
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
impl WrappedInternalCallback {
    pub fn new<F>(f: F) -> Self
    where
        F: InternalCallback + Send + Sync + 'static,
    {
        Self(Arc::new(Mutex::new(Box::new(f))))
    }
}

pub type CallbackFn = dyn Fn(*const u8, u64) + 'static;

pub struct FnCallback(Box<dyn Fn(*const u8, u64)>);

impl FnCallback {
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn(*const u8, u64) + 'static,
    {
        Self(Box::new(elem))
    }

    pub fn new(elem: Box<dyn Fn(*const u8, u64)>) -> Self {
        Self(elem)
    }

    pub fn receive(&mut self, start_pointer: *const u8, length: u64) {
        (self.0)(start_pointer, length)
    }
}

pub struct InternalReferenceRegistry {
    tree: BTreeMap<InternalPointer, WrappedInternalCallback>,
    id: u64,
}

// -- Constructors

#[allow(unused)]
impl InternalReferenceRegistry {
    pub(crate) fn new() -> Self {
        Self {
            id: 0,
            tree: BTreeMap::new(),
        }
    }
}

impl InternalReferenceRegistry {
    pub const fn create() -> Mutex<Self> {
        Mutex::new(Self {
            id: 0,
            tree: BTreeMap::new(),
        })
    }
}

// -- Methods

impl InternalReferenceRegistry {
    pub fn remove(&mut self, id: InternalPointer) -> Option<WrappedInternalCallback> {
        self.tree.remove(&id)
    }

    pub fn get(&mut self, id: InternalPointer) -> Option<WrappedInternalCallback> {
        self.tree.get(&id).cloned()
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn add<F>(&mut self, f: F) -> InternalPointer
    where
        F: InternalCallback + 'static,
    {
        self.id += 1;
        let id = self.id;
        let wrapped = WrappedInternalCallback::new(f);
        self.tree.insert(InternalPointer::from(id), wrapped);
        InternalPointer::from(id)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn add<F>(&mut self, f: F) -> InternalPointer
    where
        F: InternalCallback + Send + Sync + 'static,
    {
        self.id += 1;
        let id = self.id;
        let wrapped = WrappedInternalCallback::new(f);
        self.tree.insert(InternalPointer::from(id), wrapped);
        InternalPointer::from(id)
    }
}
