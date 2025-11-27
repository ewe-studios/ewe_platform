use alloc::{boxed::Box, collections::btree_map::BTreeMap};

use foundation_nostd::spin::Mutex;

use crate::{InternalPointer, ReturnTypeHints, Returns, TaskResult, WrappedItem};

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub trait InternalCallback {
    fn receive(&self, value: TaskResult<Returns>);
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub trait InternalCallback: Send + Sync {
    fn receive(&self, value: TaskResult<Returns>);
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct FnCallback(Box<dyn Fn(TaskResult<Returns>)>);

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct FnCallback(Mutex<Box<dyn Fn(TaskResult<Returns>) + Send + 'static>>);

impl FnCallback {
    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn(TaskResult<Returns>) + Send + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn(TaskResult<Returns>) + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn new(elem: Box<dyn Fn(TaskResult<Returns>) + Send + 'static>) -> Self {
        Self(Mutex::new(elem))
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn new(elem: Box<dyn Fn(TaskResult<Returns>)>) -> Self {
        Self(elem)
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Sync for FnCallback {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for FnCallback {}

impl InternalCallback for FnCallback {
    fn receive(&self, value: TaskResult<Returns>) {
        #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
        {
            (self.0.lock())(value);
        }

        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        {
            (self.0)(value);
        }
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct InternalReferenceRegistry {
    tree: BTreeMap<
        InternalPointer,
        (
            ReturnTypeHints,
            WrappedItem<Box<dyn InternalCallback + 'static>>,
        ),
    >,
    id: u64,
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct InternalReferenceRegistry {
    tree: BTreeMap<
        InternalPointer,
        (
            ReturnTypeHints,
            WrappedItem<Box<dyn InternalCallback + Sync + Send + 'static>>,
        ),
    >,
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
    pub fn delete(&mut self, id: InternalPointer) -> Option<ReturnTypeHints> {
        self.tree.remove(&id).map(|(hint, _)| hint)
    }

    pub fn get_type(&self, id: InternalPointer) -> Option<ReturnTypeHints> {
        self.tree.get(&id).map(|(hint, _)| hint.clone())
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
impl InternalReferenceRegistry {
    pub fn call(&self, id: InternalPointer, values: TaskResult<Returns>) -> Option<()> {
        if let Some((_, callback)) = self.tree.get(&id) {
            callback.0.receive(values);
            return Some(());
        }
        None
    }

    pub fn add(
        &mut self,
        returns: ReturnTypeHints,
        callback: Box<dyn InternalCallback + 'static>,
    ) -> InternalPointer {
        self.id += 1;
        let id = self.id;
        let wrapped = WrappedItem::new(callback);
        self.tree
            .insert(InternalPointer::from(id), (returns, wrapped));
        InternalPointer::from(id)
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
impl InternalReferenceRegistry {
    pub fn call(&self, id: InternalPointer, values: TaskResult<Returns>) -> Option<()> {
        if let Some((_, callback)) = self.tree.get(&id) {
            callback.0.lock().receive(values);
            return Some(());
        }
        None
    }

    pub fn add(
        &mut self,
        returns: ReturnTypeHints,
        callback: Box<dyn InternalCallback + Send + Sync + 'static>,
    ) -> InternalPointer {
        self.id += 1;
        let id = self.id;
        let wrapped = WrappedItem::new(callback);
        self.tree
            .insert(InternalPointer::from(id), (returns, wrapped));
        InternalPointer::from(id)
    }
}
