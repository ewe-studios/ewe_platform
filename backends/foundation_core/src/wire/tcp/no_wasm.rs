// use derive_more::derive::From;

// use crate::io::ioutils::{self, BufferedReader, PeekError, PeekableReadStream};

// use crate::native_tls::{Identity, TlsConnector, TlsStream};
// use crate::wire::simple_http::{self};
// use core::net;
// use std::cell;
// use std::{net::TcpStream, time};

// use super::error;

// pub enum RawStream {
//     AsPlain(TcpStream, super::DataStreamAddr),
//     AsTls(BufferedReader<TlsStream<TcpStream>>, super::DataStreamAddr),
// }

// // --- Constructors

// #[allow(unused)]
// impl RawStream {
//     pub fn try_wrap_tls_with_connector<'a>(
//         plain: TcpStream,
//         connector: &'a TlsConnector,
//         sni: &str,
//     ) -> error::TlsResult<Self> {
//         let local_addr = plain.local_addr()?;
//         let peer_addr = plain.peer_addr()?;

//         let stream = connector
//             .connect(sni, plain)
//             .map_err(|_| error::TlsError::Handshake)?;

//         Ok(Self::AsTls(
//             BufferedReader::new(stream),
//             super::DataStreamAddr::new(local_addr, peer_addr),
//         ))
//     }

//     pub fn try_wrap_tls_with_identity(
//         plain: TcpStream,
//         identity: Identity,
//         sni: &str,
//     ) -> error::TlsResult<Self> {
//         let connector = TlsConnector::builder()
//             .identity(identity)
//             .build()
//             .map_err(|_| error::TlsError::ConnectorCreation)?;

//         Self::try_wrap_tls_with_connector(plain, &connector, sni)
//     }

//     pub fn try_wrap_tls(plain: TcpStream, sni: &str) -> error::TlsResult<Self> {
//         let connector = TlsConnector::new().map_err(|_| error::TlsError::ConnectorCreation)?;
//         Self::try_wrap_tls_with_connector(plain, &connector, sni)
//     }

//     #[inline]
//     pub fn try_wrap_plain(plain: TcpStream) -> error::TlsResult<Self> {
//         let local_addr = plain.local_addr()?;
//         let peer_addr = plain.peer_addr()?;
//         Ok(Self::AsPlain(
//             plain,
//             super::DataStreamAddr::new(local_addr, peer_addr),
//         ))
//     }

//     #[inline]
//     pub fn wrap_plain(plain: TcpStream) -> Self {
//         Self::try_wrap_plain(plain).expect("should wrap plain TcpStream")
//     }
// }

// // --- Methods

// #[allow(unused)]
// impl RawStream {
//     #[inline]
//     pub fn set_read_timeout(&self, duration: Option<time::Duration>) -> error::TlsResult<()> {
//         let work = match self {
//             RawStream::AsPlain(inner, _) => inner.set_read_timeout(duration),
//             RawStream::AsTls(inner, _) => {
//                 inner.get_inner_ref().get_ref().set_read_timeout(duration)
//             }
//         };

//         match work {
//             Ok(_) => Ok(()),
//             Err(err) => Err(err.into()),
//         }
//     }

//     #[inline]
//     pub fn clone_plain(&self) -> error::TlsResult<TcpStream> {
//         let work = match self {
//             RawStream::AsPlain(inner, _) => inner.try_clone(),
//             RawStream::AsTls(inner, _) => inner.get_inner_ref().get_ref().try_clone(),
//         };

//         match work {
//             Ok(inner) => Ok(inner),
//             Err(err) => Err(err.into()),
//         }
//     }

//     #[inline]
//     pub fn addrs(&self) -> super::DataStreamAddr {
//         match self {
//             RawStream::AsTls(inner, addr) => addr.clone(),
//             RawStream::AsPlain(inner, addr) => addr.clone(),
//         }
//     }

//     #[inline]
//     pub fn peer_addr(&self) -> net::SocketAddr {
//         match self {
//             RawStream::AsPlain(inner, addr) => addr.peer_addr(),
//             RawStream::AsTls(inner, addr) => addr.peer_addr(),
//         }
//     }

