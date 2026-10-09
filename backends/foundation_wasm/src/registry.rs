use alloc::{boxed::Box, collections::btree_map::BTreeMap};

use foundation_nostd::comp::basic::Mutex;

use crate::{InternalPointer, ReturnTypeHints, Returns, TaskResult, WrappedItem};

#[cfg(target_family = "wasm")]
pub trait InternalCallback {
    fn receive(&self, value: TaskResult<Returns>);
}

#[cfg(not(target_family = "wasm"))]
pub trait InternalCallback: Send + Sync {
    fn receive(&self, value: TaskResult<Returns>);
}

#[cfg(target_family = "wasm")]
pub struct FnCallback(Box<dyn Fn(TaskResult<Returns>)>);

#[cfg(not(target_family = "wasm"))]
pub struct FnCallback(Mutex<Box<dyn Fn(TaskResult<Returns>) + Send + 'static>>);

impl FnCallback {
    #[cfg(not(target_family = "wasm"))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn(TaskResult<Returns>) + Send + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(target_family = "wasm")]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn(TaskResult<Returns>) + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(not(target_family = "wasm"))]
    #[must_use]
    pub fn new(elem: Box<dyn Fn(TaskResult<Returns>) + Send + 'static>) -> Self {
        Self(Mutex::new(elem))
    }

    #[cfg(target_family = "wasm")]
    pub fn new(elem: Box<dyn Fn(TaskResult<Returns>)>) -> Self {
        Self(elem)
    }
}

#[cfg(target_family = "wasm")]
unsafe impl Sync for FnCallback {}

#[cfg(target_family = "wasm")]
unsafe impl Send for FnCallback {}

impl InternalCallback for FnCallback {
    fn receive(&self, value: TaskResult<Returns>) {
        #[cfg(not(target_family = "wasm"))]
        {
            (self
                .0
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner))(
                value
            );
        }

        #[cfg(target_family = "wasm")]
        {
            (self.0)(value);
        }
    }
}

#[cfg(target_family = "wasm")]
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

#[cfg(not(target_family = "wasm"))]
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
impl Default for InternalReferenceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InternalReferenceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: 0,
            tree: BTreeMap::new(),
        }
    }
}

impl InternalReferenceRegistry {
    #[must_use]
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

    #[must_use]
    pub fn get_type(&self, id: InternalPointer) -> Option<ReturnTypeHints> {
        self.tree.get(&id).map(|(hint, _)| hint.clone())
    }
}

#[cfg(target_family = "wasm")]
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

#[cfg(not(target_family = "wasm"))]
impl InternalReferenceRegistry {
    #[must_use]
    pub fn call(&self, id: InternalPointer, values: TaskResult<Returns>) -> Option<()> {
        if let Some((_, callback)) = self.tree.get(&id) {
            callback
                .0
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                .receive(values);
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
