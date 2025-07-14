//! Taken from the tiny-http project https://github.com/tiny-http/tiny-http/

#![cfg(not(target_arch = "wasm32"))]

use crate::io::ioutils::{PeekError, PeekableReadStream};
use crate::netcap::connection::Connection;
use crate::netcap::{DataStreamAddr, Endpoint, EndpointConfig};
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

#[cfg(feature = "ssl-native-tls")]
use native_tls::{Identity, TlsConnector, TlsStream};

/// A wrapper around a `native_tls` stream.
///
/// Uses an internal Mutex to permit disparate reader & writer threads to access the stream independently.
#[derive(Clone)]
pub struct NativeTlsStream(Arc<Mutex<native_tls::TlsStream<Connection>>>);

// These struct methods form the implict contract for swappable TLS implementations
impl NativeTlsStream {
    pub fn local_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .get_mut()
            .local_addr()
    }

    pub fn peer_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .get_mut()
            .peer_addr()
    }

    pub fn stream_addr(&mut self) -> std::io::Result<Option<DataStreamAddr>> {
        Ok(Some(DataStreamAddr::new(
            self.local_addr()?,
            self.peer_addr()?,
        )))
    }

    pub fn shutdown(&mut self, how: Shutdown) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .get_mut()
            .shutdown(how)
    }
}

impl PeekableReadStream for Connection {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .get_mut()
            .peek(buf)
    }
}

impl Read for NativeTlsStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .read(buf)
    }
}

impl Write for NativeTlsStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .flush()
    }
}

// Implementation for accepting incoming client connection within a TLS servers.
pub struct NativeTlsAcceptor(native_tls::TlsAcceptor);

impl NativeTlsAcceptor {
    pub fn from_identity(identity: Identity) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let acceptor = native_tls::TlsAcceptor::new(identity)?;
        Ok(Self(acceptor))
    }

    pub fn from_der(
        der: Vec<u8>,
        password: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let identity = native_tls::Identity::from_pkcs12(&der, &password)?;
        Self::from_identity(identity)
    }

    pub fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let identity = native_tls::Identity::from_pkcs8(&certificates, &private_key)?;
        Self::from_identity(identity)
    }

    pub fn accept(
        &self,
        stream: Connection,
    ) -> Result<NativeTlsStream, Box<dyn Error + Send + Sync + 'static>> {
        let stream = self.0.accept(stream)?;
        Ok(NativeTlsStream(Arc::new(Mutex::new(stream))))
    }
}

// Implementation for creating client connection to TLS servers.
pub struct NativeTlsConnector(Arc<native_tls::TlsConnector>);

impl NativeTlsConnector {
    pub fn create(endpoint: &Endpoint<Arc<native_tls::TlsConnector>>) -> Self {
        match &endpoint {
            Endpoint::WithIdentity(config, identity) => {
                Self(identity.clone());
            }
            _ => unreachable!("You generally won't call this method with Endpoint::NoIdentity since its left to you to generate")
        }
    }

    pub fn from_tcp_stream(
        &self,
        sni: &str,
        plain: Connection,
    ) -> Result<(NativeTlsStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let ssl_stream = self.0.connect(sni, plain)?;

        Ok(NativeTlsStream(Arc::new(Mutex::new(ssl_stream))))
    }

    pub fn from_endpoint<T: Clone>(
        &self,
        endpoint: Endpoint<T>,
    ) -> Result<(NativeTlsStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let host = endpoint.host();
        let host_socket_addr: SocketAddr = host.parse()?;

        let plain_stream = match self {
            Endpoint::WithDefault(config) => match config {
                EndpointConfig::WithTimeout(_, timeout) => {
                    TcpStream::connect_timeout(&host_socket_addr, timeout)
                }
                _ => TcpStream::connect(&host_socket_addr),
            },
            Endpoint::WithIdentity(config, _) => match config {
                EndpointConfig::WithTimeout(_, timeout) => {
                    TcpStream::connect_timeout(&host_socket_addr, timeout)
                }
                _ => TcpStream::connect(&host_socket_addr),
            },
        }?;

        Self::from_tcp_stream(host, plain_stream)
    }
}
