use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc};

use crate::InternalPointer;

pub trait InternalCallback {
    fn receive(&self, start_pointer: *const u8, length: u64);
}

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
    tree: BTreeMap<InternalPointer, Arc<Box<dyn InternalCallback + Send + Sync + 'static>>>,
    id: u64,
}

impl Default for InternalReferenceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InternalReferenceRegistry {
    pub const fn create() -> Self {
        Self {
            id: 0,
            tree: BTreeMap::new(),
        }
    }
    pub fn new() -> Self {
        Self {
            id: 0,
            tree: BTreeMap::new(),
        }
    }

    pub fn remove(
        &mut self,
        id: InternalPointer,
    ) -> Option<Arc<Box<dyn InternalCallback + Sync + Send + 'static>>> {
        self.tree.remove(&id)
    }

    pub fn get(
        &mut self,
        id: InternalPointer,
    ) -> Option<Arc<Box<dyn InternalCallback + Sync + Send + 'static>>> {
        self.tree.get(&id).cloned()
    }

    pub fn add<F>(&mut self, f: F) -> InternalPointer
    where
        F: InternalCallback + Send + Sync + 'static,
    {
        self.id += 1;
        let id = self.id;
        self.tree
            .insert(InternalPointer::from(id), Arc::new(Box::new(f)));
        InternalPointer::from(id)
    }
}
