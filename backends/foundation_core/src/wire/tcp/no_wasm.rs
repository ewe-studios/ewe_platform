use crate::io::ioutils;

use crate::native_tls::{Identity, TlsConnector, TlsStream};
use core::net;
use std::{
    collections::HashMap,
    io::{BufRead, Write},
    net::TcpStream,
    time,
};

use super::error;

pub enum RawStream {
    AsPlain(TcpStream, super::DataStreamAddr),
    AsTls(TlsStream<TcpStream>, super::DataStreamAddr),
}

// --- Constructors

#[allow(unused)]
impl RawStream {
    pub fn try_wrap_tls_with_connector<'a>(
        plain: TcpStream,
        connector: &'a TlsConnector,
        sni: &str,
    ) -> error::TlsResult<Self> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let stream = connector
            .connect(sni, plain)
            .map_err(|_| error::TlsError::Handshake)?;

        Ok(Self::AsTls(
            stream,
            super::DataStreamAddr::new(local_addr, peer_addr),
        ))
    }

    pub fn try_wrap_tls_with_identity(
        plain: TcpStream,
        identity: Identity,
        sni: &str,
    ) -> error::TlsResult<Self> {
        let connector = TlsConnector::builder()
            .identity(identity)
            .build()
            .map_err(|_| error::TlsError::ConnectorCreation)?;

        Self::try_wrap_tls_with_connector(plain, &connector, sni)
    }

    pub fn try_wrap_tls(plain: TcpStream, sni: &str) -> error::TlsResult<Self> {
        let connector = TlsConnector::new().map_err(|_| error::TlsError::ConnectorCreation)?;
        Self::try_wrap_tls_with_connector(plain, &connector, sni)
    }

    #[inline]
    pub fn try_wrap_plain(plain: TcpStream) -> error::TlsResult<Self> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;
        Ok(Self::AsPlain(
            plain,
            super::DataStreamAddr::new(local_addr, peer_addr),
        ))
    }

    #[inline]
    pub fn wrap_plain(plain: TcpStream) -> Self {
        Self::try_wrap_plain(plain).expect("should wrap plain TcpStream")
    }
}

// --- Methods

#[allow(unused)]
impl RawStream {
    #[inline]
    pub fn set_read_timeout(&self, duration: Option<time::Duration>) -> error::TlsResult<()> {
        let work = match self {
            RawStream::AsPlain(inner, _) => inner.set_read_timeout(duration),
            RawStream::AsTls(inner, _) => inner.get_ref().set_read_timeout(duration),
        };

        match work {
            Ok(_) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    #[inline]
    pub fn clone_plain(&self) -> error::TlsResult<TcpStream> {
        let work = match self {
            RawStream::AsPlain(inner, _) => inner.try_clone(),
            RawStream::AsTls(inner, _) => inner.get_ref().try_clone(),
        };

        match work {
            Ok(inner) => Ok(inner),
            Err(err) => Err(err.into()),
        }
    }

    #[inline]
    pub fn addrs(&self) -> super::DataStreamAddr {
        match self {
            RawStream::AsTls(inner, addr) => addr.clone(),
            RawStream::AsPlain(inner, addr) => addr.clone(),
        }
    }

    #[inline]
    pub fn peer_addr(&self) -> net::SocketAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.peer_addr(),
            RawStream::AsTls(inner, addr) => addr.peer_addr(),
        }
    }

    #[inline]
    pub fn local_addr(&self) -> net::SocketAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.local_addr(),
            RawStream::AsTls(inner, addr) => addr.local_addr(),
        }
    }
}

impl std::io::Read for RawStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsTls(inner, _) => inner.read(buf),
            RawStream::AsPlain(inner, _) => inner.read(buf),
        }
    }
}

impl std::io::Write for RawStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsTls(inner, _) => inner.write(buf),
            RawStream::AsPlain(inner, _) => inner.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            RawStream::AsTls(inner, _) => inner.flush(),
            RawStream::AsPlain(inner, _) => inner.flush(),
        }
    }
}

// -- Basic constructors

#[allow(unused)]
impl RawStream {
    /// from_endpoint creates a naked RawStream which is not mapped to a specific
    /// protocol version and simply is a TCPStream connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned RawStream is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_endpoint<T: Clone>(endpoint: super::Endpoint<T>) -> super::DataStreamResult<Self> {
        let host = endpoint.host();

        #[cfg(feature = "native-tls")]
        let mut stream = {
            let plain_stream = TcpStream::connect(host.as_str())?;
            let encrypted_stream = if endpoint.scheme() == "https" {
                RawStream::try_wrap_tls(plain_stream, &endpoint.host())?
            } else {
                RawStream::wrap_plain(plain_stream)
            };
            encrypted_stream
        };

