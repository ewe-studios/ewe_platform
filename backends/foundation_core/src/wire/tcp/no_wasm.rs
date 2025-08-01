#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::missing_errors_doc)]

use derive_more::derive::From;

#[cfg(feature = "native-tls")]
use crate::io::ioutils::BufferedReader;

use crate::io::ioutils::{PeekError, PeekableReadStream};
use crate::retries::{
    CloneableReconnectionDecider, ExponentialBackoffDecider, RetryDecider, RetryState,
};
use crate::valtron::delayed_iterators::Delayed;
use crate::valtron::delayed_iterators::{DelayedIterator, SleepIterator};

#[cfg(feature = "native-tls")]
use crate::native_tls::{Identity, TlsConnector, TlsStream};

use crate::wire::simple_http::{self};
use core::net;
use std::net::SocketAddr;
use std::time::Duration;
use std::{net::TcpStream, time};

use super::error;

pub enum RawStream {
    AsPlain(TcpStream, super::DataStreamAddr),

    #[cfg(feature = "native-tls")]
    AsTls(BufferedReader<TlsStream<TcpStream>>, super::DataStreamAddr),
}

// --- Constructors

#[allow(unused)]
impl RawStream {
    /// Returns a fallible TLS connection from the provided `TcpStream` and connector.
    #[cfg(feature = "native-tls")]
    pub fn try_wrap_tls_with_connector(
        plain: TcpStream,
        connector: &TlsConnector,
        sni: &str,
    ) -> error::TlsResult<Self> {
        let local_addr = plain.local_addr()?;
        let peer_addr = plain.peer_addr()?;

        let stream = connector
            .connect(sni, plain)
            .map_err(|_| error::TlsError::Handshake)?;

        Ok(Self::AsTls(
            BufferedReader::new(stream),
            super::DataStreamAddr::new(local_addr, peer_addr),
        ))
    }

    #[cfg(feature = "native-tls")]
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

    #[cfg(feature = "native-tls")]
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
            #[cfg(feature = "native-tls")]
            RawStream::AsTls(inner, _) => {
                inner.get_inner_ref().get_ref().set_read_timeout(duration)
            }
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
            #[cfg(feature = "native-tls")]
            RawStream::AsTls(inner, _) => inner.get_inner_ref().get_ref().try_clone(),
        };

        match work {
            Ok(inner) => Ok(inner),
            Err(err) => Err(err.into()),
        }
    }

    #[inline]
    pub fn addrs(&self) -> super::DataStreamAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.clone(),
            #[cfg(feature = "native-tls")]
            RawStream::AsTls(inner, addr) => addr.clone(),
        }
    }

    #[inline]
    pub fn peer_addr(&self) -> net::SocketAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.peer_addr(),
            #[cfg(feature = "native-tls")]
            RawStream::AsTls(inner, addr) => addr.peer_addr(),
        }
    }

    #[inline]
    pub fn local_addr(&self) -> net::SocketAddr {
        match self {
            RawStream::AsPlain(inner, addr) => addr.local_addr(),
            #[cfg(feature = "native-tls")]
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
            #[cfg(feature = "native-tls")]
            Self::AsTls(_, addr) => f
                .debug_tuple("RawStream::TLS")
                .field(&"_")
                .field(addr)
                .finish(),
        }
    }
}

impl PeekableReadStream for RawStream {
    fn peek(&mut self, buf: &mut [u8]) -> simple_http::Result<usize, PeekError> {
        match self {
            RawStream::AsPlain(inner, _addr) => match inner.peek(buf) {
                Ok(count) => Ok(count),
                Err(err) => Err(PeekError::IOError(err)),
            },
            #[cfg(feature = "native-tls")]
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
            #[cfg(feature = "native-tls")]
            RawStream::AsTls(inner, _) => inner.read(buf),
        }
    }
}

impl std::io::Write for RawStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            RawStream::AsPlain(inner, _) => inner.write(buf),
            #[cfg(feature = "native-tls")]
            RawStream::AsTls(inner, _) => inner.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            RawStream::AsPlain(inner, _) => inner.flush(),
            #[cfg(feature = "native-tls")]
            RawStream::AsTls(inner, _) => inner.flush(),
        }
    }
}

