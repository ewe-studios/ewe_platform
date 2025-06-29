use alloc::{boxed::Box, collections::btree_map::BTreeMap};

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
use alloc::rc::Rc;

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
use alloc::sync::Arc;

use foundation_nostd::spin::Mutex;

use crate::{InternalPointer, ReturnTypeHints, ReturnValues, Returns};

pub type TaskResult<T> = core::result::Result<T, TaskErrorCode>;

/// [`TaskErrorCode`] represents the converted response of an
/// [`ReturnValues::ErrorCode`] when its communicated that a async task
/// or function failed.
///
/// Usually when the only response is [`ReturnValues::ErrorCode`] when
/// the response hint provided did not match that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskErrorCode(pub u16);

impl TaskErrorCode {
    pub fn new(code: u16) -> Self {
        Self(code)
    }
}

impl From<u16> for TaskErrorCode {
    fn from(code: u16) -> Self {
        Self(code)
    }
}

impl From<ReturnValues> for TaskErrorCode {
    fn from(value: ReturnValues) -> Self {
        match &value {
            ReturnValues::ErrorCode(code) => {
                Self(*code)
            }
            _ => unreachable!("We should never attempt to convert anything but a ReturnValues::ErrorCode to a TaskErrorCode. This is a bug in the runtime code. Please report it.")
        }
    }
}

impl core::error::Error for TaskErrorCode {}

impl core::fmt::Display for TaskErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub trait InternalCallback {
    fn receive(&self, value: TaskResult<Returns>);
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub trait InternalCallback: Send + Sync {
    fn receive(&self, value: TaskResult<Returns>);
}

#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
pub struct WrappedInternalCallback(Arc<Mutex<Box<dyn InternalCallback + 'static>>>);

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub struct WrappedInternalCallback(Rc<Box<dyn InternalCallback + 'static>>);

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Sync for WrappedInternalCallback {}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
unsafe impl Send for WrappedInternalCallback {}

impl WrappedInternalCallback {
    pub fn new<F>(f: F) -> Self
    where
        F: InternalCallback + 'static,
    {
        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        {
            Self(Rc::new(Box::new(f)))
        }

        #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
        {
            Self(Arc::new(Mutex::new(Box::new(f))))
        }
    }
}

impl Clone for WrappedInternalCallback {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl InternalCallback for WrappedInternalCallback {
    fn receive(&self, value: TaskResult<Returns>) {
        #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
        {
            self.0.lock().receive(value);
        }

        #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
        {
            self.0.receive(value);
        }
    }
}

pub type CallbackFn = dyn Fn(*const u8, u64) + 'static;

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

pub struct InternalReferenceRegistry {
    tree: BTreeMap<InternalPointer, (ReturnTypeHints, WrappedInternalCallback)>,
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
    pub fn remove(
        &mut self,
        id: InternalPointer,
    ) -> Option<(ReturnTypeHints, WrappedInternalCallback)> {
        self.tree.remove(&id)
    }

    pub fn get(
        &mut self,
        id: InternalPointer,
    ) -> Option<(ReturnTypeHints, WrappedInternalCallback)> {
        self.tree.get(&id).cloned()
    }

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn add<F>(&mut self, returns: ReturnTypeHints, f: F) -> InternalPointer
    where
        F: InternalCallback + 'static,
    {
        self.id += 1;
        let id = self.id;
        let wrapped = WrappedInternalCallback::new(f);
        self.tree
            .insert(InternalPointer::from(id), (returns, wrapped));
        InternalPointer::from(id)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn add<F>(&mut self, returns: ReturnTypeHints, f: F) -> InternalPointer
    where
        F: InternalCallback + Send + Sync + 'static,
    {
        self.id += 1;
        let id = self.id;
        let wrapped = WrappedInternalCallback::new(f);
        self.tree
            .insert(InternalPointer::from(id), (returns, wrapped));
        InternalPointer::from(id)
    }
}
