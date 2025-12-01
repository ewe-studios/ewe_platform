use crate::extensions::result_ext::{BoxedError, SendableBoxedError};

pub type AnyResult<T, E> = std::result::Result<T, E>;

pub type GenericResult<T> = AnyResult<T, BoxedError>;

pub type SendGenericResult<T> = AnyResult<T, SendableBoxedError>;
