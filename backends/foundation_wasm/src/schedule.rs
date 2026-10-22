use alloc::{boxed::Box, collections::btree_map::BTreeMap};

use crate::{InternalPointer, WrappedItem};
use foundation_nostd::comp::basic::Mutex;

#[cfg(target_family = "wasm")]
pub trait DoTask {
    fn perform(&self);
}

#[cfg(not(target_family = "wasm"))]
pub trait DoTask: Send + Sync {
    fn perform(&self);
}

#[cfg(target_family = "wasm")]
pub struct FnDoTask(Box<dyn Fn()>);

#[cfg(not(target_family = "wasm"))]
pub struct FnDoTask(Mutex<Box<dyn Fn() + Send + 'static>>);

impl FnDoTask {
    #[cfg(not(target_family = "wasm"))]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn() + Send + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(target_family = "wasm")]
    pub fn from<F>(elem: F) -> Self
    where
        F: Fn() + 'static,
    {
        Self::new(Box::new(elem))
    }

    #[cfg(not(target_family = "wasm"))]
    #[must_use]
    pub fn new(elem: Box<dyn Fn() + Send + 'static>) -> Self {
        Self(Mutex::new(elem))
    }

    #[cfg(target_family = "wasm")]
    pub fn new(elem: Box<dyn Fn()>) -> Self {
        Self(elem)
    }
}

#[cfg(target_family = "wasm")]
unsafe impl Sync for FnDoTask {}

#[cfg(target_family = "wasm")]
unsafe impl Send for FnDoTask {}

impl DoTask for FnDoTask {
    fn perform(&self) {
        #[cfg(not(target_family = "wasm"))]
        {
            (self
                .0
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner))(
            );
        }

        #[cfg(target_family = "wasm")]
        {
            (self.0)()
        }
    }
}

#[cfg(target_family = "wasm")]
pub struct ScheduleRegistry {
    tree: BTreeMap<InternalPointer, WrappedItem<Box<dyn DoTask + 'static>>>,
    id: u64,
}

#[cfg(not(target_family = "wasm"))]
pub struct ScheduleRegistry {
    tree: BTreeMap<InternalPointer, WrappedItem<Box<dyn DoTask + Sync + Send + 'static>>>,
    id: u64,
}

// -- Constructors

#[allow(unused)]
impl Default for ScheduleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ScheduleRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: 0,
            tree: BTreeMap::new(),
        }
    }
}

impl ScheduleRegistry {
    #[must_use]
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

    /// Remove a one-shot callback and hand it back so the caller can run it
    /// AFTER releasing the registry lock.
    ///
    /// WHY: a scheduled callback commonly schedules again (valtron's JS yield
    /// loop, `testing::run_async`'s re-poll). Running it while the registry
    /// mutex is held re-locks the same mutex, which panics or deadlocks; and a
    /// timeout/microtask fires once, so its entry must not outlive the call.
    pub fn take(&mut self, id: InternalPointer) -> Option<ScheduledItem> {
        self.tree.remove(&id)
    }
}

/// A callback removed from the [`ScheduleRegistry`] by [`ScheduleRegistry::take`].
#[cfg(target_family = "wasm")]
pub type ScheduledItem = WrappedItem<Box<dyn DoTask + 'static>>;

/// A callback removed from the [`ScheduleRegistry`] by [`ScheduleRegistry::take`].
#[cfg(not(target_family = "wasm"))]
pub type ScheduledItem = WrappedItem<Box<dyn DoTask + Sync + Send + 'static>>;

/// Run a callback previously removed with [`ScheduleRegistry::take`].
pub fn perform_scheduled(item: &ScheduledItem) {
    #[cfg(target_family = "wasm")]
    {
        item.0.perform();
    }
    #[cfg(not(target_family = "wasm"))]
    {
        item.0
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .perform();
    }
}

#[cfg(target_family = "wasm")]
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

#[cfg(not(target_family = "wasm"))]
impl ScheduleRegistry {
    #[must_use]
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    #[must_use]
    pub fn call(&self, id: InternalPointer) -> Option<()> {
        if let Some(callback) = self.tree.get(&id) {
            callback
                .0
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                .perform();
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
