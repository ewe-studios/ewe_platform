//! The streamed download with the hash check.
//!
//! The body goes to a temporary file in bounded chunks while a Secure Hash
//! Algorithm 256 (SHA-256) hasher reads the same chunks. The module never holds
//! the body in memory (invariant I3).
//!
//! The status is checked before the body. A status other than 200 reads at most
//! `max_error_message` bytes of the body (invariant I8).
//!
//! The body comes from the streamed exchange of `foundation_netio`
//! (`open_exchange`), driven by the valtron pool. That path carries a large
//! Hypertext Transfer Protocol Secure (HTTPS) body; the `send_async` body
//! helper ended such a body early on this host (2026-11-25).

use std::fmt::Write as _;
use std::io::Write;
use std::pin::Pin;
use std::sync::Arc;

use foundation_context::env::Timer;
use foundation_context::Ctx;
use foundation_core::valtron::{execute, StreamIteratorExt};
use foundation_netio::shared::client::request_task::HttpExchange;
use foundation_netio::shared::http::{HttpClientError, SimpleHeader, SimpleHeaders, Status};
use foundation_netio::DynNetClient;
use foundation_netio::PreparedRequestBuilder;
use futures::{Stream, StreamExt};
use sha2::{Digest, Sha256};

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::limits::ArtifactLimits;

/// The boxed exchange stream of one response.
type ExchangeStream = Pin<Box<dyn Stream<Item = HttpExchange> + Send + 'static>>;

/// The result of one download step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// One chunk arrived and went to the file and the hasher.
    Chunk,
    /// The body ended. The caller checks the hash with [`Download::finish`].
    Done,
}

/// One streamed download.
pub struct Download {
    stream: ExchangeStream,
    writer: Box<dyn Write + Send>,
    hasher: Sha256,
    done: u64,
    total: Option<u64>,
    expected: String,
    limits: ArtifactLimits,
    timer: Arc<dyn Timer>,
    ctx: Ctx,
}

/// Converts an HTTP client error to the typed store error.
fn client_error(url: &str, error: &HttpClientError) -> SandboxError {
    SandboxError::Io {
        op: format!("download {url}"),
        message: error.to_string(),
    }
}

/// The `Content-Length` of the head, when it is there.
fn content_length(headers: &SimpleHeaders) -> Option<u64> {
    headers
        .get(&SimpleHeader::CONTENT_LENGTH)
        .and_then(|values| values.first())
        .and_then(|value| value.parse::<u64>().ok())
}

