use derive_more::From;

use std::{error, fmt, io, net::AddrParseError};

pub type BoxedError = Box<dyn error::Error + Send + Sync + 'static>;

pub type TlsResult<T> = std::result::Result<T, TlsError>;

#[derive(From, Debug)]
pub enum TlsError {
    Failed,
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
    NoAddr,
    NoLocalAddr,
    NoPeerAddr,
    ConnectionFailed,
    ReconnectionError,
    FailedToAcquireAddrs,

    #[from(ignore)]
    Boxed(BoxedError),

    #[from(ignore)]
    IO(io::Error),

    #[from(ignore)]
    TLS(TlsError),

    #[from(ignore)]
    SocketAddrError(AddrParseError),
}

impl Eq for DataStreamError {}

impl PartialEq for DataStreamError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::TLS(m1), Self::TLS(m2)) => m1 == m2,
            (Self::IO(m1), Self::IO(m2)) => m1.kind() == m2.kind(),
            (Self::SocketAddrError(m1), Self::SocketAddrError(m2)) => {
                m1.to_string() == m2.to_string()
            }
            (Self::ConnectionFailed, Self::ConnectionFailed) => true,
            (Self::ReconnectionError, Self::ReconnectionError) => true,
            _ => false,
        }
    }
}

impl From<BoxedError> for DataStreamError {
    fn from(value: BoxedError) -> Self {
        Self::Boxed(value)
    }
}

impl From<AddrParseError> for DataStreamError {
    fn from(value: AddrParseError) -> Self {
        Self::SocketAddrError(value)
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