//     #[inline]
//     pub fn local_addr(&self) -> net::SocketAddr {
//         match self {
//             RawStream::AsPlain(inner, addr) => addr.local_addr(),
//             RawStream::AsTls(inner, addr) => addr.local_addr(),
//         }
//     }
// }

// impl PeekableReadStream for RawStream {
//     fn peek(&mut self, buf: &mut [u8]) -> simple_http::Result<usize, PeekError> {
//         match self {
//             RawStream::AsPlain(inner, _addr) => match inner.peek(buf) {
//                 Ok(count) => Ok(count),
//                 Err(err) => Err(PeekError::IOError(err)),
//             },
//             RawStream::AsTls(inner, _addr) => match inner.peek(buf) {
//                 Ok(count) => Ok(count),
//                 Err(err) => Err(err),
//             },
//         }
//     }
// }

// impl std::io::Read for RawStream {
//     #[inline]
//     fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
//         match self {
//             RawStream::AsTls(inner, _) => inner.read(buf),
//             RawStream::AsPlain(inner, _) => inner.read(buf),
//         }
//     }
// }

// impl std::io::Write for RawStream {
//     #[inline]
//     fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
//         match self {
//             RawStream::AsTls(inner, _) => inner.write(buf),
//             RawStream::AsPlain(inner, _) => inner.write(buf),
//         }
//     }

//     #[inline]
//     fn flush(&mut self) -> std::io::Result<()> {
//         match self {
//             RawStream::AsTls(inner, _) => inner.flush(),
//             RawStream::AsPlain(inner, _) => inner.flush(),
//         }
//     }
// }

// // -- Basic constructors

// impl RawStream {
//     /// from_endpoint creates a naked RawStream which is not mapped to a specific
//     /// protocol version and simply is a TCPStream connected to the relevant Endpoint
//     /// upgrade to TLS if required.
//     ///
//     /// How you take the returned RawStream is up to you but this allows you more control
//     /// on how exactly the request starts.
//     pub fn from_endpoint<T: Clone>(endpoint: super::Endpoint<T>) -> super::DataStreamResult<Self> {
//         let host = endpoint.host();

//         #[cfg(feature = "native-tls")]
//         let mut stream = {
//             let plain_stream = TcpStream::connect(host.as_str())?;
//             let encrypted_stream = if endpoint.scheme() == "https" {
//                 RawStream::try_wrap_tls(plain_stream, &endpoint.host())?
//             } else {
//                 RawStream::wrap_plain(plain_stream)
//             };
//             encrypted_stream
//         };

//         #[cfg(not(feature = "native-tls"))]
//         let mut stream = {
//             let plain_stream = TcpStream::connect(host.as_str())?;
//             RawStream::wrap_plain(plain_stream)
//         };

//         Ok(stream)
//     }
// }

// pub fn create_simple_http_reader<T: simple_http::BodyExtractor>(
//     stream: RawStream,
//     extractor: T,
// ) -> simple_http::HttpReader<T, RawStream> {
//     simple_http::HttpReader::new(crate::io::ioutils::BufferedReader::new(stream), extractor)
// }

// pub type SharedReadWriteStream<T> = std::sync::Arc<
//     std::sync::Mutex<cell::RefCell<ioutils::BufferedReader<ioutils::BufferedWriter<T>>>>,
// >;

// pub type SharedWriteStream<T> =
//     std::sync::Arc<std::sync::Mutex<cell::RefCell<ioutils::BufferedWriter<T>>>>;

// pub type SharedReadStream<T> =
//     std::sync::Arc<std::sync::Mutex<cell::RefCell<ioutils::BufferedReader<T>>>>;

// /// Attempts is a state identifying the overall expectation for
// /// when a reconnection attempt should re-occur. It is Most
// /// useful to allow the ConnectionStateIterator to be able to
// /// securely handle retries.
// #[derive(Clone, Debug)]
// pub struct ReconnectionState {
//     pub wait: Option<time::Duration>,
//     pub total_allowed: u32,
//     pub attempt: u32,
// }

// impl ReconnectionState {
//     pub fn can_retry(&self) -> bool {
//         self.attempt == self.total_allowed
//     }
// }

