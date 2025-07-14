//! Taken from the tiny-http project https://github.com/tiny-http/tiny-http/

#![cfg(not(target_arch = "wasm32"))]

use crate::netcap::connection::Connection;
use crate::netcap::{DataStreamAddr, Endpoint, EndpointConfig};
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

pub struct OpenSslStream {
    inner: openssl::ssl::SslStream<Connection>,
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

// These struct methods form the implict contract for swappable TLS implementations
impl SplitOpenSslStream {
    pub fn local_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0.lock().unwrap().inner.get_mut().local_addr()
    }

    pub fn peer_addr(&mut self) -> std::io::Result<Option<SocketAddr>> {
        self.0.lock().unwrap().inner.get_mut().peer_addr()
    }

    pub fn stream_addr(&mut self) -> std::io::Result<Option<DataStreamAddr>> {
        Ok(Some(DataStreamAddr::new(
            self.local_addr()?,
            self.peer_addr()?,
        )))
    }

    pub fn shutdown(&mut self, how: Shutdown) -> std::io::Result<()> {
        self.0.lock().unwrap().inner.get_mut().shutdown(how)
    }
}

impl Clone for SplitOpenSslStream {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Read for SplitOpenSslStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl Write for SplitOpenSslStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
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
        let session = Ssl::new(&self.0).expect("Failed to create new OpenSSL session");
        let stream = session.accept(stream)?;
        Ok(OpenSslStream { inner: stream })
    }
}

#[derive(Clone)]
pub struct OpenSslConnector(Arc<openssl::ssl::SslConnector>);

impl OpenSslConnector {
    pub fn create(endpoint: &Endpoint<Arc<openssl::ssl::SslConnector>>) -> Self {
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
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let ssl_stream = self.0.connect(sni, plain)?;

        Ok(SplitOpenSslStream(Arc::new(Mutex::new(ssl_stream))))
    }

    pub fn from_endpoint<T: Clone>(
        &self,
        endpoint: Endpoint<T>,
    ) -> Result<(SplitOpenSslStream, DataStreamAddr), Box<dyn Error + Send + Sync + 'static>> {
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
