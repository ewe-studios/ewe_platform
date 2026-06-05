/// Marker for methods that should be delegated by `#[scaffold_impl]`.
///
/// Used inside `impl` blocks annotated with `#[scaffold_impl]` to mark
/// methods that should have their body replaced with delegation code.
///
/// # Example
///
/// ```ignore
/// use foundation_nostd::macros::scaffold;
///
/// #[scaffold_impl(via = "self.inner")]
/// impl MyTrait for Wrapper {
///     fn method(&self, x: u32) -> String { scaffold!() }
/// }
/// ```
///
/// If the proc macro hasn't processed the impl block, this compiles to
/// `unreachable!()`, ensuring a clear panic instead of silent misbehavior.
#[macro_export]
macro_rules! scaffold {
    () => {
        unreachable!("scaffold!() was not processed by #[scaffold_impl] — did you forget the attribute on the impl block?")
    };
}

/// Re-export so users can write `use foundation_macros::scaffold;`
/// (the proc-macro crate re-exports this)
pub use crate::scaffold;
