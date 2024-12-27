/// ClonableFnMut implements a cloning for your FnMut/Fn types
/// which allows you define a Fn/FnMut that can be owned and
/// wholely Send as well without concerns on Sync.
/// This then allows you safely clone an Fn and send across threads easily.
pub trait ClonableFn<I, R>: Fn(I) -> R + Send {
    fn clone_box(&self) -> Box<dyn ClonableFn<I, R>>;
}

impl<F, I, R> ClonableFn<I, R> for F
where
    F: Fn(I) -> R + Send + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ClonableFn<I, R>> {
        Box::new(self.clone())
    }
}

// // TODO(alex.ewetumo): investigate why this potential caused an explosion
// // of stack overflow.
// impl<I: 'static, R: 'static> Clone for Box<dyn ClonableFn<I, R>> {
//     fn clone(&self) -> Self {
//         self.clone_box()
//     }
// }

/// WrappedClonableFnMut exists to provide for cases where the compiler
/// wants your implementing type for ClonableFnMut to also implement Clone.
pub struct WrappedClonableFnMut<I, R>(Box<dyn ClonableFn<I, R>>);

impl<I, R> WrappedClonableFnMut<I, R> {
    pub fn new(elem: Box<dyn ClonableFn<I, R>>) -> Self {
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
/// See https://doc.rust-lang.org/rust-by-example/scope/lifetime/static_lifetime.html.
impl<I: 'static, R: 'static> Clone for WrappedClonableFnMut<I, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}
