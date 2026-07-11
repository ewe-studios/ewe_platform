#![cfg(not(target_family = "wasm"))]
#![allow(clippy::missing_errors_doc)]

use std::sync::Arc;
use std::time::Duration;
use std::{net::TcpStream, time};

use foundation_core::io::ioutils::{
    BufferedReader, BufferedWriter, PeekError, PeekableReadStream, ReadTimeoutOperations,
};

use crate::native::connection::{Connection, DataStreamAddr, Endpoint, EndpointConfig, SocketAddr};
use crate::shared::errors::{self, DataStreamError, TlsError};

#[cfg(any(
    feature = "ssl-rustls",
    feature = "ssl-openssl",
    feature = "ssl-native-tls"
))]
use crate::native::ssl::{ClientSSLStream, ServerSSLStream};

// TLS module imports with priority resolution: rustls > openssl > native-tls.
// When --all-features enables all backends, the primary (rustls) wins.
#[cfg(feature = "ssl-rustls")]
use crate::native::ssl::rustls;

#[cfg(all(not(feature = "ssl-rustls"), feature = "ssl-openssl"))]
use crate::native::ssl::openssl;

#[cfg(all(
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl"),
    feature = "ssl-native-tls"
))]
use crate::native::ssl::native_ttls;

pub enum RawStream {
    AsPlain(BufferedReader<BufferedWriter<Connection>>, DataStreamAddr),
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    AsServerTls(
        BufferedReader<BufferedWriter<ServerSSLStream>>,
        DataStreamAddr,
    ),
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    AsClientTls(
        BufferedReader<BufferedWriter<ClientSSLStream>>,
        DataStreamAddr,
    ),
}

impl core::fmt::Debug for RawStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AsPlain(_, addr) => f
                .debug_tuple("RawStream::Plain")
                .field(&"_")
                .field(addr)
                .finish(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(_, addr) => f
                .debug_tuple("RawStream::Server::TLS")
                .field(&"_")
                .field(addr)
                .finish(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(_, addr) => f
                .debug_tuple("RawStream::Client::TLS")
                .field(&"_")
                .field(addr)
                .finish(),
        }
    }
}

/// `AsRawFd`/`AsFd` reach through the `BufferedReader<BufferedWriter<…>>` wrapper
/// to the underlying socket's file descriptor, so the `ConnectRPC` layer (above
/// `netio`) can register a live `RawStream` with the `foundation_nativeapis`
/// reactor and obtain a `RegisteredFd: EventReadiness` to park on (Decision 12
/// §12). TLS variants delegate to the fd of the wrapped TCP socket — readiness
/// is a socket property, not a TLS-layer one. Unix-only (native-socket); wasm
/// parks via the browser instead.
#[cfg(unix)]
impl std::os::unix::io::AsRawFd for RawStream {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        match self {
            Self::AsPlain(inner, _) => inner.get_core_ref().as_raw_fd(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _) => inner.get_core_ref().as_raw_fd(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _) => inner.get_core_ref().as_raw_fd(),
        }
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsFd for RawStream {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        // SAFETY: the fd is owned by the wrapped socket and stays open for the
        // borrow's lifetime; `BorrowedFd` does not close it.
        unsafe {
            std::os::unix::io::BorrowedFd::borrow_raw(
                <Self as std::os::unix::io::AsRawFd>::as_raw_fd(self),
            )
        }
    }
}

// -- Basic constructors

impl RawStream {
    /// [`Self::from_tcp`] creates a naked `RawStream` from a `TCPStream` connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned `RawStream` is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_tcp(stream: TcpStream) -> crate::shared::errors::DataStreamResult<Self> {
        let conn = Connection::Tcp(stream);
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;

        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsPlain(reader, conn_addr))
    }

    /// [`Self::from_connection`] creates a naked `RawStream` which is not mapped to a specific
    /// protocol version and simply is a `TCPStream` connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned `RawStream` is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_connection(conn: Connection) -> crate::shared::errors::DataStreamResult<Self> {
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;

        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsPlain(reader, conn_addr))
    }

    /// `from_server_tls` creates a `RawStream` from a server generated TLS Connection wrapped
    /// by the [`ServerSSLStream`] type. Generally this is generated from a `Listener`
    /// which outputs the necessary connection.
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    pub fn from_server_tls(conn: ServerSSLStream) -> crate::shared::errors::DataStreamResult<Self> {
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;
        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsServerTls(reader, conn_addr))
    }

    /// `from_client_tls` creates a `RawStream` from a client generated TLS Connection wrapped
    /// by the [`ClientSSLStream`] type. Generally this is generated from [`TcpStream`] or equivalent
    /// that connects to a remote endpoint.
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    pub fn from_client_tls(conn: ClientSSLStream) -> crate::shared::errors::DataStreamResult<Self> {
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;
        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsClientTls(reader, conn_addr))
    }
}

