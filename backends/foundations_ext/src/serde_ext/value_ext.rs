use serde::de::DeserializeOwned;
use serde::Serialize;

use super::ValueError;

///  Extension trait that allows us define a conversion of a JSON value to
/// more native rust types.
///
/// It's implemented for bool, u32/64, f64, i32/64 and str
pub trait AsType<'a, T>: Sized {
    fn from_value(value: &'a T) -> ValueResult<Self, ValueError>;
}

pub type ValueResult<T, E> = core::result::Result<T, E>;

/// PointerValueExt defines method to pull value from an underlying value.
pub trait PointerValueExt {
    type Item;
    type Error;

    // get_path returns a reference to the underlying type identified by `PointerValueExt::Item`.
    fn get_path(&self, name_or_pointer: &str) -> ValueResult<&Self::Item, Self::Error>;

    // get_path returns a reference to the underlying type identified by `PointerValueExt::Item`.
    fn take_path(&mut self, name_or_pointer: &str) -> ValueResult<Self::Item, Self::Error>;
}

/// DynamicValueExt defines a core expectation for dynamically generated
/// value containers such as those in json, toml or yaml.
/// This trait is focused on making interactions directly with these types
/// easily and affordable without specifically dragging then need to serialize
/// into specific type. This brings the joy of Data Oriented programming
/// to rust.
pub trait DynamicValueExt {
    type Item;
    type Error;

    fn d_new() -> Self::Item;

    /// Returns an owned type `T` for a given name or pointer path.
    /// - `name_or_pointer`: Can be a direct name or a pointer path (path starting with `/`),
    fn d_get<T: DeserializeOwned>(&self, name_or_pointer: &str) -> ValueResult<T, Self::Error>;

    /// Returns an reference of type `T` (or value for copy type) for a given name or pointer path.
    /// - `name_or_pointer`: Can be a direct name or a pointer path (path starting with `/`),
    fn d_get_as<'a, V: AsType<'a, Self::Item>>(
        &'a self,
        name_or_pointer: &str,
    ) -> ValueResult<V, Self::Error>;

    /// Returns an owned type `T` for a given name or pointer path replacing with `Null`.
    /// - `name_or_pointer`: Can be a direct name or a pointer path (path starting with `/`),
    fn d_take<T: DeserializeOwned>(&mut self, name_or_pointer: &str)
        -> ValueResult<T, Self::Error>;

    /// Inserts a new value of type `T` at the specified name or pointer path.
    /// It creates a missing `Value::Object` entries as needed.
    fn d_insert<T: Serialize>(
        &mut self,
        name_or_pointer: &str,
        value: T,
    ) -> ValueResult<(), Self::Error>;

    /// Returns a pretty-printed string representation of the JSON value.
    fn d_pretty(&self) -> ValueResult<String, Self::Error>;
}
