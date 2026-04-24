//! HTTP request task implementation using `TaskIterator` pattern.
//!
//! WHY: Provides a non-blocking, state-machine-based HTTP request executor that
//! integrates with the valtron executor system. Enables async-like request handling
//! without async/await.
//!
//! WHAT: Implements `HttpRequestTask` which processes HTTP requests through a series
//! of states (connecting, sending request, receiving response).
//! Uses `TaskIterator` trait to yield `TaskStatus` variants.
//!
//! HOW: State machine pattern where each `next()` call advances through states.
//! Phase 1 uses blocking connection for simplicity. Future phases will use
//! non-blocking connection spawning and TLS support.
//!
//! PHASE 1 SCOPE: HTTP-only (no HTTPS), blocking connection, basic GET requests.
//! PHASE 2 SCOPE: HTTPS support, non-blocking connection, advanced request handling.

use crate::valtron::{
    drive_receiver, inlined_task, BoxedSendExecutionAction, DrivenRecvIterator, InlineSendAction,
    IntoBoxedSendExecutionAction, TaskIterator, TaskStatus,
};
use crate::wire::simple_http::client::{
    redirects, ClientConfig, DnsResolver, HttpConnectionPool, PreparedRequest,
};
use crate::wire::simple_http::url::Uri;
use crate::wire::simple_http::{
    HttpClientError, IncomingResponseParts, SendSafeBody, SimpleHeader, SimpleIncomingRequest,
    SimpleMethod, Status,
};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;

use super::{
    GetHttpRequestRedirectTask, GetRequestIntroTask, HttpRequestRedirectResponse, RequestIntro,
    SendRequest,
};

pub enum SendRequestState<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Init starts out with the request based on the provided `GetHttpRequestStreamInner`
    /// which then moves to [`Self::Connecting`] and then `Self::Pull`
    /// to get the actual request response.
    Init(Option<Box<SendRequest<R>>>),

    /// `Connecting` contains the read iterator to read the  response from the connection.
    Connecting(DrivenRecvIterator<GetHttpRequestRedirectTask<R>>),

    /// `Reading` reads the introduction information from (Status + Headers) from the connection.
    Reading(DrivenRecvIterator<GetRequestIntroTask>),

    /// `SkipReading` skips the attempt to read the intro from the stream
    /// as indicates the request intro has just being read already
    /// and provides the request response.
    SkipReading(Box<Option<RequestIntro>>),

    /// `CheckRedirect` checks if the response is a redirect and if so,
    /// moves to [`Self::Connecting`] to follow the redirect.
    CheckRedirect(Box<Option<RequestIntro>>),

    /// No more work to do
    Done,
}

pub struct SendRequestTask<R>(
    Option<SendRequestState<R>>,
    ClientConfig,
    Arc<HttpConnectionPool<R>>,
    Uri,
)
where
    R: DnsResolver + Send + 'static;

