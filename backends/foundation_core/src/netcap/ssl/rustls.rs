//! Taken from the tiny-http project <https://github.com/tiny-http/tiny-http>/

#![cfg(not(target_arch = "wasm32"))]

use crate::netcap::connection::Connection;
use crate::netcap::errors::BoxedError;
use crate::netcap::{
    DataStreamAddr, DataStreamError, DataStreamResult, Endpoint, EndpointConfig, SocketAddr,
};
use rustls::pki_types::ServerName;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

pub use rustls::{ClientConfig, ServerConfig};

/// A wrapper around an owned Rustls connection and corresponding stream.
///
/// Uses an internal Mutex to permit disparate reader & writer threads to access the stream independently.
pub struct RustlsStream<T>(Arc<Mutex<rustls::StreamOwned<T, Connection>>>);

impl<T> RustlsStream<T> {
    pub fn try_clone_connection(&self) -> std::io::Result<Connection> {
        self.0
            .lock()
            .expect("Failed to lock ssl stream")
            .sock
            .try_clone()
    }

    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .read_timeout()
    }

    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .write_timeout()
    }

    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .set_write_timeout(dur)
    }

    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .set_read_timeout(dur)
    }
}

impl<T> RustlsStream<T> {
    pub fn local_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .local_addr()
    }

    pub fn peer_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .peer_addr()
    }

    pub fn stream_addr(&self) -> DataStreamResult<DataStreamAddr> {
        let local_addr = self.local_addr()?;
        let peer_addr = self.peer_addr()?;

        match (local_addr, peer_addr) {
            (Some(l1), Some(l2)) => Ok(DataStreamAddr::new(l1, Some(l2))),
            (Some(l1), None) => Ok(DataStreamAddr::new(l1, None)),
            (None, Some(_)) => Err(DataStreamError::NoLocalAddr),
            _ => Err(DataStreamError::NoAddr),
        }
    }

    pub fn shutdown(&mut self, how: Shutdown) -> std::io::Result<()> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .shutdown(how)
    }
}

impl<T> Clone for RustlsStream<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Read for RustlsStream<rustls::ClientConnection> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .read(buf)
    }
}

impl Read for RustlsStream<rustls::ServerConnection> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .read(buf)
    }
}

impl Write for RustlsStream<rustls::ClientConnection> {
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

impl Write for RustlsStream<rustls::ServerConnection> {
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

pub type RustTlsServerStream = RustlsStream<rustls::ServerConnection>;

#[derive(Clone)]
pub struct RustlsAcceptor(Arc<rustls::ServerConfig>);

impl RustlsAcceptor {
    pub fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        use rustls::pki_types::pem::PemObject;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let certs_result: Result<
            Vec<rustls::pki_types::CertificateDer<'static>>,
            rustls::pki_types::pem::Error,
        > = CertificateDer::pem_slice_iter(certificates.as_slice()).collect();

        let certs = certs_result?;
        let p_key = PrivateKeyDer::from_pem_slice(private_key.as_slice())?;

        let tls_conf = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, p_key)?;

        Ok(Self(Arc::new(tls_conf)))
    }

    pub fn accept(
        &self,
        stream: Connection,
    ) -> Result<RustTlsServerStream, Box<dyn Error + Send + Sync + 'static>> {
        let connection = rustls::ServerConnection::new(self.0.clone())?;
        Ok(RustlsStream(Arc::new(Mutex::new(
            rustls::StreamOwned::new(connection, stream),
        ))))
    }
}

#[derive(Clone)]
pub struct RustlsConnector(Arc<rustls::ClientConfig>);

pub type RustTlsClientStream = RustlsStream<rustls::ClientConnection>;

impl RustlsConnector {
    #[must_use]
    pub fn create(endpoint: &Endpoint<Arc<rustls::ClientConfig>>) -> Self {
        match &endpoint {
            Endpoint::WithIdentity(_, identity) => {
                Self(identity.clone())
            }
            _ => unreachable!("You generally won't call this method with Endpoint::NoIdentity since its left to you to generate")
        }
    }

    pub fn from_tcp_stream(
        &self,
        sni: String,
        plain: Connection,
    ) -> Result<(RustTlsClientStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let addr = match (local_addr, peer_addr) {
            (Some(l1), Some(l2)) => Ok(DataStreamAddr::new(l1, Some(l2))),
            (Some(l1), None) => Ok(DataStreamAddr::new(l1, None)),
            (None, Some(_)) => Err(DataStreamError::NoPeerAddr),
            _ => Err(DataStreamError::NoAddr),
        }?;

        let server_name: ServerName = sni.try_into().map_err(Box::new)?;
        let conn = rustls::ClientConnection::new(self.0.clone(), server_name)?;
        let ssl_stream = rustls::StreamOwned::new(conn, plain);
        let shared_stream = Arc::new(Mutex::new(ssl_stream));

        Ok((RustlsStream(shared_stream), addr))
    }

    pub fn from_endpoint(
        &self,
        endpoint: &Endpoint<Arc<rustls::ClientConfig>>,
    ) -> Result<(RustTlsClientStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let host = endpoint.host();
        let host_socket_addr: core::net::SocketAddr = host.parse()?;

        let plain_stream = match endpoint {
            Endpoint::WithDefault(config) => match config {
                EndpointConfig::WithTimeout(_, timeout) => {
                    TcpStream::connect_timeout(&host_socket_addr, *timeout)
                }
                _ => TcpStream::connect(host_socket_addr),
            },
            Endpoint::WithIdentity(config, _) => match config {
                EndpointConfig::WithTimeout(_, timeout) => {
                    TcpStream::connect_timeout(&host_socket_addr, *timeout)
                }
                _ => TcpStream::connect(host_socket_addr),
            },
        }?;

        self.from_tcp_stream(host, Connection::Tcp(plain_stream))
    }
}
