use crate::types::AnyResult;

/// ClonableBoxIterator is a type definition for an Iterator that can safely be
/// sent across threads safely and easily. Requring the underlying generic
/// type to be `Send` but not `Sync`.
///
/// This is intended for owned types where the receiving thread owns the object fully.
pub type ClonableBoxIterator<T, E> = Box<dyn ClonableIterator<Item = AnyResult<T, E>>>;

// Nice pre-defined types, feel free to define yours
pub type ClonableI8Iterator<E> = ClonableBoxIterator<i8, E>;
pub type ClonableU8Iterator<E> = ClonableBoxIterator<u8, E>;
pub type ClonableU16Iterator<E> = ClonableBoxIterator<u16, E>;
pub type ClonableU32Iterator<E> = ClonableBoxIterator<u32, E>;
pub type ClonableU64Iterator<E> = ClonableBoxIterator<u64, E>;
pub type ClonableI16Iterator<E> = ClonableBoxIterator<i16, E>;
pub type ClonableI32Iterator<E> = ClonableBoxIterator<i32, E>;
pub type ClonableI64Iterator<E> = ClonableBoxIterator<i64, E>;
pub type ClonableVecIterator<E> = ClonableBoxIterator<Vec<u8>, E>;
pub type ClonableStringIterator<E> = ClonableBoxIterator<String, E>;
pub type ClonableByteIterator<'a, E> = ClonableBoxIterator<&'a [u8], E>;

/// ClonableIterator defines a trait which requires the implementing type to
/// be Send and Clonable this allows you to have a implementing type that can
/// safely be cloned and wholely send across a thread into another without having
/// to juggle the usual complainst of requiring the type to also be sync.
pub trait ClonableIterator: Iterator {
    fn clone_box(&self) -> Box<dyn ClonableIterator<Item = Self::Item>>;
}

impl<T, I> ClonableIterator for T
where
    T: Iterator<Item = I> + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ClonableIterator<Item = I>> {
        Box::new(self.clone())
    }
}

impl<T: 'static> Clone for Box<dyn ClonableIterator<Item = T>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// CanCloneIterator provides a wrapper that lets you outrigt deal with
/// situations where the compiler wants your ClonbableIterator implementing
/// type to directly implement Clone.
pub struct CanCloneIterator<T>(Box<dyn ClonableIterator<Item = T>>);

impl<T> CanCloneIterator<T> {
    pub fn new(elem: Box<dyn ClonableIterator<Item = T>>) -> Self {
        Self(elem)
    }
}

impl<T: 'static> Clone for CanCloneIterator<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<T> Iterator for CanCloneIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