// Type alias for TLS config with priority resolution: rustls > openssl > native-tls.
#[cfg(feature = "ssl-rustls")]
type TlsClientConfig = rustls::ClientConfig;

#[cfg(all(not(feature = "ssl-rustls"), feature = "ssl-openssl"))]
type TlsClientConfig = openssl::SslConnector;

#[cfg(all(
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl"),
    feature = "ssl-native-tls"
))]
type TlsClientConfig = native_ttls::TlsConnector;

// [`ClientEndpoint`] is a client connector that wraps and hides the complexity of
// what type of endpoint is need to connect to a server from a client side.
#[derive(Clone, Debug)]
pub enum ClientEndpoint {
    Plain(Endpoint<()>),
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    Tls(Endpoint<Arc<TlsClientConfig>>),
}

// --- Constructors

impl RawStream {
    pub fn from_endpoint(
        endpoint: &ClientEndpoint,
    ) -> crate::shared::errors::DataStreamResult<Self> {
        match endpoint {
            ClientEndpoint::Plain(endpoint) => Self::client_from_endpoint(endpoint),
            #[cfg(feature = "ssl-rustls")]
            ClientEndpoint::Tls(endpoint) => {
                let (connection, addr) =
                    rustls::RustlsConnector::client_tls_from_endpoint(endpoint)?;
                let reader = BufferedReader::new(BufferedWriter::new(connection));
                Ok(Self::AsClientTls(reader, addr))
            }
            #[cfg(all(not(feature = "ssl-rustls"), feature = "ssl-openssl"))]
            ClientEndpoint::Tls(endpoint) => {
                let (connection, addr) =
                    openssl::OpenSslConnector::client_tls_from_endpoint(endpoint)?;
                let reader = BufferedReader::new(BufferedWriter::new(connection));
                Ok(RawStream::AsClientTls(reader, addr))
            }
            #[cfg(all(
                not(feature = "ssl-rustls"),
                not(feature = "ssl-openssl"),
                feature = "ssl-native-tls"
            ))]
            ClientEndpoint::Tls(endpoint) => {
                let (connection, addr) =
                    native_ttls::NativeTlsConnector::client_tls_from_endpoint(endpoint)?;
                let reader = BufferedReader::new(BufferedWriter::new(connection));
                Ok(RawStream::AsClientTls(reader, addr))
            }
        }
    }

    pub fn client_from_endpoint(
        endpoint: &Endpoint<()>,
    ) -> crate::shared::errors::DataStreamResult<Self> {
        let host = endpoint.host();
        let host_socket_addr: core::net::SocketAddr = host.parse()?;

        let plain_stream = if let Endpoint::WithDefault(config) = endpoint {
            match config {
                EndpointConfig::WithTimeout(_, timeout) => {
                    Connection::with_timeout(host_socket_addr, *timeout)
                }
                _ => Connection::without_timeout(host_socket_addr),
            }
        } else {
            unreachable!("Should not attempt creating tls connection in this method")
        }?;

        Self::from_connection(plain_stream)
    }
}

// --- Methods

