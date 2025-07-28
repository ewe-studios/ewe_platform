#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::missing_errors_doc)]

use derive_more::derive::From;

use crate::io::ioutils::{BufferedReader, BufferedWriter, PeekError, PeekableReadStream};

use super::ssl::{SSLConnector, ServerSSLStream};
use super::Endpoint;

#[cfg(feature = "ssl-openssl")]
use super::ssl::openssl;

#[cfg(feature = "ssl-rustls")]
use super::ssl::rustls;

#[cfg(feature = "ssl-native-tls")]
use super::ssl::native_tls;

use core::net;
use std::time::Duration;
use std::{net::TcpStream, time};

use super::{errors, Connection, DataStreamError, SocketAddr, TlsError};

pub enum RawStream {
    AsPlain(
        BufferedReader<BufferedWriter<Connection>>,
        super::DataStreamAddr,
        Option<SSLConnector>,
    ),
    AsTls(
        BufferedReader<BufferedWriter<ServerSSLStream>>,
        super::DataStreamAddr,
        Option<SSLConnector>,
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
        Ok(Self::AsPlain(reader, conn_addr, None))
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
        Ok(Self::AsTls(reader, conn_addr, None))
    }
}

// --- Constructors

impl RawStream {
    #[cfg(feature = "ssl-openssl")]
    pub fn tls_from_endpoint(
        endpoint: Endpoint<openssl::OpenSslConnector>,
    ) -> super::DataStreamResult<Self> {
        todo!()
    }

    #[cfg(feature = "ssl-rustls")]
    pub fn tls_from_endpoint(
        endpoint: Endpoint<rustls::RustlsConnector>,
    ) -> super::DataStreamResult<Self> {
        todo!()
    }

    #[cfg(feature = "ssl-native-tls")]
    pub fn tls_from_endpoint(
        endpoint: Endpoint<native_tls::NativeTlsConnector>,
    ) -> super::DataStreamResult<Self> {
        todo!()
    }
}

// --- Methods

#[allow(unused)]
impl RawStream {
    #[inline]
    pub fn read_timeout(&self) -> errors::TlsResult<Option<Duration>> {
        let result = match self {
            RawStream::AsPlain(inner, _, _) => inner.get_core_mut().read_timeout(),
            RawStream::AsTls(inner, _, _) => inner.get_core_mut().read_timeout(),
        };
        result.map_err(|_| TlsError::Failed)
    }

    #[inline]
    pub fn write_timeout(&self) -> errors::TlsResult<Option<Duration>> {
        let result = match self {
            RawStream::AsPlain(inner, _, _) => inner.get_core_mut().write_timeout(),
            RawStream::AsTls(inner, _, _) => inner.get_core_mut().write_timeout(),
        };
        result.map_err(|_| TlsError::Failed)
    }

    #[inline]
    pub fn set_write_timeout(&mut self, duration: Option<time::Duration>) -> errors::TlsResult<()> {
        let work = match self {
            RawStream::AsPlain(inner, _, _) => inner.get_core_mut().set_write_timeout(duration),
            RawStream::AsTls(inner, _, _) => inner.get_core_mut().set_write_timeout(duration),
        };

        match work {
            Ok(_) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    #[inline]
    pub fn set_read_timeout(&mut self, duration: Option<time::Duration>) -> errors::TlsResult<()> {
        let work = match self {
            RawStream::AsPlain(inner, _, _) => inner.get_core_mut().set_read_timeout(duration),
            RawStream::AsTls(inner, _, _) => inner.get_core_mut().set_read_timeout(duration),
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
            RawStream::AsPlain(inner, addr, _) => addr.clone(),
            RawStream::AsTls(inner, addr, _) => addr.clone(),
        }
    }

    #[inline]
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        match self {
            RawStream::AsPlain(inner, addr, _) => addr.peer_addr(),
            RawStream::AsTls(inner, addr, _) => addr.peer_addr(),
        }
    }

    #[inline]
    pub fn local_addr(&self) -> SocketAddr {
        match self {
            RawStream::AsPlain(inner, addr, _) => addr.local_addr(),
            RawStream::AsTls(inner, addr, _) => addr.local_addr(),
        }
    }
}

impl core::fmt::Debug for RawStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AsPlain(_, addr, _) => f
                .debug_tuple("RawStream::Plain")
                .field(&"_")
                .field(addr)
                .finish(),
            Self::AsTls(_, addr, _) => f
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
            RawStream::AsPlain(inner, _addr, _) => inner.peek(buf),
            RawStream::AsTls(inner, _addr, _) => inner.peek(buf),
        }
    }
}

impl std::io::Read for RawStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsPlain(inner, _, _) => inner.read(buf),
            RawStream::AsTls(inner, _, _) => inner.read(buf),
        }
    }
}

impl std::io::Write for RawStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsPlain(inner, _, _) => inner.write(buf),
            RawStream::AsTls(inner, _, _) => inner.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            RawStream::AsPlain(inner, _, _) => inner.flush(),
            RawStream::AsTls(inner, _, _) => inner.flush(),
        }
    }
}
