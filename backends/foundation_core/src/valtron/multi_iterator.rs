/// Multi provides a state enum that can specify if the
/// underlying returned content is a list or not. This lets us
/// to be able to define an iterator that can represent both
/// and present results that are `Multi::One` or `Multi::Many`
/// values when we wish to send out multiple value types.
///
/// For now, many always owns a owned `Vec` type.
pub enum Multi<T> {
    One(T),
    Many(Vec<T>),
}

/// [`AsMultiIterator`] defines a custom iterator type that is
/// a blanket definition of a type that both implements an iterator.
///
/// Most might prefer to use this plain type that specifies indirectly
/// the required iterator item via a super-trait dependency
/// else directly from the [`MultiIterator`] trait definition which is
/// more custom and requires wrapping via it's `MultiIterator::into_iter()`
/// method.
pub trait AsMultiIterator<T>: Iterator<Item = Multi<T>> {}

/// MultiIterator is an iterator that can represent a type whose output
/// can be both a singular item or multiple, this lets internal operation
/// be flexible to define the output Item type.
pub trait MultiIterator {
    type Item;

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<Multi<Self::Item>>;

    /// into_iter consumes the implementation and wraps
    /// it in an iterator type that emits [`Multi<MultiIterator::Item>`]
    /// match the behavior desired for an iterator.
    fn into_iter(self) -> impl Iterator<Item = Multi<Self::Item>>
    where
        Self: Sized + 'static,
    {
        MultiAsIterator(Box::new(self))
    }
}

pub struct MultiAsIterator<T>(Box<dyn MultiIterator<Item = T>>);

impl<T> MultiAsIterator<T> {
    pub fn from_impl(t: impl MultiIterator<Item = T> + 'static) -> Self {
        Self(Box::new(t))
    }

    pub fn new(t: Box<dyn MultiIterator<Item = T>>) -> Self {
        Self(t)
    }
}

impl<T> AsMultiIterator<T> for MultiAsIterator<T> {}

impl<T> Iterator for MultiAsIterator<T> {
    type Item = Multi<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
