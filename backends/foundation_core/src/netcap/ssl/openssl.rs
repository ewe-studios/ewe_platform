//! Taken from the tiny-http project https://github.com/tiny-http/tiny-http/

#![cfg(not(target_arch = "wasm32"))]

use crate::netcap::connection::Connection;
use crate::netcap::{
    DataStreamAddr, DataStreamError, DataStreamResult, Endpoint, EndpointConfig, SocketAddr,
};
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

pub use openssl::ssl::SslConnector;

pub struct OpenSslStream {
    inner: openssl::ssl::SslStream<Connection>,
}

impl OpenSslStream {
    pub fn try_clone_connection(&self) -> std::io::Result<Connection> {
        self.inner.get_ref().try_clone()
    }

    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.inner.get_ref().read_timeout()
    }

    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        self.inner.get_ref().write_timeout()
    }

    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.inner.get_mut().set_write_timeout(dur)
    }

    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        self.inner.get_mut().set_read_timeout(dur)
    }
}

impl Read for OpenSslStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for OpenSslStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// An OpenSSL stream which has been split into two mutually exclusive streams (e.g. for read / write)
pub struct SplitOpenSslStream(Arc<Mutex<OpenSslStream>>);

impl core::fmt::Debug for SplitOpenSslStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SplitOpenSslStream(Arc<Mutex<OpenSslStream>>)")
            .finish()
    }
}

impl SplitOpenSslStream {
    pub fn read_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        let guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.read_timeout()
    }

    pub fn write_timeout(&self) -> std::io::Result<Option<std::time::Duration>> {
        let guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.write_timeout()
    }

    pub fn set_write_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.set_write_timeout(dur)
    }

    pub fn set_read_timeout(&mut self, dur: Option<std::time::Duration>) -> std::io::Result<()> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.set_read_timeout(dur)
    }
}

// These struct methods form the implict contract for swappable TLS implementations
impl SplitOpenSslStream {
    pub fn local_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.inner.get_mut().local_addr()
    }

    pub fn peer_addr(&self) -> std::io::Result<Option<SocketAddr>> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.inner.get_mut().peer_addr()
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
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.inner.get_mut().shutdown(how)
    }
}

impl Clone for SplitOpenSslStream {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Read for SplitOpenSslStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.read(buf)
    }
}

impl Write for SplitOpenSslStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut guard = self.0.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Mutex poisoned: {}", e))
        })?;
        guard.flush()
    }
}

pub struct OpenSslAcceptor(openssl::ssl::SslContext);

impl OpenSslAcceptor {
    pub fn from_pem(
        certificates: Vec<u8>,
        private_key: Zeroizing<Vec<u8>>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        use openssl::pkey::PKey;
        use openssl::ssl::{self, SslVerifyMode};
        use openssl::x509::X509;

        let mut ctx = openssl::ssl::SslContext::builder(ssl::SslMethod::tls())?;
        ctx.set_cipher_list("DEFAULT")?;

        let certificate_chain = X509::stack_from_pem(&certificates)?;
        if certificate_chain.is_empty() {
            return Err("Couldn't extract certificate chain from config.".into());
        }
        // The leaf certificate must always be first in the PEM file
        ctx.set_certificate(&certificate_chain[0])?;
        for chain_cert in certificate_chain.into_iter().skip(1) {
            ctx.add_extra_chain_cert(chain_cert)?;
        }
        let key = PKey::private_key_from_pem(&private_key)?;
        ctx.set_private_key(&key)?;
        ctx.set_verify(SslVerifyMode::NONE);
        ctx.check_private_key()?;

        Ok(Self(ctx.build()))
    }

    pub fn accept(
        &self,
        stream: Connection,
    ) -> Result<OpenSslStream, Box<dyn Error + Send + Sync + 'static>> {
        use openssl::ssl::Ssl;
        let session =
            Ssl::new(&self.0).map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync + 'static>)?;
        let stream = session.accept(stream)?;
        Ok(OpenSslStream { inner: stream })
    }
}

#[must_use]
pub fn default_ssl_connector(
) -> Result<Arc<openssl::ssl::SslConnector>, Box<dyn Error + Send + Sync + 'static>> {
    let connector = openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls())?.build();
    Ok(Arc::new(connector))
}

#[derive(Clone)]
pub struct OpenSslConnector(Arc<openssl::ssl::SslConnector>);

impl OpenSslConnector {
    pub fn new() -> Self {
        Self(default_ssl_connector().expect("should get ssl connector successfully"))
    }

    pub fn create(endpoint: &Endpoint<Arc<openssl::ssl::SslConnector>>) -> Self {
        match &endpoint {
            Endpoint::WithIdentity(_, identity) => {
                Self(identity.clone())
            }
            _ => unreachable!("You generally won't call this method with Endpoint::NoIdentity since its left to you to generate")
        }
    }

    pub fn client_tls_from_endpoint(
        endpoint: &Endpoint<Arc<openssl::ssl::SslConnector>>,
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let connector = Self::create(endpoint);
        connector.from_endpoint(endpoint)
    }

    pub fn from_tcp_stream(
        &self,
        sni: String,
        plain: Connection,
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let addr = match (local_addr, peer_addr) {
            (Some(l1), Some(l2)) => Ok(DataStreamAddr::new(l1, Some(l2))),
            (Some(l1), None) => Ok(DataStreamAddr::new(l1, None)),
            (None, Some(_)) => Err(DataStreamError::NoPeerAddr),
            _ => Err(DataStreamError::NoAddr),
        }?;

        let ssl_stream = OpenSslStream {
            inner: self.0.connect(sni.as_str(), plain)?,
        };

        let split_stream = SplitOpenSslStream(Arc::new(Mutex::new(ssl_stream)));

        Ok((split_stream, addr))
    }

    pub fn from_endpoint(
        &self,
        endpoint: &Endpoint<Arc<openssl::ssl::SslConnector>>,
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
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
