use super::types::AnyResult;

/// `CloneableBoxIterator` is a type definition for an Iterator that can safely be
/// sent across threads safely and easily. Requiring the underlying generic
/// type to be `Send` but not `Sync`.
///
/// This is intended for owned types where the receiving thread owns the object fully.
pub type CloneableBoxIterator<T, E> = Box<dyn CloneableIterator<Item = AnyResult<T, E>>>;

// Nice pre-defined types, feel free to define yours
pub type CloneableI8Iterator<E> = CloneableBoxIterator<i8, E>;
pub type CloneableU8Iterator<E> = CloneableBoxIterator<u8, E>;
pub type CloneableU16Iterator<E> = CloneableBoxIterator<u16, E>;
pub type CloneableU32Iterator<E> = CloneableBoxIterator<u32, E>;
pub type CloneableU64Iterator<E> = CloneableBoxIterator<u64, E>;
pub type CloneableI16Iterator<E> = CloneableBoxIterator<i16, E>;
pub type CloneableI32Iterator<E> = CloneableBoxIterator<i32, E>;
pub type CloneableI64Iterator<E> = CloneableBoxIterator<i64, E>;
pub type CloneableVecIterator<E> = CloneableBoxIterator<Vec<u8>, E>;
pub type CloneableStringIterator<E> = CloneableBoxIterator<String, E>;
pub type CloneableByteIterator<'a, E> = CloneableBoxIterator<&'a [u8], E>;

/// `CloneableIterator` defines a trait which requires the implementing type to
/// be Send and Cloneable this allows you to have a implementing type that can
/// safely be cloned and wholly send across a thread into another without having
/// to juggle the usual complainst of requiring the type to also be sync.
pub trait CloneableIterator: Iterator {
    fn clone_box_iterator(&self) -> Box<dyn CloneableIterator<Item = Self::Item>>;
}

/// `CloneableSendBoxIterator` is a type definition for an Iterator that can safely be
/// sent across threads safely and easily. Requiring the underlying generic
/// type to be `Send` but not `Sync`.
///
/// This is intended for owned types where the receiving thread owns the object fully.
pub type CloneableSendBoxIterator<T, E> = Box<dyn CloneableSendIterator<Item = AnyResult<T, E>>>;

/// `CloneableIterator` that can be Send
pub trait CloneableSendIterator: Iterator + Send {
    fn clone_box_send_iterator(&self) -> Box<dyn CloneableSendIterator<Item = Self::Item>>;
}

// Nice pre-defined types, feel free to define yours
pub type SendableCloneableI8Iterator<E> = CloneableSendBoxIterator<i8, E>;
pub type CloneableSendU8Iterator<E> = CloneableSendBoxIterator<u8, E>;
pub type CloneableSendU16Iterator<E> = CloneableSendBoxIterator<u16, E>;
pub type CloneableSendU32Iterator<E> = CloneableSendBoxIterator<u32, E>;
pub type CloneableSendU64Iterator<E> = CloneableSendBoxIterator<u64, E>;
pub type CloneableSendI16Iterator<E> = CloneableSendBoxIterator<i16, E>;
pub type CloneableSendI32Iterator<E> = CloneableSendBoxIterator<i32, E>;
pub type CloneableSendI64Iterator<E> = CloneableSendBoxIterator<i64, E>;
pub type SendableVecIterator<E> = CloneableSendBoxIterator<Vec<u8>, E>;
pub type CloneableSendStringIterator<E> = CloneableSendBoxIterator<String, E>;
pub type CloneableSendByteIterator<'a, E> = CloneableSendBoxIterator<&'a [u8], E>;

impl<T, I> CloneableIterator for T
where
    T: Iterator<Item = I> + Clone + 'static,
{
    fn clone_box_iterator(&self) -> Box<dyn CloneableIterator<Item = I>> {
        Box::new(self.clone())
    }
}

impl<T, I> CloneableSendIterator for T
where
    T: Iterator<Item = I> + Clone + Send + 'static,
{
    fn clone_box_send_iterator(&self) -> Box<dyn CloneableSendIterator<Item = I>> {
        Box::new(self.clone())
    }
}

impl<T: 'static> Clone for Box<dyn CloneableIterator<Item = T>> {
    fn clone(&self) -> Self {
        self.clone_box_iterator()
    }
}

