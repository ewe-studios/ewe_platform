pub type BoxedError = Box<dyn std::error::Error + Send>;

pub type AnyResult<T, E> = std::result::Result<T, E>;

pub type GenericResult<T> = AnyResult<T, BoxedError>;

/// ClonableBoxIterator is a type definition for an Iterator that can safely be
/// sent across threads safely and easily. Requring the underlying generic
/// type to be `Send` but not `Sync`.
///
/// This is intended for owned types where the receiving thread owns the object fully.
pub type ClonableBoxIterator<T, E> = Box<dyn ClonableIterator<Item = AnyResult<T, E>> + Send>;

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
pub trait ClonableIterator: Iterator + Send {
    fn clone_box(&self) -> Box<dyn ClonableIterator<Item = Self::Item>>;
}

impl<T, I> ClonableIterator for T
where
    T: Iterator<Item = I> + Clone + Send + 'static,
{
    fn clone_box(&self) -> Box<dyn ClonableIterator<Item = I>> {
        Box::new(self.clone())
    }
}

impl<T: Send + 'static> Clone for Box<dyn ClonableIterator<Item = T>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// WrappedIterator provides a wrapper that lets you outrigt deal with
/// situations where the compiler wants your ClonbableIterator implementing
/// type to directly implement Clone.
pub struct WrappedIterator<T>(Box<dyn ClonableIterator<Item = T>>);

impl<T> WrappedIterator<T> {
    pub fn new(elem: Box<dyn ClonableIterator<Item = T>>) -> Self {
        Self(elem)
    }
}

impl<T> Clone for WrappedIterator<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<T> Iterator for WrappedIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// ClonableFnMut implements a cloning for your FnMut/Fn types
/// which allows you define a Fn/FnMut that can be owned and
/// wholely Send as well without concerns on Sync.
/// This then allows you safely clone an Fn and send across threads easily.
pub trait ClonableFnMut<I, R>: FnMut(I) -> R + Send {
    fn clone_box(&self) -> Box<dyn ClonableFnMut<I, R>>;
}

impl<F, I, R> ClonableFnMut<I, R> for F
where
    F: FnMut(I) -> R + Send + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ClonableFnMut<I, R>> {
        Box::new(self.clone())
    }
}

impl<I: 'static, R: 'static> Clone for Box<dyn ClonableFnMut<I, R>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// WrappedClonableFnMut exists to provide for cases where the compiler
/// wants your implementing type for ClonableFnMut to also implement Clone.
pub struct WrappedClonableFnMut<I, R>(Box<dyn ClonableFnMut<I, R>>);

impl<I, R> WrappedClonableFnMut<I, R> {
    pub fn new(elem: Box<dyn ClonableFnMut<I, R>>) -> Self {
        Self(elem)
    }
}

/// After much research, it turns out the 'static lifetime is actually
/// implicit for all owned types. Box<T> is always equivalent to
/// Box<T + 'static>, since Box always owns its contents.
/// Lifetimes only apply to references in rust.
///
/// See https://doc.rust-lang.org/rust-by-example/scope/lifetime/static_lifetime.html.
impl<I: 'static, R: 'static> Clone for WrappedClonableFnMut<I, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}
