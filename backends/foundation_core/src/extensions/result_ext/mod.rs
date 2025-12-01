pub type Result<T, E> = std::result::Result<T, E>;
pub type BoxedError = Box<dyn std::error::Error + 'static>;
pub type SendableBoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub trait BoxedResult {
    fn into_boxed_error(self) -> BoxedError;
}

impl<E> BoxedResult for E
where
    E: std::error::Error + 'static,
{
    fn into_boxed_error(self) -> BoxedError {
        Box::new(self)
    }
}

pub trait SendableBoxedResult {
    fn into_sendable_boxed_error(self) -> SendableBoxedError;
}

impl<E> SendableBoxedResult for E
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_sendable_boxed_error(self) -> SendableBoxedError {
        Box::new(self)
    }
}
