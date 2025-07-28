pub type Result<T, E> = std::result::Result<T, E>;
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

pub trait BoxedResult {
    fn into_boxed_error(self) -> BoxedError;
}

impl<E> BoxedResult for E
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_boxed_error(self) -> BoxedError {
        Box::new(self)
    }
}