// /// ReconnectionDecider defines an retry mechanism that allows
// /// a central system to decide the next reconnection attempt parameters
// /// regarding how long to wait before attempt and state info on the current
// /// attempts and when such attempt to stop by returning None.
// pub trait ReconnectionDecider {
//     fn decide(&mut self, state: ReconnectionState) -> Option<ReconnectionState>;
// }

// pub trait ClonableReconnectionDecider: ReconnectionDecider {
//     fn clone_box(&self) -> Box<dyn ClonableReconnectionDecider>;
// }

// impl<T> ClonableReconnectionDecider for T
// where
//     T: ReconnectionDecider + Clone + 'static,
// {
//     fn clone_box(&self) -> Box<dyn ClonableReconnectionDecider> {
//         Box::new(self.clone())
//     }
// }

// #[derive(Clone, Debug)]
// pub struct ExponentialBackoffDecider {
//     pub factor: u32,
//     pub jitter: f32,
//     pub min_duration: time::Duration,
//     pub max_duration: time::Duration,
//     pub rng: fastrand::Rng,
// }

// const DEFAULT_JITTER: f32 = 0.6;
// const DEFAULT_FACTOR: u32 = 3;
// const DEFAULT_MIN_DURATION: time::Duration = time::Duration::from_millis(100);

// impl Default for ExponentialBackoffDecider {
//     fn default() -> Self {
//         Self::new(DEFAULT_FACTOR, DEFAULT_JITTER, DEFAULT_MIN_DURATION, None)
//     }
// }

// impl ExponentialBackoffDecider {
//     pub fn new(
//         factor: u32,
//         jitter: f32,
//         min_duration: time::Duration,
//         max_duration: impl Into<Option<time::Duration>>,
//     ) -> Self {
//         assert!(
//             jitter > 0f32 && jitter < 1f32,
//             "<exponential-backoff>: jitter must be between 0 and 1."
//         );
//         Self {
//             factor,
//             jitter,
//             min_duration,
//             rng: fastrand::Rng::new(),
//             max_duration: max_duration.into().unwrap_or(time::Duration::MAX),
//         }
//     }
// }

// impl ReconnectionDecider for ExponentialBackoffDecider {
//     fn decide(&mut self, state: ReconnectionState) -> Option<ReconnectionState> {
//         let last_attempt = state.attempt.clone();
//         if last_attempt > state.total_allowed {
//             return None;
//         }

//         let next_attempt = last_attempt.saturating_add(1);

//         // create exponential duraton
//         let exponent = self.factor.saturating_pow(next_attempt);
//         let duration = self.min_duration.saturating_mul(exponent);

//         // Apply jitter - use multiples of 100 to prevent rely on floats.
//         let jitter_factor = (self.jitter * 100f32) as u32;
//         let random = self.rng.u32(0..jitter_factor * 2);

//         let mut duration = duration.saturating_mul(100);
//         if random < jitter_factor {
//             let jitter = duration.saturating_mul(random) / 100;
//             duration = duration.saturating_sub(jitter)
//         } else {
//             let jitter = duration.saturating_mul(random / 2) / 100;
//             duration = duration.saturating_add(jitter)
//         }

//         duration /= 100;

//         // keep within boundaries
//         duration = duration.clamp(self.min_duration, self.max_duration);

//         Some(ReconnectionState {
//             wait: Some(duration),
//             attempt: next_attempt,
//             total_allowed: state.total_allowed,
//         })
//     }
// }

// /// Representing the different state a connection goes through
// /// where it can move from established to exhuasted.
// #[derive(Clone, Debug)]
// pub enum ConnectionState<T: Clone> {
//     Todo(super::Endpoint<T>),
//     Reconnect(super::Endpoint<T>, ReconnectionState),
//     Established(super::Endpoint<T>),
//     Exhausted(super::Endpoint<T>),
// }

// const DEFAULT_MAX_RETRIES: u32 = 10;

// pub struct ReconnectingStream<T: Clone> {
//     max_retries: u32,
//     state: ConnectionState<T>,
//     decider: Box<dyn ClonableReconnectionDecider>,
// }

