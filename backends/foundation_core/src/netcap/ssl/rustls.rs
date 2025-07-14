//! Taken from the tiny-http project https://github.com/tiny-http/tiny-http/

#![cfg(not(target_arch = "wasm32"))]

use crate::netcap::connection::Connection;
use crate::netcap::errors::BoxedErrors;
use crate::netcap::{DataStreamAddr, Endpoint, EndpointConfig};
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

/// A wrapper around an owned Rustls connection and corresponding stream.
///
/// Uses an internal Mutex to permit disparate reader & writer threads to access the stream independently.
pub struct RustlsStream<T>(Arc<Mutex<rustls::StreamOwned<T, Connection>>>);

impl<T> RustlsStream<T> {
    pub fn local_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
            .local_addr()
    }

    pub fn peer_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .sock
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
            .sock
            .shutdown(how)
    }
}

impl<T> Clone for RustlsStream<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Read for RustlsStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("Failed to lock SSL stream mutex")
            .read(buf)
    }
}

impl<T> Write for RustlsStream<T> {
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

        let certs = CertificateDer::from_pem_slice(private_key.as_slice())?;
        let p_key = PrivateKeyDer::from_pem_slice(private_key.as_slice())?;

        let tls_conf = rustls::ServerConfig::builder()
            .with_safe_defaults()
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
    pub fn create(endpoint: &Endpoint<Arc<rustls::ClientConfig>>) -> Self {
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
    ) -> Result<(RustTlsClientStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let mut conn = rustls::ClientConnection::new(self.0.clone(), sni)?;
        let mut ssl_stream = rustls::StreamOwned::new(conn, plain);

        Ok(RustlsStream(Arc::new(Mutex::new(ssl_stream))))
    }

    pub fn from_endpoint<T: Clone>(
        &self,
        endpoint: &Endpoint<T>,
    ) -> Result<(RustTlsClientStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
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
