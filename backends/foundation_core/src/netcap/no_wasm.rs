#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::missing_errors_doc)]

use derive_more::derive::From;

use crate::io::ioutils::{BufferedReader, BufferedWriter, PeekError, PeekableReadStream};

#[cfg(any(
    feature = "ssl-rustls",
    feature = "ssl-openssl",
    feature = "ssl-native-tls"
))]
use super::ssl::{ClientSSLStream, SSLConnector, ServerSSLStream};
use super::{Endpoint, EndpointConfig};

#[cfg(all(
    feature = "ssl-openssl",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-native-tls")
))]
use super::ssl::openssl;

#[cfg(all(
    feature = "ssl-rustls",
    not(feature = "ssl-openssl"),
    not(feature = "ssl-native-tls")
))]
use super::ssl::rustls;

#[cfg(all(
    feature = "ssl-native-tls",
    not(feature = "ssl-rustls"),
    not(feature = "ssl-openssl")
))]
use super::ssl::native_ttls;

use core::net;
use std::sync::Arc;
use std::time::Duration;
use std::{net::TcpStream, time};

use super::{errors, Connection, DataStreamError, SocketAddr, TlsError};

pub enum RawStream {
    AsPlain(
        BufferedReader<BufferedWriter<Connection>>,
        super::DataStreamAddr,
    ),
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    AsServerTls(
        BufferedReader<BufferedWriter<ServerSSLStream>>,
        super::DataStreamAddr,
    ),
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    AsClientTls(
        BufferedReader<BufferedWriter<ClientSSLStream>>,
        super::DataStreamAddr,
    ),
}

// -- Basic constructors

impl RawStream {
    /// [`from_tcp`] creates a naked `RawStream` from a `TCPStream` connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned `RawStream` is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_tcp(stream: TcpStream) -> super::DataStreamResult<Self> {
        let conn = Connection::Tcp(stream);
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;

        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsPlain(reader, conn_addr))
    }

    /// [`from_connection`] creates a naked `RawStream` which is not mapped to a specific
    /// protocol version and simply is a `TCPStream` connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned `RawStream` is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_connection(conn: Connection) -> super::DataStreamResult<Self> {
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;

        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsPlain(reader, conn_addr))
    }

    /// `from_server_tls` creates a `RawStream` from a server generated TLS Connection wrapped
    /// by the [`ServerSSLStream`] type. Generally this is generated from a [`Listener`]
    /// which outputs the necessary connection.
    #[cfg(any(
        feature = "ssl-rustls",
        feature = "ssl-openssl",
        feature = "ssl-native-tls"
    ))]
    pub fn from_server_tls(conn: ServerSSLStream) -> super::DataStreamResult<Self> {
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
    pub fn from_client_tls(conn: ClientSSLStream) -> super::DataStreamResult<Self> {
        let conn_addr = conn
            .stream_addr()
            .map_err(|_| DataStreamError::FailedToAcquireAddrs)?;
        let reader = BufferedReader::new(BufferedWriter::new(conn));
        Ok(Self::AsClientTls(reader, conn_addr))
    }
}

// [`ClientEndpoint`] is a client connector that wraps and hides the complexity of
// what type of endpoint is need to connect to a server from a client side.
#[derive(Clone, Debug)]
pub enum ClientEndpoint {
    Plain(Endpoint<()>),

    #[cfg(all(
        feature = "ssl-rustls",
        not(feature = "ssl-openssl"),
        not(feature = "ssl-native-tls")
    ))]
    Tls(Endpoint<Arc<rustls::ClientConfig>>),

    #[cfg(all(
        feature = "ssl-openssl",
        not(feature = "ssl-rustls"),
        not(feature = "ssl-native-tls")
    ))]
    Tls(Endpoint<Arc<openssl::SslConnector>>),

    #[cfg(all(
        feature = "ssl-native-tls",
        not(feature = "ssl-rustls"),
        not(feature = "ssl-openssl")
    ))]
    Tls(Endpoint<Arc<native_ttls::TlsConnector>>),
}

// --- Constructors