/// `CanCloneIterator` provides a wrapper that lets you outrigt deal with
/// situations where the compiler wants your `ClonbableIterator` implementing
/// type to directly implement Clone.
pub struct CanCloneIterator<T>(Box<dyn CloneableIterator<Item = T>>);

impl<T> CanCloneIterator<T> {
    #[must_use] 
    pub fn new(elem: Box<dyn CloneableIterator<Item = T>>) -> Self {
        Self(elem)
    }
}

impl<T: 'static> Clone for CanCloneIterator<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box_iterator())
    }
}

impl<T> Iterator for CanCloneIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub struct CanCloneSendIterator<T>(Box<dyn CloneableSendIterator<Item = T>>);

impl<T> CanCloneSendIterator<T> {
    #[must_use] 
    pub fn new(elem: Box<dyn CloneableSendIterator<Item = T>>) -> Self {
        Self(elem)
    }
}

impl<T: 'static> Clone for CanCloneSendIterator<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box_send_iterator())
    }
}

impl<T> Iterator for CanCloneSendIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// [`BoxedIterator`] defines a type alias for a boxed iterator that always returns a object of type
/// [`T`].
pub type BoxedIterator<T> = Box<dyn Iterator<Item = T>>;

/// [`BoxedResultIterator`] defines a type alias for a boxed iterator that always returns a Result object.
pub type BoxedResultIterator<T, E> = BoxedIterator<AnyResult<T, E>>;

/// Boxed iterator of Strings.
pub type StringBoxedIterator<E> = BoxedResultIterator<String, E>;

/// `SendableIterator` that can be Send and implements iterator.
pub trait SendableIterator<T>: Iterator<Item = T> + Send {}

pub type SendableBoxIterator<T, E> = Box<dyn SendableIterator<AnyResult<T, E>>>;

// Nice pre-defined types, feel free to define yours
pub type SendableI8Iterator<E> = SendableBoxIterator<i8, E>;
pub type SendU8Iterator<E> = SendableBoxIterator<u8, E>;
pub type SendU16Iterator<E> = SendableBoxIterator<u16, E>;
pub type SendU32Iterator<E> = SendableBoxIterator<u32, E>;
pub type SendU64Iterator<E> = SendableBoxIterator<u64, E>;
pub type SendI16Iterator<E> = SendableBoxIterator<i16, E>;
pub type SendI32Iterator<E> = SendableBoxIterator<i32, E>;
pub type SendI64Iterator<E> = SendableBoxIterator<i64, E>;
pub type SendVecIterator<E> = SendableBoxIterator<Vec<u8>, E>;
pub type SendStringIterator<E> = SendableBoxIterator<String, E>;
pub type SendByteIterator<'a, E> = SendableBoxIterator<&'a [u8], E>;

// impl<T, I> SendableIterator<I> for T where T: Box<dyn SendableIterator<T> + Send + 'static> {}

pub struct TransformSendIterator<T: Send, V: Send> {
    transformer: Box<dyn Fn(T) -> Option<V> + Send + 'static>,
    source: Box<dyn SendableIterator<T>>,
}

impl<T: Send, V: Send> TransformSendIterator<T, V> {
    #[must_use] 
    pub fn new(
        tn: Box<dyn Fn(T) -> Option<V> + Send + 'static>,
        source: Box<dyn SendableIterator<T>>,
    ) -> Self {
        Self {
            transformer: tn,
            source,
        }
    }
}

impl<T: Send, V: Send> SendableIterator<V> for TransformSendIterator<T, V> {}

impl<T: Send, V: Send> Iterator for TransformSendIterator<T, V> {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        match self.source.next() {
            Some(item) => (self.transformer)(item),
            _ => None,
        }
    }
}

pub struct TransformIterator<T, V> {
    transformer: Box<dyn Fn(T) -> Option<V>>,
    source: BoxedIterator<T>,
}

impl<T, V> TransformIterator<T, V> {
    #[must_use] 
    pub fn new(tn: Box<dyn Fn(T) -> Option<V>>, source: BoxedIterator<T>) -> Self {
        Self {
            transformer: tn,
            source,
        }
    }
}

impl<T, V> Iterator for TransformIterator<T, V> {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        match self.source.next() {
            Some(item) => (self.transformer)(item),
            _ => None,
        }
    }
}
