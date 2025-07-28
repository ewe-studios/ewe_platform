pub type BoxedError = Box<dyn std::error::Error + Send>;

pub type NonSendBoxedError = Box<dyn std::error::Error>;

pub type AnyResult<T, E> = std::result::Result<T, E>;

pub type GenericResult<T> = AnyResult<T, BoxedError>;

pub type NonSendGenericResult<T> = AnyResult<T, NonSendBoxedError>;