// -- Basic constructors

impl RawStream {
    /// from_endpoint_timeout creates a naked RawStream which is not mapped to a specific
    /// protocol version and simply is a TCPStream connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned RawStream is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_endpoint_timeout<T: Clone>(
        endpoint: super::Endpoint<T>,
        timeout: time::Duration,
    ) -> super::DataStreamResult<Self> {
        let host = endpoint.host();

        let host_socket_addr: SocketAddr = host.parse()?;

        #[cfg(feature = "native-tls")]
        let stream = {
            let plain_stream = TcpStream::connect_timeout(&host_socket_addr, timeout)?;
            if endpoint.scheme() == "https" {
                RawStream::try_wrap_tls(plain_stream, &endpoint.host())?
            } else {
                RawStream::wrap_plain(plain_stream)
            }
        };

        #[cfg(not(feature = "native-tls"))]
        let stream = {
            let plain_stream = TcpStream::connect_timeout(&host_socket_addr, timeout)?;
            RawStream::wrap_plain(plain_stream)
        };

        Ok(stream)
    }
    /// from_endpoint creates a naked RawStream which is not mapped to a specific
    /// protocol version and simply is a TCPStream connected to the relevant Endpoint
    /// upgrade to TLS if required.
    ///
    /// How you take the returned RawStream is up to you but this allows you more control
    /// on how exactly the request starts.
    pub fn from_endpoint<T: Clone>(endpoint: super::Endpoint<T>) -> super::DataStreamResult<Self> {
        Self::from_endpoint_timeout(endpoint, Duration::from_micros(0))
    }
}

pub fn create_simple_http_reader<T: simple_http::BodyExtractor>(
    stream: RawStream,
    extractor: T,
) -> simple_http::HttpReader<T, RawStream> {
    simple_http::HttpReader::new(crate::io::ioutils::BufferedReader::new(stream), extractor)
}

/// Representing the different state a connection goes through
/// where it can move from established to exhuasted.
#[derive(Clone, Debug)]
pub enum ConnectionState<T: Clone> {
    Todo(super::Endpoint<T>),
    Redo(super::Endpoint<T>, RetryState),
    Reconnect(RetryState, Option<SleepIterator<super::Endpoint<T>>>),
    Established(super::Endpoint<T>),
    Exhausted(super::Endpoint<T>),
}

const DEFAULT_MAX_RETRIES: u32 = 10;

pub struct ReconnectingStream<T: Clone> {
    max_retries: u32,
    state: ConnectionState<T>,
    connection_timeout: time::Duration,
    decider: Box<dyn CloneableReconnectionDecider>,
}

impl<T: Clone> ReconnectingStream<T> {
    pub fn from_endpoint(endpoint: super::Endpoint<T>) -> Self {
        static CONNECTION_TIMEOUT: time::Duration = time::Duration::from_millis(600);

        Self::new(
            DEFAULT_MAX_RETRIES,
            endpoint,
            CONNECTION_TIMEOUT,
            ExponentialBackoffDecider::default(),
        )
    }

    pub fn with_connection_timeout(
        endpoint: super::Endpoint<T>,
        connection_timeout: time::Duration,
    ) -> Self {
        Self::new(
            DEFAULT_MAX_RETRIES,
            endpoint,
            connection_timeout,
            ExponentialBackoffDecider::default(),
        )
    }

    pub fn with_duration(
        max_retries: u32,
        endpoint: super::Endpoint<T>,
        connection_timeout: time::Duration,
        min_duration: time::Duration,
        max_duration: impl Into<Option<time::Duration>>,
    ) -> Self {
        Self::new(
            max_retries,
            endpoint,
            connection_timeout,
            ExponentialBackoffDecider::from_duration(min_duration, max_duration),
        )
    }

    pub fn new(
        max_retries: u32,
        endpoint: super::Endpoint<T>,
        connection_timeout: time::Duration,
        decider: impl RetryDecider + Clone + 'static,
    ) -> Self {
        Self {
            max_retries,
            connection_timeout,
            decider: Box::new(decider),
            state: ConnectionState::Todo(endpoint),
        }
    }
}

