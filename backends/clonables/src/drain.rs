pub trait Drain {
    fn drain(&mut self);
}

impl<T, I> Drain for T
where
    T: Iterator<Item = I> + Clone + Send + 'static,
{
    fn drain(&mut self) {
        for item in self {
            drop(item);
        }
    }
}
