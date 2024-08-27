// Types for the packages

pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub type Result<T> = std::result::Result<T, BoxedError>;