impl<T: Clone> Clone for ReconnectingStream<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            max_retries: self.max_retries,
            decider: self.decider.clone_box(),
            connection_timeout: self.connection_timeout,
        }
    }
}

#[derive(From, Debug)]
pub enum ReconnectionError {
    UnexpectedRetryState,

    NoMoreRetries,

    #[from(ignore)]
    CanRetry(super::DataStreamError),

    #[from(ignore)]
    Failed(super::DataStreamError),
}

impl PartialEq for ReconnectionError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::UnexpectedRetryState, Self::UnexpectedRetryState)
                | (Self::NoMoreRetries, Self::NoMoreRetries)
                | (Self::CanRetry(_), Self::CanRetry(_))
                | (Self::Failed(_), Self::Failed(_))
        )
    }
}

impl std::error::Error for ReconnectionError {}

impl core::fmt::Display for ReconnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug)]
pub enum ReconnectionStatus {
    Waiting(std::time::Duration),
    NoMoreWaiting,
    Ready(RawStream),
}

impl Eq for ReconnectionStatus {}

impl PartialEq for ReconnectionStatus {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ReconnectionStatus::Waiting(m1), ReconnectionStatus::Waiting(m2)) => m1 == m2,
            (ReconnectionStatus::Ready(_), ReconnectionStatus::Ready(_)) => true,
            (ReconnectionStatus::NoMoreWaiting, ReconnectionStatus::NoMoreWaiting) => true,
            _ => false,
        }
    }
}

impl<T: Clone> Iterator for ReconnectingStream<T> {
    type Item = Result<ReconnectionStatus, ReconnectionError>;

    #[allow(clippy::too_many_lines)]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            ConnectionState::Todo(endpoint) => {
                // if we get called and we are at the established state then it
                // means reconnection is required.
                let reconnection_state_option = self.decider.decide(RetryState {
                    total_allowed: self.max_retries,
                    attempt: 0,
                    wait: None,
                });

                match RawStream::from_endpoint_timeout(endpoint.clone(), self.connection_timeout) {
                    Ok(connected_stream) => {
                        self.state = ConnectionState::Established(endpoint.clone());
                        Some(Ok(ReconnectionStatus::Ready(connected_stream)))
                    }
                    Err(connection_error) => {
                        println!("Failed to connect: {connection_error:?}");
                        match reconnection_state_option {
                            Some(rstate) => {
                                let duration = rstate.wait.unwrap_or(Duration::from_secs(0));

                                let sleeper = SleepIterator::until(duration, endpoint.clone());
                                self.state = ConnectionState::Reconnect(rstate, Some(sleeper));
                                Some(Ok(ReconnectionStatus::Waiting(duration)))
                            }
                            None => {
                                self.state = ConnectionState::Exhausted(endpoint.clone());
                                Some(Err(ReconnectionError::Failed(connection_error)))
                            }
                        }
                    }
                }
            }
            ConnectionState::Redo(endpoint, last_state) => {
                // if we get called and we are at the established state then it
                // means reconnection is required.
                let reconnection_state_option = self.decider.decide(last_state.clone());

                match RawStream::from_endpoint(endpoint.clone()) {
                    Ok(connected_stream) => {
                        self.state = ConnectionState::Established(endpoint.clone());
                        Some(Ok(ReconnectionStatus::Ready(connected_stream)))
                    }
                    Err(connection_error) => match reconnection_state_option {
                        Some(rstate) => {
                            let duration = rstate.wait.unwrap_or(Duration::from_secs(0));

                            let sleeper = SleepIterator::until(duration, endpoint.clone());
                            self.state = ConnectionState::Reconnect(rstate, Some(sleeper));
                            Some(Ok(ReconnectionStatus::Waiting(duration)))
                        }
                        None => {
                            self.state = ConnectionState::Exhausted(endpoint.clone());
                            Some(Err(ReconnectionError::Failed(connection_error)))
                        }
                    },
                }
            }
            ConnectionState::Established(endpoint) => {
                // if we get called and we are at the established state then it
                // means reconnection is required.
                let reconnection_state = self.decider.decide(RetryState {
                    total_allowed: self.max_retries,
                    attempt: 0,
                    wait: None,
                });

                match reconnection_state {
                    Some(rstate) => {
                        let duration = match rstate.wait {
                            Some(duration) => duration,
                            None => Duration::from_secs(0),
                        };

                        let sleeper = SleepIterator::until(duration, endpoint.clone());
                        self.state = ConnectionState::Reconnect(rstate, Some(sleeper));
                        Some(Ok(ReconnectionStatus::Waiting(duration)))
                    }
                    None => {
                        self.state = ConnectionState::Exhausted(endpoint.clone());
                        Some(Err(ReconnectionError::NoMoreRetries))
                    }
                }
            }
            ConnectionState::Reconnect(rstate, sleeper_container) => {
                match sleeper_container.take() {
                    Some(mut sleeper) => match sleeper.next() {
                        Some(delayed_state) => match delayed_state {
                            Delayed::Pending(_, _, remaining_dur) => {
                                self.state =
                                    ConnectionState::Reconnect(rstate.clone(), Some(sleeper));
                                Some(Ok(ReconnectionStatus::Waiting(remaining_dur)))
                            }
                            Delayed::Done(endpoint) => {
                                self.state = ConnectionState::Redo(endpoint, rstate.clone());
                                Some(Ok(ReconnectionStatus::NoMoreWaiting))
                            }
                        },
                        None => unreachable!(
                            "should never occur as we will stop once Delayed::Done() is reached"
                        ),
                    },
                    None => unreachable!("we should never have a Reconnect with no sleeper"),
                }
            }
            ConnectionState::Exhausted(_) => None,
        }
    }
}