        #[cfg(not(feature = "native-tls"))]
        let mut stream = {
            let plain_stream = TcpStream::connect(host.as_str())?;
            RawStream::wrap_plain(plain_stream)
        };

        Ok(stream)
    }
}

// -- Protocol constructors

pub struct ProtocolStream<T: Clone>(RawStream, super::Endpoint<T>);

impl<T: Clone> std::io::Read for ProtocolStream<T> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<T: Clone> std::io::Write for ProtocolStream<T> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

#[allow(unused)]
impl<T: Clone> ProtocolStream<T> {
    pub fn endpoint(&self) -> super::Endpoint<T> {
        self.1.clone()
    }

    // get_mut_stream_ref returns a reference to the internal stream.
    pub fn get_stream_ref(&self) -> &RawStream {
        &self.0
    }

    // get_mut_stream_ref returns a mutable reference to the internal stream.
    pub fn get_mut_stream_ref(&mut self) -> &mut RawStream {
        &mut self.0
    }
}

#[allow(unused)]
impl ProtocolStream<()> {
    pub fn http1<S>(
        endpoint: super::Endpoint<()>,
        request: S,
    ) -> super::DataStreamResult<Http1Stream<()>>
    where
        S: Into<String>,
    {
        Self::with_http1(endpoint, request)
    }

    pub fn http2<S>(endpoint: super::Endpoint<()>, request: S) -> super::DataStreamResult<Self>
    where
        S: Into<String>,
    {
        Self::with_http2(endpoint, request)
    }
}

#[allow(unused)]
impl<T: Clone> ProtocolStream<T> {
    /// creates a http1 wrapped TcpStream stream from the provided endpoint
    /// allowing you to communicate across the HTTP 1 protocol.
    pub fn with_http1<S>(
        endpoint: super::Endpoint<T>,
        request: S,
    ) -> super::DataStreamResult<Http1Stream<T>>
    where
        S: Into<String>,
    {
        let stream = RawStream::from_endpoint::<T>(endpoint.clone())?;
        let mut http1_stream = Http1Stream::new(Self(stream, endpoint));
        http1_stream.with_request(request.into());
        http1_stream.fetch_headers()?;

        return Ok(http1_stream);
    }

    /// creates a http2 wrapped TcpStream stream from the provided endpoint
    /// allowing you to communicate across the HTTP 2 protocol.
    pub fn with_http2<S>(endpoint: super::Endpoint<T>, request: S) -> super::DataStreamResult<Self>
    where
        S: Into<String>,
    {
        unimplemented!()
    }
}

pub struct Http1Stream<T: Clone> {
    addr: super::DataStreamAddr,
    endpoint: super::Endpoint<T>,
    headers: Option<HashMap<String, String>>,
    stream: ioutils::BufferedStream<ProtocolStream<T>>,
}

// -- constructors

impl<T: Clone> Http1Stream<T> {
    pub fn new(stream: ProtocolStream<T>) -> Self {
        let stream_endpoint = stream.endpoint();
        let stream_addr = stream.get_stream_ref().addrs();
        let buffered_stream = ioutils::buffered_stream(stream);

        Self {
            headers: None,
            addr: stream_addr,
            stream: buffered_stream,
            endpoint: stream_endpoint,
        }
    }
}

// -- Function parks
impl<T: Clone> Http1Stream<T> {
    pub(crate) fn read_till_line(&mut self) -> super::DataStreamResult<String> {
        let mut status_line = String::new();
        loop {
            self.stream.read_line(&mut status_line)?;
            if status_line == "" {
                continue;
            }
            return Ok(status_line);
        }
    }

    pub(crate) fn fetch_status(&mut self) -> super::DataStreamResult<()> {
        // let status_line = self.read_till_line()?;

        // // pull
        // let protocol_part = &status_line[..9].trim_end();
        // Ok(())
        todo!()
    }

    pub(crate) fn fetch_headers(&mut self) -> super::DataStreamResult<()> {
        Ok(())
    }

    /// with_request consumes the stream setting up the stream the
    /// starting communication to the underlying stream for your use.
    ///
    /// You can only really call this
    pub fn with_request(&mut self, request: String) -> super::DataStreamResult<()> {
        let request_as_bytes = request.as_bytes();
        self.stream.write(request_as_bytes)?;
        self.stream.flush()?;
        Ok(())
    }
}

// impl super::DataStream for ConnectedTcpStream {
//     type Headers = HashMap<String, String>;
//     type Body
// }
