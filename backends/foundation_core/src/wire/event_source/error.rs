pub(crate) type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub(crate) type GenericResult<T> = std::result::Result<T, BoxedError>;
