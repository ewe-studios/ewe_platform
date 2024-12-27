use derive_more::From;

use std::io;

pub type TlsResult<T> = std::result::Result<T, TlsError>;

#[derive(From, Debug)]
pub enum TlsError {
    Handshake,
    ConnectorCreation,

    #[from(ignore)]
    IO(io::Error),
}

impl Eq for TlsError {}

impl PartialEq for TlsError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::IO(m1), Self::IO(m2)) => m1.kind() == m2.kind(),
            (Self::Handshake, Self::Handshake) => true,
            (Self::ConnectorCreation, Self::ConnectorCreation) => true,
            _ => false,
        }
    }
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

impl Eq for DataStreamError {}

impl PartialEq for DataStreamError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::TLS(m1), Self::TLS(m2)) => m1 == m2,
            (Self::IO(m1), Self::IO(m2)) => m1.kind() == m2.kind(),
            (Self::ConnectionFailed, Self::ConnectionFailed) => true,
            (Self::ReconnectionError, Self::ReconnectionError) => true,
            _ => false,
        }
    }
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
