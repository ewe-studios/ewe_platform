use super::types::AnyResult;
use crate::io::readers::Data;

pub use super::streams::Stream;

/// `CloneableBoxIterator` is a type definition for an Iterator that can safely be
/// sent across threads safely and easily. Requiring the underlying generic
/// type to be `Send` but not `Sync`.
///
/// This is intended for owned types where the receiving thread owns the object fully.
pub type CloneableBoxIterator<T, E> = Box<dyn CloneableIterator<Item = AnyResult<T, E>>>;

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
pub trait CloneableSendIterator: Iterator + Send + Sync {
    fn clone_box_send_iterator(&self) -> Box<dyn CloneableSendIterator<Item = Self::Item>>;
}

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
    T: Iterator<Item = I> + Clone + Send + Sync + 'static,
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
/// `T`.
pub type BoxedIterator<T> = Box<dyn Iterator<Item = T>>;

/// [`BoxedSendIterator`] defines a type alias for a boxed iterator that always returns a object of type
/// `T`.
pub type BoxedSendIterator<T> = Box<dyn Iterator<Item = T> + Send + Sync>;

/// [`BoxedSendableIterator`] defines a type which is an iterator that can also be Send.
pub type BoxedSendableIterator<T, E> = BoxedSendIterator<AnyResult<T, E>>;

/// [`BoxedSendableVecIterator`] defines a sendable boxed iterator that can be Send.
pub type BoxedSendableVecIterator<E> = BoxedSendableIterator<Vec<u8>, E>;

/// [`BoxedSendableDataIterator`] defines a sendable boxed iterator yielding `Data`.
pub type BoxedSendableDataIterator<E> = BoxedSendableIterator<Data, E>;

/// [`BoxedResultIterator`] defines a type alias for a boxed iterator that always returns a Result object.
pub type BoxedResultIterator<T, E> = BoxedIterator<AnyResult<T, E>>;

/// Boxed iterator of Strings.
pub type StringBoxedIterator<E> = BoxedResultIterator<String, E>;

/// Boxed iterator of Vecs.
pub type VecBoxedIterator<E> = BoxedResultIterator<Vec<u8>, E>;

/// `SendableIterator` that can be Send and implements iterator.
pub trait SendableIterator<T>: Iterator<Item = T> + Send {}

pub type SendableBoxIterator<T, E> = Box<dyn SendableIterator<AnyResult<T, E>>>;

// Only commonly-used concrete type aliases; define your own as needed
pub type SendVecIterator<E> = SendableBoxIterator<Vec<u8>, E>;

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

    /// Returns the next transformed item.
    ///
    /// Uses filter_map semantics: if the transformer returns None, the item
    /// is skipped and the next source item is attempted. This continues
    /// until either the transformer returns Some(transformed) or the source
    /// is exhausted.
    ///
    /// NOTE: This iterator processes items in a loop internally which is
    /// acceptable for this specific use case. Callers should be aware that
    /// next() may process multiple source items before returning.
    fn next(&mut self) -> Option<Self::Item> {
        for item in self.source.by_ref() {
            if let Some(transformed) = (self.transformer)(item) {
                return Some(transformed);
            }
            // transformer returned None - skip this item, try next
        }
        None
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

    /// Returns the next transformed item.
    ///
    /// Uses filter_map semantics: if the transformer returns None, the item
    /// is skipped and the next source item is attempted. This continues
    /// until either the transformer returns Some(transformed) or the source
    /// is exhausted.
    ///
    /// NOTE: This iterator processes items in a loop internally which is
    /// acceptable for this specific use case. Callers should be aware that
    /// next() may process multiple source items before returning.
    fn next(&mut self) -> Option<Self::Item> {
        for item in self.source.by_ref() {
            if let Some(transformed) = (self.transformer)(item) {
                return Some(transformed);
            }
            // transformer returned None - skip this item, try next
        }
        None
    }
}

/// TransformUntilIterator - preserves take_while-on-None behavior.
///
/// Unlike TransformIterator which skips None values (filter_map semantics),
/// TransformUntilIterator stops iterating when the transformer returns None.
/// This preserves the old behavior for callers that need it.
pub struct TransformUntilIterator<T, V> {
    transformer: Box<dyn Fn(T) -> Option<V>>,
    source: BoxedIterator<T>,
}

impl<T, V> TransformUntilIterator<T, V> {
    #[must_use]
    pub fn new(tn: Box<dyn Fn(T) -> Option<V>>, source: BoxedIterator<T>) -> Self {
        Self {
            transformer: tn,
            source,
        }
    }
}

impl<T, V> Iterator for TransformUntilIterator<T, V> {
    type Item = V;

    /// Returns the next transformed item.
    ///
    /// Uses take_while semantics: when transformer returns None, iterator
    /// terminates (returns None). This is the old TransformIterator behavior.
    fn next(&mut self) -> Option<Self::Item> {
        match self.source.next() {
            Some(item) => (self.transformer)(item),
            None => None,
        }
    }
}

/// TransformUntilSendIterator - Send variant preserving take_while-on-None behavior.
///
/// Unlike TransformSendIterator which skips None values (filter_map semantics),
/// TransformUntilSendIterator stops iterating when the transformer returns None.
pub struct TransformUntilSendIterator<T: Send, V: Send> {
    transformer: Box<dyn Fn(T) -> Option<V> + Send + 'static>,
    source: Box<dyn SendableIterator<T>>,
}

impl<T: Send, V: Send> TransformUntilSendIterator<T, V> {
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

impl<T: Send, V: Send> SendableIterator<V> for TransformUntilSendIterator<T, V> {}

impl<T: Send, V: Send> Iterator for TransformUntilSendIterator<T, V> {
    type Item = V;

    /// Returns the next transformed item.
    ///
    /// Uses take_while semantics: when transformer returns None, iterator
    /// terminates (returns None). This is the old TransformSendIterator behavior.
    fn next(&mut self) -> Option<Self::Item> {
        match self.source.next() {
            Some(item) => (self.transformer)(item),
            None => None,
        }
    }
}