impl RawStream {
    pub fn from_endpoint(endpoint: &ClientEndpoint) -> super::DataStreamResult<Self> {
        match endpoint {
            ClientEndpoint::Plain(endpoint) => Self::client_from_endpoint(endpoint),
            #[cfg(all(
                feature = "ssl-rustls",
                not(feature = "ssl-openssl"),
                not(feature = "ssl-native-tls")
            ))]
            ClientEndpoint::Tls(endpoint) => Self::client_tls_from_endpoint(endpoint),
            #[cfg(all(
                feature = "ssl-openssl",
                not(feature = "ssl-rustls"),
                not(feature = "ssl-native-tls")
            ))]
            ClientEndpoint::Tls(endpoint) => Self::client_tls_from_endpoint(endpoint),
            #[cfg(all(
                feature = "ssl-native-tls",
                not(feature = "ssl-rustls"),
                not(feature = "ssl-openssl")
            ))]
            ClientEndpoint::Tls(endpoint) => Self::client_tls_from_endpoint(endpoint),
        }
    }

    pub fn client_from_endpoint(endpoint: &Endpoint<()>) -> super::DataStreamResult<Self> {
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

    #[cfg(all(
        feature = "ssl-rustls",
        not(feature = "ssl-openssl"),
        not(feature = "ssl-native-tls")
    ))]
    pub fn client_tls_from_endpoint(
        endpoint: &Endpoint<Arc<rustls::ClientConfig>>,
    ) -> super::DataStreamResult<Self> {
        let connector = rustls::RustlsConnector::create(endpoint);
        let (connection, addr) = connector.from_endpoint(endpoint)?;
        let reader = BufferedReader::new(BufferedWriter::new(connection));
        Ok(RawStream::AsClientTls(reader, addr))
    }

    #[cfg(all(
        feature = "ssl-openssl",
        not(feature = "ssl-rustls"),
        not(feature = "ssl-native-tls")
    ))]
    pub fn client_tls_from_endpoint(
        endpoint: &Endpoint<Arc<openssl::SslConnector>>,
    ) -> super::DataStreamResult<Self> {
        let connector = openssl::OpenSslConnector::create(endpoint);
        let (connection, addr) = connector.from_endpoint(endpoint)?;
        let reader = BufferedReader::new(BufferedWriter::new(connection));
        Ok(RawStream::AsClientTls(reader, addr))
    }

    #[cfg(all(
        feature = "ssl-native-tls",
        not(feature = "ssl-rustls"),
        not(feature = "ssl-openssl")
    ))]
    pub fn client_tls_from_endpoint(
        endpoint: &Endpoint<Arc<native_ttls::TlsConnector>>,
    ) -> super::DataStreamResult<Self> {
        let connector = native_ttls::NativeTlsConnector::create(endpoint);
        let (connection, addr) = connector.from_endpoint(endpoint)?;
        let reader = BufferedReader::new(BufferedWriter::new(connection));
        Ok(RawStream::AsClientTls(reader, addr))
    }
}

// --- Methods

#[allow(unused)]
impl RawStream {
    #[inline]
    pub fn read_timeout(&self) -> errors::TlsResult<Option<Duration>> {
        let result = match self {
            RawStream::AsPlain(inner, _) => inner.get_core_ref().read_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, _) => inner.get_core_ref().read_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, _) => inner.get_core_ref().read_timeout(),
        };
        result.map_err(|_| TlsError::Failed)
    }

    #[inline]
    pub fn write_timeout(&self) -> errors::TlsResult<Option<Duration>> {
        let result = match self {
            RawStream::AsPlain(inner, _) => inner.get_core_ref().write_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, _) => inner.get_core_ref().write_timeout(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, _) => inner.get_core_ref().write_timeout(),
        };
        result.map_err(|_| TlsError::Failed)
    }

    #[inline]
    pub fn set_write_timeout(&mut self, duration: Option<time::Duration>) -> errors::TlsResult<()> {
        let work = match self {
            RawStream::AsPlain(inner, _) => inner.get_core_mut().set_write_timeout(duration),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, _) => inner.get_core_mut().set_write_timeout(duration),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, _) => inner.get_core_mut().set_write_timeout(duration),
        };

        match work {
            Ok(()) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    #[inline]
    pub fn set_read_timeout(&mut self, duration: Option<time::Duration>) -> errors::TlsResult<()> {
        let work = match self {
            RawStream::AsPlain(inner, _) => inner.get_core_mut().set_read_timeout(duration),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, _) => inner.get_core_mut().set_read_timeout(duration),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, _) => inner.get_core_mut().set_read_timeout(duration),
        };

        match work {
            Ok(()) => Ok(()),
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
    #[must_use]
    pub fn addrs(&self) -> super::DataStreamAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.clone(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, addr) => addr.clone(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, addr) => addr.clone(),
        }
    }

    #[inline]
    #[must_use]
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        match self {
            RawStream::AsPlain(inner, addr) => addr.peer_addr(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, addr) => addr.peer_addr(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, addr) => addr.peer_addr(),
        }
    }

    #[inline]
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.local_addr(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, addr) => addr.local_addr(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, addr) => addr.local_addr(),
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

impl PeekableReadStream for RawStream {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        match self {
            RawStream::AsPlain(inner, _addr) => inner.peek(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, _addr) => inner.peek(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, _addr) => inner.peek(buf),
        }
    }
}

impl std::io::Read for RawStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsPlain(inner, _addr) => inner.read(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, _addr) => inner.read(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, _addr) => inner.read(buf),
        }
    }
}

impl std::io::Write for RawStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsPlain(inner, _addr) => inner.write(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, _addr) => inner.write(buf),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, _addr) => inner.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            RawStream::AsPlain(inner, _addr) => inner.flush(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsServerTls(inner, _addr) => inner.flush(),
            #[cfg(any(
                feature = "ssl-rustls",
                feature = "ssl-openssl",
                feature = "ssl-native-tls"
            ))]
            RawStream::AsClientTls(inner, _addr) => inner.flush(),
        }
    }
}