#[allow(unused)]
impl RawStream {
    #[inline]
    pub fn read_timeout(&self) -> errors::TlsResult<Option<Duration>> {
        let result = match self {
            Self::AsPlain(inner, _) => inner.get_core_ref().read_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _) => inner.get_core_ref().read_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _) => inner.get_core_ref().read_timeout(),
        };
        result.map_err(|_| TlsError::Failed)
    }

    #[inline]
    pub fn write_timeout(&self) -> errors::TlsResult<Option<Duration>> {
        let result = match self {
            Self::AsPlain(inner, _) => inner.get_core_ref().write_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _) => inner.get_core_ref().write_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _) => inner.get_core_ref().write_timeout(),
        };
        result.map_err(|_| TlsError::Failed)
    }

    #[inline]
    pub fn set_write_timeout(&mut self, duration: Option<time::Duration>) -> errors::TlsResult<()> {
        let work = match self {
            Self::AsPlain(inner, _) => inner.get_core_mut().set_write_timeout(duration),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _) => inner.get_core_mut().set_write_timeout(duration),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _) => inner.get_core_mut().set_write_timeout(duration),
        };

        match work {
            Ok(()) => Ok(()),
            Err(err) => Err(errors::TlsError::from(err)),
        }
    }

    #[inline]
    pub fn set_read_timeout(&mut self, duration: Option<time::Duration>) -> errors::TlsResult<()> {
        tracing::debug!(
            "set_read_timeout: Received instruction to set read timeout to: {:?}",
            &duration
        );
        let work = match self {
            Self::AsPlain(inner, _) => inner.get_core_mut().set_read_timeout(duration),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _) => inner.get_core_mut().set_read_timeout(duration),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _) => inner.get_core_mut().set_read_timeout(duration),
        };

        match work {
            Ok(()) => Ok(()),
            Err(err) => Err(errors::TlsError::from(err)),
        }
    }

    // #[inline]
    // pub fn clone_stream(&self) -> errors::TlsResult<Self> {
    //     let work = match self {
    //         RawStream::AsPlain(inner, addr) => Self::AsPlain(inner.try_clone(), addr.clone()),
    //         RawStream::AsTls(inner, _) => inner.get_inner_ref().get_ref().clone(),
    //     };

    //     match work {
    //         Ok(inner) => Ok(inner),
    //         Err(err) => Err(err.into()),
    //     }
    // }

    #[inline]
    #[must_use]
    pub fn addrs(&self) -> DataStreamAddr {
        match self {
            Self::AsPlain(inner, addr) => addr.clone(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, addr) => addr.clone(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, addr) => addr.clone(),
        }
    }

    #[inline]
    #[must_use]
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        match self {
            Self::AsPlain(inner, addr) => addr.peer_addr(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, addr) => addr.peer_addr(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, addr) => addr.peer_addr(),
        }
    }

    #[inline]
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        match self {
            Self::AsPlain(inner, addr) => addr.local_addr(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, addr) => addr.local_addr(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, addr) => addr.local_addr(),
        }
    }
}

impl PeekableReadStream for RawStream {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        match self {
            Self::AsPlain(inner, _addr) => inner.peek(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _addr) => inner.peek(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _addr) => inner.peek(buf),
        }
    }
}

impl ReadTimeoutOperations for RawStream {
    fn read_timeout_into(
        &mut self,
        buf: &mut [u8],
        timeout: std::time::Duration,
    ) -> std::result::Result<usize, std::io::Error> {
        match self {
            Self::AsPlain(inner, _addr) => inner.read_timeout_into(buf, timeout),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _addr) => inner.read_timeout_into(buf, timeout),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _addr) => inner.read_timeout_into(buf, timeout),
        }
    }

    fn set_read_timeout_as(
        &mut self,
        timeout: std::time::Duration,
    ) -> std::result::Result<(), std::io::Error> {
        tracing::debug!(
            "set_read_timeout_as: Received instruction to set read timeout to: {:?}",
            &timeout,
        );
        match self {
            Self::AsPlain(inner, _addr) => inner.set_read_timeout_as(timeout),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _addr) => inner.set_read_timeout_as(timeout),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _addr) => inner.set_read_timeout_as(timeout),
        }
    }

    fn get_current_read_timeout(
        &self,
    ) -> std::result::Result<Option<std::time::Duration>, std::io::Error> {
        match self {
            Self::AsPlain(inner, _addr) => inner.get_current_read_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _addr) => inner.get_current_read_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _addr) => inner.get_current_read_timeout(),
        }
    }
}

impl std::io::Read for RawStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::AsPlain(inner, _addr) => inner.read(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _addr) => inner.read(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _addr) => inner.read(buf),
        }
    }
}

impl std::io::Write for RawStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::AsPlain(inner, _addr) => inner.write(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _addr) => inner.write(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _addr) => inner.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::AsPlain(inner, _addr) => inner.flush(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsServerTls(inner, _addr) => inner.flush(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            Self::AsClientTls(inner, _addr) => inner.flush(),
        }
    }
}