/// Reads at most `limit` bytes of a failed body as text.
async fn read_error_body(stream: &mut ExchangeStream, limit: usize) -> String {
    let mut bytes = Vec::new();
    while bytes.len() < limit {
        match stream.next().await {
            Some(HttpExchange::BodyChunk(chunk)) => {
                let room = limit - bytes.len();
                let take = room.min(chunk.len());
                bytes.extend_from_slice(&chunk[..take]);
                if take < chunk.len() {
                    break;
                }
            }
            Some(HttpExchange::Failed(error)) => {
                bytes.extend_from_slice(format!(" ({error})").as_bytes());
                break;
            }
            Some(HttpExchange::Head { .. }) | None => break,
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Sends the request and checks the status before the body.
///
/// # Errors
/// [`SandboxError::Io`] for a transport failure or a status other than 200.
pub async fn open(
    client: &DynNetClient,
    ctx: &Ctx,
    timer: Arc<dyn Timer>,
    url: &str,
    writer: Box<dyn Write + Send>,
    expected: &str,
    limits: &ArtifactLimits,
) -> SandboxResult<Download> {
    let request = PreparedRequestBuilder::get(url)
        .map_err(|error| client_error(url, &error))?
        .build();
    let task = client.open_exchange(request);
    let driven = execute(task, None).map_err(|error| SandboxError::Io {
        op: format!("download {url}"),
        message: error.to_string(),
    })?;
    let mut stream: ExchangeStream = Box::pin(driven.into_next_stream());
    let (status, headers) = match stream.next().await {
        Some(HttpExchange::Head { status, headers }) => (status, headers),
        Some(HttpExchange::Failed(error)) => {
            return Err(SandboxError::Io {
                op: format!("download {url}"),
                message: error.to_string(),
            }
            .into());
        }
        Some(HttpExchange::BodyChunk(_)) => {
            return Err(SandboxError::Io {
                op: format!("download {url}"),
                message: "the body started before the response head".to_string(),
            }
            .into());
        }
        None => {
            return Err(SandboxError::Io {
                op: format!("download {url}"),
                message: "the answer ended before the response head".to_string(),
            }
            .into());
        }
    };
    let code = status_code(&status);
    if code != 200 {
        let message = read_error_body(&mut stream, limits.max_error_message).await;
        return Err(SandboxError::Io {
            op: format!("download {url}"),
            message: format!("status {code}: {message}"),
        }
        .into());
    }
    Ok(Download {
        stream,
        writer,
        hasher: Sha256::new(),
        done: 0,
        total: content_length(&headers),
        expected: expected.to_ascii_lowercase(),
        limits: limits.clone(),
        timer,
        ctx: ctx.clone(),
    })
}

/// Sends one request and follows redirects with the scheme rule of feature 14.
///
/// The client must not follow redirects itself: this function reads each
/// response head, resolves `Location`, and refuses a redirect from `https` to
/// `http` (feature 14, section 4.5).
///
/// # Errors
/// [`SandboxError::Io`] for a transport failure, a status other than 200, a
/// redirect that loses transport security, or more than `max_redirects`
/// redirects.
#[allow(
    clippy::too_many_arguments,
    reason = "the call names the client, the injected effects, the request and its limits"
)]
pub(crate) async fn open_following(
    client: &DynNetClient,
    ctx: &Ctx,
    timer: Arc<dyn Timer>,
    url: &str,
    writer: Box<dyn Write + Send>,
    expected: &str,
    limits: &ArtifactLimits,
    max_redirects: u8,
) -> SandboxResult<Download> {
    let mut current = url.to_string();
    for redirects in 0..=max_redirects {
        let request = PreparedRequestBuilder::get(&current)
            .map_err(|error| client_error(&current, &error))?
            .build();
        let scheme = request.url.scheme().as_str().to_string();
        let task = client.open_exchange(request);
        let driven = execute(task, None).map_err(|error| SandboxError::Io {
            op: format!("download {current}"),
            message: error.to_string(),
        })?;
        let mut stream: ExchangeStream = Box::pin(driven.into_next_stream());
        let (status, headers) = match stream.next().await {
            Some(HttpExchange::Head { status, headers }) => (status, headers),
            Some(HttpExchange::Failed(error)) => {
                return Err(SandboxError::Io {
                    op: format!("download {current}"),
                    message: error.to_string(),
                }
                .into());
            }
            Some(HttpExchange::BodyChunk(_)) | None => {
                return Err(SandboxError::Io {
                    op: format!("download {current}"),
                    message: "the answer ended before the response head".to_string(),
                }
                .into());
            }
        };
        let code = status_code(&status);
        if (300..400).contains(&code) {
            let location = headers
                .get(&SimpleHeader::LOCATION)
                .and_then(|values| values.first())
                .cloned();
            let Some(location) = location else {
                return Err(SandboxError::Io {
                    op: format!("download {current}"),
                    message: format!("status {code} has no Location"),
                }
                .into());
            };
            if redirects == max_redirects {
                return Err(SandboxError::Io {
                    op: format!("download {current}"),
                    message: format!("the redirect limit of {max_redirects} is passed"),
                }
                .into());
            }
            let prepared = PreparedRequestBuilder::get(&current)
                .map_err(|error| client_error(&current, &error))?
                .build();
            let target = foundation_netio::shared::client::redirects::resolve_location(
                &prepared.url,
                &location,
            )
            .map_err(|error| SandboxError::Io {
                op: format!("download {current}"),
                message: format!("the Location is not valid: {error}"),
            })?;
            let target_scheme = target.scheme().as_str();
            if scheme == "https" && target_scheme != "https" {
                return Err(SandboxError::Io {
                    op: format!("download {current}"),
                    message: format!("a redirect from https to {target_scheme} is refused"),
                }
                .into());
            }
            current = target.to_string();
            continue;
        }
        if code != 200 {
            let message = read_error_body(&mut stream, limits.max_error_message).await;
            return Err(SandboxError::Io {
                op: format!("download {current}"),
                message: format!("status {code}: {message}"),
            }
            .into());
        }
        return Ok(Download {
            stream,
            writer,
            hasher: Sha256::new(),
            done: 0,
            total: content_length(&headers),
            expected: expected.to_ascii_lowercase(),
            limits: limits.clone(),
            timer,
            ctx: ctx.clone(),
        });
    }
    Err(SandboxError::Io {
        op: "download".to_string(),
        message: "the redirect limit is passed".to_string(),
    }
    .into())
}

/// The numeric status code.
fn status_code(status: &Status) -> usize {
    let code: usize = status.clone().into();
    code
}

impl Download {
    /// The total size of the body, when the server gave it.
    #[must_use]
    pub fn total(&self) -> Option<u64> {
        self.total
    }

    /// Reads one chunk, writes it and hashes it.
    ///
    /// # Errors
    /// [`SandboxError::ArtifactTooLarge`] when the body passes the limit, and
    /// [`SandboxError::Io`] for a failed read or write.
    pub async fn step(&mut self) -> SandboxResult<Step> {
        let deadline = self.ctx.clock().now() + self.limits.idle_timeout;
        let sleep = self.timer.sleep_until(self.ctx.clone(), deadline);
        let stream = &mut self.stream;
        let next = futures::future::select(stream.next(), sleep).await;
        let item = match next {
            futures::future::Either::Left((item, _)) => item,
            futures::future::Either::Right(_) => {
                return Err(SandboxError::Io {
                    op: "download".to_string(),
                    message: format!("the body sent no bytes for {:?}", self.limits.idle_timeout),
                }
                .into());
            }
        };
        let chunk = match item {
            Some(HttpExchange::BodyChunk(chunk)) => chunk,
            Some(HttpExchange::Failed(error)) => {
                return Err(SandboxError::Io {
                    op: "download".to_string(),
                    message: error.to_string(),
                }
                .into());
            }
            Some(HttpExchange::Head { .. }) => {
                return Err(SandboxError::Io {
                    op: "download".to_string(),
                    message: "the response sent a second head".to_string(),
                }
                .into());
            }
            None => {
                self.writer.flush().map_err(|error| SandboxError::Io {
                    op: "download".to_string(),
                    message: error.to_string(),
                })?;
                return Ok(Step::Done);
            }
        };
        self.done = self.done.saturating_add(chunk.len() as u64);
        if self.done > self.limits.max_download_bytes {
            return Err(SandboxError::ArtifactTooLarge {
                limit: self.limits.max_download_bytes,
                message: format!("the body passed the limit after {} bytes", self.done),
            }
            .into());
        }
        self.hasher.update(&chunk);
        self.writer
            .write_all(&chunk)
            .map_err(|error| SandboxError::Io {
                op: "download".to_string(),
                message: error.to_string(),
            })?;
        Ok(Step::Chunk)
    }

    /// The bytes that arrived.
    #[must_use]
    pub fn done(&self) -> u64 {
        self.done
    }

    /// Checks the hash of the body that arrived.
    ///
    /// # Errors
    /// [`SandboxError::ArtifactHashMismatch`] when the hash differs.
    pub fn finish(&mut self) -> SandboxResult<()> {
        let actual = hex(&self.hasher.clone().finalize());
        if actual != self.expected {
            return Err(SandboxError::ArtifactHashMismatch {
                expected: self.expected.clone(),
                actual,
            }
            .into());
        }
        Ok(())
    }
}

/// The lowercase hexadecimal form of the given digest.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// The SHA-256 of `bytes`, in lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