// impl<T: Clone> ReconnectingStream<T> {
//     pub fn from_endpoint(endpoint: super::Endpoint<T>) -> Self {
//         Self {
//             max_retries: DEFAULT_MAX_RETRIES,
//             decider: Box::new(ExponentialBackoffDecider::default()),
//             state: ConnectionState::Todo(endpoint),
//         }
//     }

//     pub fn new(
//         max_retries: u32,
//         endpoint: super::Endpoint<T>,
//         decider: impl ReconnectionDecider + Clone + 'static,
//     ) -> Self {
//         Self {
//             max_retries,
//             decider: Box::new(decider),
//             state: ConnectionState::Todo(endpoint),
//         }
//     }
// }

// impl<T: Clone> Clone for ReconnectingStream<T> {
//     fn clone(&self) -> Self {
//         Self {
//             max_retries: self.max_retries.clone(),
//             decider: self.decider.clone_box(),
//             state: self.state.clone(),
//         }
//     }
// }

// #[derive(From, Debug)]
// pub enum ReconnectionError {
//     #[from(ignore)]
//     CanRetry(super::DataStreamError),

//     #[from(ignore)]
//     Failed(super::DataStreamError),
// }

// impl std::error::Error for ReconnectionError {}

// impl core::fmt::Display for ReconnectionError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{self:?}")
//     }
// }

// pub enum ReconnectionStatus {
//     Pending(time::Instant),
//     Ready(RawStream),
// }

// impl<T: Clone> Iterator for ReconnectingStream<T> {
//     type Item = Result<ReconnectionStatus, ReconnectionError>;

//     fn next(&mut self) -> Option<Self::Item> {
//         match &self.state {
//             ConnectionState::Todo(endpoint) => {
//                 // if we get called and we are at the established state then it
//                 // means reconnection is required.
//                 let reconnection_state = self.decider.decide(ReconnectionState {
//                     total_allowed: self.max_retries,
//                     attempt: 0,
//                     wait: None,
//                 });

//                 match RawStream::from_endpoint(endpoint.clone()) {
//                     Ok(connected_stream) => {
//                         self.state = ConnectionState::Established(endpoint.clone());
//                         Some(Ok(ReconnectionStatus::Ready(connected_stream)))
//                     }
//                     Err(connection_error) => {
//                         match reconnection_state {
//                             Some(state)
//                         }
//                         if state.is_none() {
//                             self.state = ConnectionState::Exhausted(endpoint.clone());
//                             return Some(Err(ReconnectionError::Failed(connection_error)));
//                         }

//                         self.state = ConnectionState::Reconnect(endpoint.clone(), state);
//                         return Some(Ok(ReconnectionStatus::Pending()));
//                     }
//                 }
//             }
//             ConnectionState::Established(endpoint) => {
//                 // if we get called and we are at the established state then it
//                 // means reconnection is required.
//                 let state = ReconnectionState {
//                     total_allowed: self.max_retries,
//                     attempt: 0,
//                     wait: None,
//                 };

//                 match RawStream::from_endpoint(endpoint.clone()) {
//                     Ok(connected_stream) => {
//                         self.state = ConnectionState::Established(endpoint.clone());
//                         Some(Ok(ReconnectionStatus::Ready(connected_stream)))
//                     }
//                     Err(connection_error) => {
//                         self.state = ConnectionState::Reconnect(endpoint.clone(), state);
//                         Some(Err(ReconnectionError::Failed(connection_error)))
//                     }
//                 }
//             }
//             ConnectionState::Reconnect(endpoint, state) => {
//                 match RawStream::from_endpoint(endpoint.clone()) {
//                     Ok(connected_stream) => {
//                         self.state = ConnectionState::Established(endpoint.clone());
//                         Some(Ok(connected_stream))
//                     }
//                     Err(connection_error) => {
//                         self.state = ConnectionState::Reconnect(endpoint.clone(), state.clone());
//                         Some(Err(ReconnectionError::Failed(connection_error)))
//                     }
//                 }
//             }
//             ConnectionState::Exhausted(_) => None,
//         }
//     }
// }