impl<R> SendRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    #[must_use]
    pub fn new(
        request: PreparedRequest,
        max_redirects: u8,
        pool: Arc<HttpConnectionPool<R>>,
        config: crate::wire::simple_http::client::ClientConfig,
    ) -> Self {
        let parsed_uri = request.url.clone();
        Self(
            Some(SendRequestState::Init(Some(Box::new(SendRequest::new(
                request,
                max_redirects,
                pool.clone(),
                config.clone(),
            ))))),
            config,
            pool,
            parsed_uri,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub enum HttpRequestPending {
    WaitingForStream,
    CheckRedirectResponse,
    WaitingIntroAndHeaders,
}

impl<R> TaskIterator for SendRequestTask<R>
where
    R: DnsResolver + Send + 'static,
{
    type Ready = RequestIntro;
    type Pending = HttpRequestPending;
    type Spawner = BoxedSendExecutionAction;

    #[allow(clippy::too_many_lines)]
    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.0.take()? {
            SendRequestState::Init(mut inner) => match inner.take() {
                Some(boxed_req) => {
                    let send_request = *boxed_req;
                    if send_request.request.is_none() {
                        tracing::warn!("Request is missing");

                        self.0 = Some(SendRequestState::Done);

                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpClientError::ReadError,
                        )));
                    }

                    let Some(incoming_request) = send_request.request else {
                        self.0 = Some(SendRequestState::Done);
                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpClientError::NoRequestToSend,
                        )));
                    };

                    // if let Some(headers_to_add) = &self.1.headers_to_add {
                    //     tracing::debug!(
                    //         "CheckRedirect: Adding headers to req: {:?}",
                    //         headers_to_add
                    //     );
                    //     for (key, values) in headers_to_add.iter() {
                    //         for value in values {
                    //             incoming_request.headers = incoming_request
                    //                 .headers
                    //                 .add_header(key.clone(), value.clone());
                    //         }
                    //     }
                    // }

                    let Ok(into_incoming) = incoming_request.into_simple_incoming_request() else {
                        self.0 = Some(SendRequestState::Done);

                        return Some(TaskStatus::Ready(RequestIntro::Failed(
                            HttpClientError::ReadError,
                        )));
                    };

                    let (get_stream_action, get_stream_receiver) = inlined_task(
                        crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                        Vec::new(),
                        GetHttpRequestRedirectTask::new(
                            into_incoming,
                            send_request.pool.clone(),
                            send_request.config.clone(),
                            send_request.remaining_redirects,
                        ),
                        self.1.inline_processing_timeout,
                    );

                    self.0 = Some(SendRequestState::Connecting(get_stream_receiver));

                    tracing::debug!(
                        "HttpRequestTaskState::Init: Spawned task to get HTTP request stream"
                    );
                    Some(TaskStatus::Spawn(
                        get_stream_action.into_box_send_execution_action(),
                    ))
                }
                None => unreachable!("Task state must never get here"),
            },
            SendRequestState::Connecting(mut recv_iter) => {
                let next_value = recv_iter.next();

                self.0 = Some(SendRequestState::Connecting(recv_iter));

                if next_value.is_none() {
                    self.0.take();

                    tracing::debug!("HttpRequestTaskState::Connecting: failed execution");
                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                        HttpClientError::ReadError,
                    )));
                }

                match next_value {
                    None => Some(TaskStatus::Ready(RequestIntro::Failed(
                        HttpClientError::ReadError,
                    ))),
                    Some(TaskStatus::Init) => Some(TaskStatus::Init),
                    Some(TaskStatus::Delayed(dur)) => Some(TaskStatus::Delayed(dur)),
                    Some(TaskStatus::Pending(_)) => {
                        Some(TaskStatus::Pending(HttpRequestPending::WaitingForStream))
                    }
                    Some(TaskStatus::Spawn(action)) => {
                        Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
                    }
                    Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
                    Some(TaskStatus::Ready(item)) => match item {
                        HttpRequestRedirectResponse::Done(
                            stream,
                            reader,
                            boxed_optional_starters,
                        ) => {
                            if let Some(starter_array) = *boxed_optional_starters {
                                let [first, second] = starter_array;

                                tracing::debug!(
                                        "HttpRequestRedirectResponse::Done: first = {:?}, second = {:?}",
                                        first,
                                        second
                                    );

                                let intro = if let IncomingResponseParts::Intro(
                                    status,
                                    proto,
                                    text,
                                ) = first
                                {
                                    tracing::debug!("HttpRequestRedirectResponse::Done: parsed intro status={:?}", status);
                                    (status, proto, text)
                                } else {
                                    self.0.take();

                                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                                        HttpClientError::ReadError,
                                    )));
                                };

                                let IncomingResponseParts::Headers(headers) = second else {
                                    self.0.take();

                                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                                        HttpClientError::ReadError,
                                    )));
                                };

                                tracing::trace!("Setting state to SkipReading for request");

                                self.0 = Some(SendRequestState::SkipReading(Box::new(Some(
                                    RequestIntro::Success {
                                        stream: Box::new(reader),
                                        conn: stream,
                                        intro,
                                        headers,
                                    },
                                ))));

                                return Some(TaskStatus::Pending(
                                    HttpRequestPending::WaitingIntroAndHeaders,
                                ));
                            }

                            tracing::trace!("Setting state to Reading for request");
                            let (get_intro_stream_action, get_intro_receiver) =
                                InlineSendAction::boxed_mapper(
                                    crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                                    Vec::new(),
                                    GetRequestIntroTask::new(stream)
                                        .with_body_config(self.1.into_simple_http_body()),
                                    self.1.inline_processing_timeout,
                                );

                            self.0 = Some(SendRequestState::Reading(drive_receiver(
                                get_intro_receiver,
                            )));

                            Some(TaskStatus::Spawn(
                                get_intro_stream_action.into_box_send_execution_action(),
                            ))
                        }
                        HttpRequestRedirectResponse::FlushFailed(mut conn, err) => {
                            tracing::debug!(
                                "FlushFailed(err={err:?}): Failed and re-attempt and fetch intro"
                            );

                            if let Err(err) = conn.stream_mut().flush() {
                                tracing::error!("Failed to flush HTTP stream: {}", err);
                            }

                            let (get_intro_stream_action, get_intro_receiver) =
                                InlineSendAction::boxed_mapper(
                                    crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                                    Vec::new(),
                                    GetRequestIntroTask::new(conn)
                                        .with_body_config(self.1.into_simple_http_body()),
                                    self.1.inline_processing_timeout,
                                );

                            self.0 = Some(SendRequestState::Reading(drive_receiver(
                                get_intro_receiver,
                            )));

                            Some(TaskStatus::Spawn(
                                get_intro_stream_action.into_box_send_execution_action(),
                            ))
                        }
                        HttpRequestRedirectResponse::Error(err) => {
                            self.0.take();
                            Some(TaskStatus::Ready(RequestIntro::Failed(err)))
                        }
                    },
                }
            }
            SendRequestState::SkipReading(boxed) => match *boxed {
                None => Some(TaskStatus::Ready(RequestIntro::Failed(
                    HttpClientError::ReadError,
                ))),
                Some(container) => {
                    // do one last check for a redirect as final response
                    self.0 = Some(SendRequestState::CheckRedirect(Box::new(Some(container))));

                    // Some(TaskStatus::Ready(container))
                    Some(TaskStatus::Pending(
                        HttpRequestPending::CheckRedirectResponse,
                    ))
                }
            },
            SendRequestState::Reading(mut intro_recv) => {
                let next_value = intro_recv.next();

                tracing::debug!(
                    "HttpRequestTaskState::Reading: Gotten next state from iterator, is_some={}",
                    next_value.is_some()
                );
                self.0 = Some(SendRequestState::Reading(intro_recv));

                if next_value.is_none() {
                    self.0.take();

                    return Some(TaskStatus::Ready(RequestIntro::Failed(
                        HttpClientError::ReadError,
                    )));
                }

                match next_value {
                    None => Some(TaskStatus::Ready(RequestIntro::Failed(
                        HttpClientError::ReadError,
                    ))),
                    Some(TaskStatus::Init) => Some(TaskStatus::Init),
                    Some(TaskStatus::Delayed(dur)) => Some(TaskStatus::Delayed(dur)),
                    Some(TaskStatus::Pending(())) => {
                        Some(TaskStatus::Pending(HttpRequestPending::WaitingForStream))
                    }
                    Some(TaskStatus::Spawn(action)) => {
                        Some(TaskStatus::Spawn(action.into_box_send_execution_action()))
                    }
                    Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
                    Some(TaskStatus::Ready(item)) => {
                        // do one last check for a redirect as final response
                        self.0 = Some(SendRequestState::CheckRedirect(Box::new(Some(item))));

                        Some(TaskStatus::Pending(
                            HttpRequestPending::CheckRedirectResponse,
                        ))
                    }
                }
            }
            SendRequestState::CheckRedirect(boxed) => match *boxed {
                None => Some(TaskStatus::Ready(RequestIntro::Failed(
                    HttpClientError::ReadError,
                ))),
                Some(mut inner) => {
                    if let RequestIntro::Success {
                        stream: _,
                        conn,
                        intro,
                        headers,
                    } = &mut inner
                    {
                        let is_redirect = (Status::MovedPermanently..=Status::PermanentRedirect)
                            .contains(&intro.0);
                        tracing::debug!(
                            "HttpRequestTaskState::CheckRedirect: intro={:?}, is_redirect={:?}, location= {:?}, headers = {:?}",
                            &intro,
                            is_redirect,
                            headers.get(&SimpleHeader::LOCATION),
                            &headers,
                        );

                        // if not redirect, yield here
                        if !is_redirect {
                            self.0 = Some(SendRequestState::Done);
                            return Some(TaskStatus::Ready(inner));
                        }

                        // should we not follow body response redirect if so, yield here.
                        if is_redirect && !self.1.follow_other_redirects_response {
                            self.0 = Some(SendRequestState::Done);
                            return Some(TaskStatus::Ready(inner));
                        }

                        // drain connection
                        conn.drain_stream();

                        return match headers.get(&SimpleHeader::LOCATION).and_then(|v| v.first()) {
                            Some(redirect_url) => {
                                tracing::debug!(
                                    "CheckRedirect: Location header value = {}",
                                    redirect_url
                                );

                                match redirects::resolve_location(&self.3, redirect_url.as_str()) {
                                    Ok(parsed_uri) => {
                                        let parsed_uri_string: String = parsed_uri.to_string();

                                        tracing::debug!("CheckRedirect: Adding Link header from previous response for: {:?}", &self.3);

                                        let mut new_request = SimpleIncomingRequest::builder()
                                            .with_plain_url(parsed_uri_string.as_str())
                                            .with_uri(parsed_uri.clone())
                                            .with_body(SendSafeBody::None)
                                            .with_method(SimpleMethod::GET);

                                        // we always need to include the host in the headers, else
                                        // it fails.
                                        if let Some(host_str) = parsed_uri.host_str() {
                                            new_request = new_request
                                                .add_header(SimpleHeader::HOST, host_str);
                                        }

                                        if let Some(headers_to_add) = &self.1.headers_to_add {
                                            tracing::debug!(
                                                "CheckRedirect: Adding headers to req: {:?}",
                                                headers_to_add
                                            );
                                            for (key, values) in headers_to_add.iter() {
                                                for value in values {
                                                    new_request = new_request
                                                        .add_header(key.clone(), value.clone());
                                                }
                                            }
                                        }

                                        if let Some(link_values) = headers.get(&SimpleHeader::LINK)
                                        {
                                            tracing::debug!(
                                                "CheckRedirect: Link header found: {:?}",
                                                &link_values
                                            );
                                            for link in link_values {
                                                new_request = new_request
                                                    .add_header(SimpleHeader::LINK, link.clone());
                                            }
                                        }

                                        tracing::debug!(
                                            "CheckRedirect: Checking for headers to pass on: {:?} from headers={:?}",
                                            &self.1.headers_to_pass_on_redirect,
                                            &headers,
                                        );
                                        if let Some(pass_on_headers) =
                                            &self.1.headers_to_pass_on_redirect
                                        {
                                            tracing::debug!(
                                                "CheckRedirect: Adding new headers: {:?}",
                                                pass_on_headers,
                                            );
                                            for header in pass_on_headers {
                                                tracing::debug!(
                                                    "CheckRedirect: Pulling value for: {:?}",
                                                    &header,
                                                );

                                                if let Some(header_values) = headers.get(header) {
                                                    tracing::debug!(
                                                        "CheckRedirect: Adding values for: {:?} found: {:?}",
                                                        &header,
                                                        &header_values,
                                                    );
                                                    for header_value in header_values {
                                                        new_request = new_request.add_header(
                                                            header.clone(),
                                                            header_value.clone(),
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        match new_request.build() {
                                            Ok(newly_built_request) => {
                                                tracing::debug!("CheckRedirect: creating new request task for: {:?} with new request: {:?}", &parsed_uri_string, &newly_built_request);

                                                let (get_stream_action, get_stream_receiver) = inlined_task(
                                                        crate::valtron::InlineSendActionBehaviour::LiftWithParent,
                                                        Vec::new(),
                                                        GetHttpRequestRedirectTask::new(
                                                            newly_built_request,
                                                            self.2.clone(),
                                                            self.1.clone(),
                                                            self.1.max_redirects,
                                                        ),
                                                        self.1.inline_processing_timeout,
                                                    );

                                                self.0 = Some(SendRequestState::Connecting(
                                                    get_stream_receiver,
                                                ));

                                                Some(TaskStatus::Spawn(
                                                    get_stream_action
                                                        .into_box_send_execution_action(),
                                                ))
                                            }
                                            Err(err) => {
                                                Some(TaskStatus::Ready(RequestIntro::Failed(
                                                    HttpClientError::SimpleRequestError(err),
                                                )))
                                            }
                                        }
                                    }
                                    Err(parsed_err) => {
                                        Some(TaskStatus::Ready(RequestIntro::Failed(parsed_err)))
                                    }
                                }
                            }
                            None => Some(TaskStatus::Ready(RequestIntro::Failed(
                                HttpClientError::ReadError,
                            ))),
                        };
                    }

                    self.0 = Some(SendRequestState::Done);
                    Some(TaskStatus::Ready(inner))
                }
            },
            SendRequestState::Done => None,
        }
    }
}
