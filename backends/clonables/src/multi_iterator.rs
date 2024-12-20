use crate::Drain;

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

/// MultiIterator is an iterator that can represent a type whoes output
/// can be both a singular item or multiple, this lets internal operation
/// be flexible to define the output Item type.
pub trait MultiIterator<T>: Iterator<Item = Multi<T>> {}
