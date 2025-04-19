use alloc::{boxed::Box, collections::btree_map::BTreeMap, rc::Rc};

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
    tree: BTreeMap<u64, Rc<Box<dyn InternalCallback>>>,
    id: u64,
}

impl Default for InternalReferenceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl InternalReferenceRegistry {
    pub fn new() -> Self {
        Self {
            id: 0,
            tree: BTreeMap::new(),
        }
    }

    pub fn remove(&mut self, id: u64) -> Option<Rc<Box<dyn InternalCallback + 'static>>> {
        self.tree.remove(&id)
    }

    pub fn get(&mut self, id: u64) -> Option<Rc<Box<dyn InternalCallback + 'static>>> {
        self.tree.get(&id).cloned()
    }

    pub fn add<F>(&mut self, f: F) -> u64
    where
        F: InternalCallback + 'static,
    {
        self.id += 1;
        let id = self.id;
        self.tree.insert(id, Rc::new(Box::new(f)));
        id
    }
}
