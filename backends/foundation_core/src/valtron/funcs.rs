/// `CloneableFnMut` implements a cloning for your FnMut/Fn types
/// which allows you define a Fn/FnMut that can be owned and
/// Send as well without concerns on Sync.
/// This then allows you safely clone an Fn and send across threads easily.
pub trait CloneableFn<I, R>: Fn(I) -> R + Send {
    fn clone_box(&self) -> Box<dyn CloneableFn<I, R>>;
}

impl<F, I, R> CloneableFn<I, R> for F
where
    F: Fn(I) -> R + Send + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn CloneableFn<I, R>> {
        Box::new(self.clone())
    }
}

// // TODO(alex.ewetumo): investigate why this potential caused an explosion
// // of stack overflow.
// impl<I: 'static, R: 'static> Clone for Box<dyn CloneableFn<I, R>> {
//     fn clone(&self) -> Self {
//         self.clone_box()
//     }
// }

/// `WrappedCloneableFnMut` exists to provide for cases where the compiler
/// wants your implementing type for `CloneableFnMut` to also implement Clone.
pub struct WrappedCloneableFnMut<I, R>(Box<dyn CloneableFn<I, R>>);

impl<I, R> WrappedCloneableFnMut<I, R> {
    #[must_use]
    pub fn new(elem: Box<dyn CloneableFn<I, R>>) -> Self {
        Self(elem)
    }

    pub fn call(&mut self, input: I) -> R {
        (self.0)(input)
    }
}

/// After much research, it turns out the 'static lifetime is actually
/// implicit for all owned types. Box<T> is always equivalent to
/// Box<T + 'static>, since Box always owns its contents.
/// Lifetimes only apply to references in rust.
///
/// See <https://doc.rust-lang.org/rust-by-example/scope/lifetime/static_lifetime.html>.
impl<I: 'static, R: 'static> Clone for WrappedCloneableFnMut<I, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}
