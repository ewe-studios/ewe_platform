use core::time;
use std::sync::Arc;
use std::time::Duration;

use crate::netcap::{ClientEndpoint, DataStreamError, RawStream};
use crate::retries::{CReconnectionDecider, ExponentialBackoffDecider, RetryDecider, RetryState};
use crate::valtron::delayed_iterators::Delayed;
use crate::valtron::delayed_iterators::{DelayedIterator, SleepIterator};
use derive_more::derive::From;

use super::simple_http;
use crate::compati::Mutex;

pub fn create_simple_http_reader<T: simple_http::BodyExtractor>(
    stream: RawStream,
    extractor: T,
) -> simple_http::HttpReader<T, RawStream> {
    simple_http::HttpReader::new(crate::io::ioutils::BufferedReader::new(stream), extractor)
}

/// Representing the different state a connection goes through
/// where it can move from established to exhausted.
#[derive(Clone, Debug)]
pub enum ConnectionState {
    Todo(ClientEndpoint),
    Redo(ClientEndpoint, RetryState),
    Reconnect(RetryState, Option<SleepIterator<ClientEndpoint>>),
    Established(ClientEndpoint),
    Exhausted(ClientEndpoint),
}

const DEFAULT_MAX_RETRIES: u32 = 10;

pub struct ReconnectingStream {
    max_retries: u32,
    state: Arc<Mutex<ConnectionState>>,
    connection_timeout: time::Duration,
    decider: Box<dyn CReconnectionDecider>,
}

impl ReconnectingStream {
    pub fn from_endpoint(endpoint: ClientEndpoint) -> Self {
        static CONNECTION_TIMEOUT: time::Duration = time::Duration::from_millis(600);

        Self::new(
            DEFAULT_MAX_RETRIES,
            endpoint,
            CONNECTION_TIMEOUT,
            ExponentialBackoffDecider::default(),
        )
    }

    pub fn with_connection_timeout(
        endpoint: ClientEndpoint,
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
        endpoint: ClientEndpoint,
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
        endpoint: ClientEndpoint,
        connection_timeout: time::Duration,
        decider: impl RetryDecider + Clone + 'static,
    ) -> Self {
        Self {
            max_retries,
            connection_timeout,
            decider: Box::new(decider),
            state: Arc::new(Mutex::new(ConnectionState::Todo(endpoint))),
        }
    }
}

impl Clone for ReconnectingStream {
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
    CanRetry(DataStreamError),

    #[from(ignore)]
    Failed(DataStreamError),
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
    NoMoreWaiting,
    Ready(Box<RawStream>),
    Waiting(std::time::Duration),
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

impl Iterator for ReconnectingStream {
    type Item = Result<ReconnectionStatus, ReconnectionError>;

    #[allow(clippy::too_many_lines)]
    fn next(&mut self) -> Option<Self::Item> {
        let mut current_state = self.state.lock().unwrap();
        match &mut *current_state {
            ConnectionState::Todo(endpoint) => {
                // if we get called and we are at the established state then it
                // means reconnection is required.
                let reconnection_state_option = self.decider.decide(RetryState {
                    total_allowed: self.max_retries,
                    attempt: 0,
                    wait: None,
                });

                match RawStream::from_endpoint(endpoint) {
                    Ok(connected_stream) => {
                        *current_state = ConnectionState::Established(endpoint.clone());
                        Some(Ok(ReconnectionStatus::Ready(Box::new(connected_stream))))
                    }
                    Err(connection_error) => match reconnection_state_option {
                        Some(rstate) => {
                            let duration = rstate.wait.unwrap_or(Duration::from_secs(0));

                            let sleeper = SleepIterator::until(duration, endpoint.clone());
                            *current_state = ConnectionState::Reconnect(rstate, Some(sleeper));
                            Some(Ok(ReconnectionStatus::Waiting(duration)))
                        }
                        None => {
                            *current_state = ConnectionState::Exhausted(endpoint.clone());
                            Some(Err(ReconnectionError::Failed(connection_error)))
                        }
                    },
                }
            }
            ConnectionState::Redo(endpoint, last_state) => {
                // if we get called and we are at the established state then it
                // means reconnection is required.
                let reconnection_state_option = self.decider.decide(last_state.clone());

                match RawStream::from_endpoint(endpoint) {
                    Ok(connected_stream) => {
                        *current_state = ConnectionState::Established(endpoint.clone());
                        Some(Ok(ReconnectionStatus::Ready(Box::new(connected_stream))))
                    }
                    Err(connection_error) => match reconnection_state_option {
                        Some(rstate) => {
                            let duration = rstate.wait.unwrap_or(Duration::from_secs(0));

                            let sleeper = SleepIterator::until(duration, endpoint.clone());
                            *current_state = ConnectionState::Reconnect(rstate, Some(sleeper));
                            Some(Ok(ReconnectionStatus::Waiting(duration)))
                        }
                        None => {
                            *current_state = ConnectionState::Exhausted(endpoint.clone());
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
                        *current_state = ConnectionState::Reconnect(rstate, Some(sleeper));
                        Some(Ok(ReconnectionStatus::Waiting(duration)))
                    }
                    None => {
                        *current_state = ConnectionState::Exhausted(endpoint.clone());
                        Some(Err(ReconnectionError::NoMoreRetries))
                    }
                }
            }
            ConnectionState::Reconnect(rstate, sleeper_container) => {
                match sleeper_container.take() {
                    Some(mut sleeper) => match sleeper.next() {
                        Some(delayed_state) => match delayed_state {
                            Delayed::Pending(_, _, remaining_dur) => {
                                *current_state =
                                    ConnectionState::Reconnect(rstate.clone(), Some(sleeper));
                                Some(Ok(ReconnectionStatus::Waiting(remaining_dur)))
                            }
                            Delayed::Done(endpoint) => {
                                *current_state = ConnectionState::Redo(endpoint, rstate.clone());
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

    use crate::{netcap::Endpoint, panic_if_failed, retries::SameBackoffDecider};
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

        let endpoint = ClientEndpoint::Plain(panic_if_failed!(Endpoint::with_string(
            "http://127.0.0.1:3799"
        )));
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
        let endpoint = ClientEndpoint::Plain(panic_if_failed!(Endpoint::with_string(
            "http://127.0.0.1:8899"
        )));
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