#[cfg(test)]
mod test_reconnection_stream {

    use crate::{panic_if_failed, retries::SameBackoffDecider, wire::tcp::Endpoint};
    use std::{net::TcpListener, result::Result, thread};
    use tracing;

    use super::*;

    #[test]
    fn successfully_connects_on_first_try() {
        let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:3799"));
        let threader = thread::spawn(move || {
            let _ = listener.accept();
            tracing::debug!("Received client, ending");
        });

        let endpoint = panic_if_failed!(Endpoint::plain_string("http://127.0.0.1:3799"));
        let mut stream = ReconnectingStream::new(
            2,
            endpoint,
            Duration::from_millis(500),
            SameBackoffDecider::new(Duration::from_millis(200)),
        );

        let collected: Option<Result<ReconnectionStatus, ReconnectionError>> = stream.next();
        dbg!(&collected);

        assert!(matches!(collected, Some(Ok(ReconnectionStatus::Ready(_)))));

        threader.join().expect("closed");
    }

    #[test]
    fn fails_reconnection_after_max_retries() {
        let endpoint = panic_if_failed!(Endpoint::plain_string("http://localhost:8899"));
        let stream = ReconnectingStream::new(
            2,
            endpoint,
            Duration::from_millis(50),
            SameBackoffDecider::new(Duration::from_millis(200)),
        );

        let collected: Vec<Result<ReconnectionStatus, ReconnectionError>> = stream
            .filter(|item| match item {
                Ok(inner) => match inner {
                    ReconnectionStatus::Waiting(duration) => {
                        if duration == &Duration::from_millis(200) {
                            return true;
                        }
                        false
                    }
                    _ => true,
                },
                Err(_) => true,
            })
            .collect();

        dbg!(&collected);

        assert_eq!(
            collected[0..collected.len() - 1],
            vec![
                Ok(ReconnectionStatus::Waiting(Duration::from_millis(200))),
                Ok(ReconnectionStatus::NoMoreWaiting),
                Ok(ReconnectionStatus::Waiting(Duration::from_millis(200))),
                Ok(ReconnectionStatus::NoMoreWaiting),
            ]
        );

        assert!(matches!(collected[4], Err(ReconnectionError::Failed(_))));
    }
}
