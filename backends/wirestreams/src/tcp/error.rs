use derive_more::From;

use std::io;

#[allow(unused)]
pub(crate) type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub type TlsResult<T> = std::result::Result<T, TlsError>;

#[derive(From, Debug)]
pub enum TlsError {
    Handshake,
    ConnectorCreation,

    #[from(ignore)]
    IO(io::Error),
}

impl From<io::Error> for TlsError {
    fn from(value: io::Error) -> Self {
        TlsError::IO(value)
    }
}

impl std::error::Error for TlsError {}

impl core::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type DataStreamResult<T> = std::result::Result<T, DataStreamError>;

#[derive(From, Debug)]
pub enum DataStreamError {
    ConnectionFailed,
    ReconnectionError,

    #[from(ignore)]
    IO(io::Error),

    #[from(ignore)]
    TLS(TlsError),
}

impl From<TlsError> for DataStreamError {
    fn from(value: TlsError) -> Self {
        Self::TLS(value)
    }
}

impl From<io::Error> for DataStreamError {
    fn from(value: io::Error) -> Self {
        Self::IO(value)
    }
}

impl std::error::Error for DataStreamError {}

impl core::fmt::Display for DataStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
