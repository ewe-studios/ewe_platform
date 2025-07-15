#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::missing_errors_doc)]

use derive_more::derive::From;

use crate::io::ioutils::{BufferedReader, BufferedWriter, PeekError, PeekableReadStream};

use super::ssl::ServerSSLStream;

use core::net;
use std::net::SocketAddr;
use std::time::Duration;
use std::{net::TcpStream, time};

use super::{errors, Connection, DataStreamError, TlsError};

pub enum RawStream {
    AsPlain(
        BufferedReader<BufferedWriter<Connection>>,
        super::DataStreamAddr,
    ),
    AsTls(
        BufferedReader<BufferedWriter<ServerSSLStream>>,
        super::DataStreamAddr,
    ),
}

// -- Basic constructors

impl RawStream {
    /// from_endpoint_timeout creates a naked RawStream which is not mapped to a specific
    /// protocol version and simply is a TCPStream connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned RawStream is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_connection(conn: Connection) -> super::DataStreamResult<Self> {
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;

        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsPlain(reader, conn_addr))
    }

    /// from_endpoint creates a naked RawStream which is not mapped to a specific
    /// protocol version and simply is a TCPStream connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned RawStream is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_tls(conn: ServerSSLStream) -> super::DataStreamResult<Self> {
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;
        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsTls(reader, conn_addr))
    }
}

// --- Constructors

// --- Methods

#[allow(unused)]
impl RawStream {
    #[inline]
    pub fn read_timeout(&self) -> errors::TlsResult<Option<Duration>> {
        let result = match self {
            RawStream::AsPlain(inner, _) => inner.get_core_mut().read_timeout(),
            RawStream::AsTls(inner, _) => inner.get_core_mut().read_timeout(),
        };
        result.map_err(|_| TlsError::Failed)
    }

    #[inline]
    pub fn write_timeout(&self) -> errors::TlsResult<Option<Duration>> {
        let result = match self {
            RawStream::AsPlain(inner, _) => inner.get_core_mut().write_timeout(),
            RawStream::AsTls(inner, _) => inner.get_core_mut().write_timeout(),
        };
        result.map_err(|_| TlsError::Failed)
    }

    #[inline]
    pub fn set_write_timeout(&mut self, duration: Option<time::Duration>) -> errors::TlsResult<()> {
        let work = match self {
            RawStream::AsPlain(inner, _) => inner.get_core_mut().set_write_timeout(duration),
            RawStream::AsTls(inner, _) => inner.get_core_mut().set_write_timeout(duration),
        };

        match work {
            Ok(_) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    #[inline]
    pub fn set_read_timeout(&mut self, duration: Option<time::Duration>) -> errors::TlsResult<()> {
        let work = match self {
            RawStream::AsPlain(inner, _) => inner.get_core_mut().set_read_timeout(duration),
            RawStream::AsTls(inner, _) => inner.get_core_mut().set_read_timeout(duration),
        };

        match work {
            Ok(_) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    // #[inline]
    // pub fn clone(&self) -> errors::TlsResult<TcpStream> {
    //     let work = match self {
    //         RawStream::AsPlain(inner, _) => inner.clone(),
    //         RawStream::AsTls(inner, _) => inner.get_inner_ref().get_ref().clone(),
    //     };
    //
    //     match work {
    //         Ok(inner) => Ok(inner),
    //         Err(err) => Err(err.into()),
    //     }
    // }

    #[inline]
    pub fn addrs(&self) -> super::DataStreamAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.clone(),
            RawStream::AsTls(inner, addr) => addr.clone(),
        }
    }

    #[inline]
    pub fn peer_addr(&self) -> SocketAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.peer_addr(),
            RawStream::AsTls(inner, addr) => addr.peer_addr(),
        }
    }

    #[inline]
    pub fn local_addr(&self) -> SocketAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.local_addr(),
            RawStream::AsTls(inner, addr) => addr.local_addr(),
        }
    }
}

impl core::fmt::Debug for RawStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AsPlain(_, addr) => f
                .debug_tuple("RawStream::Plain")
                .field(&"_")
                .field(addr)
                .finish(),
            Self::AsTls(_, addr) => f
                .debug_tuple("RawStream::TLS")
                .field(&"_")
                .field(addr)
                .finish(),
        }
    }
}

impl PeekableReadStream for RawStream {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        match self {
            RawStream::AsPlain(inner, _addr) => match inner.peek(buf) {
                Ok(count) => Ok(count),
                Err(err) => Err(PeekError::IOError(err)),
            },
            RawStream::AsTls(inner, _addr) => match inner.peek(buf) {
                Ok(count) => Ok(count),
                Err(err) => Err(err),
            },
        }
    }
}

impl std::io::Read for RawStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsPlain(inner, _) => inner.read(buf),
            RawStream::AsTls(inner, _) => inner.read(buf),
        }
    }
}

impl std::io::Write for RawStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsPlain(inner, _) => inner.write(buf),
            RawStream::AsTls(inner, _) => inner.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            RawStream::AsPlain(inner, _) => inner.flush(),
            RawStream::AsTls(inner, _) => inner.flush(),
        }
    }
}
