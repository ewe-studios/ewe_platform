//! A fake registry over a local socket (feature 15, section 7).
//!
//! The unit and simulation tests use this server: it plays the distribution
//! API, its token service and a storage host for a redirect. It records every
//! request, so a test can check the `Authorization` header and the number of
//! blob downloads.
//!
//! It is a test helper, not a registry. It holds the blobs in memory and it
//! serves one request on each connection.

#![allow(
    dead_code,
    reason = "the later rows of feature 15 use every part of the fake registry"
)]

use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use foundation_sandbox::native::artifacts::sha256_hex;
use serde_json::json;

/// The media type of an image manifest.
pub const OCI_MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
/// The media type of an image configuration.
pub const OCI_CONFIG: &str = "application/vnd.oci.image.config.v1+json";

/// One descriptor JSON object.
#[must_use]
pub fn descriptor(media_type: &str, digest: &str, size: u64) -> serde_json::Value {
    json!({"mediaType": media_type, "digest": digest, "size": size})
}

/// One image manifest body.
#[must_use]
pub fn manifest_body(config: &serde_json::Value, layers: &[serde_json::Value]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "mediaType": OCI_MANIFEST,
        "config": config,
        "layers": layers,
    }))
    .expect("the manifest encodes")
}

/// One image configuration body for `architecture`.
#[must_use]
pub fn config_body(architecture: &str, config: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "architecture": architecture,
        "os": "linux",
        "config": config,
    }))
    .expect("the configuration encodes")
}

/// The token that the token service gives when the test gives none.
pub const TEST_TOKEN: &str = "test-token";

/// The `Authorization` value of `user:pass`.
pub const BASIC_USER_PASS: &str = "Basic dXNlcjpwYXNz";

/// How the fake registry asks for credentials.
#[derive(Clone, Debug, Default)]
pub enum Challenge {
    /// No credentials.
    #[default]
    None,
    /// A bearer token. The registry accepts `Bearer <token>`.
    Bearer {
        /// The token that the registry accepts.
        token: String,
    },
    /// Basic credentials. The registry accepts this exact header value.
    Basic {
        /// The full `Authorization` header value that the registry accepts.
        authorization: String,
    },
}

/// The body of one stored blob.
#[derive(Clone, Debug)]
pub enum BlobBody {
    /// The bytes.
    Bytes(Vec<u8>),
    /// A chunk repeated until `total` bytes went out.
    Repeat {
        /// One piece of the body.
        chunk: Vec<u8>,
        /// The full body size.
        total: u64,
    },
}

impl BlobBody {
    /// The full size of the body.
    #[must_use]
    pub fn len(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            Self::Repeat { total, .. } => *total,
        }
    }

    /// Writes the body to a stream.
    fn write_to(&self, stream: &mut TcpStream) {
        match self {
            Self::Bytes(bytes) => {
                let _ = stream.write_all(bytes);
            }
            Self::Repeat { chunk, total } => {
                let mut left = *total;
                while left > 0 {
                    let take = chunk.len().min(usize::try_from(left).unwrap_or(usize::MAX));
                    if stream.write_all(&chunk[..take]).is_err() {
                        return;
                    }
                    left -= u64::try_from(take).unwrap_or(u64::MAX);
                }
            }
        }
    }
}

/// One manifest that the server serves.
#[derive(Clone, Debug)]
pub struct StoredManifest {
    /// The media type of the manifest.
    pub media_type: String,
    /// The exact body bytes.
    pub body: Vec<u8>,
}

/// One recorded request.
#[derive(Clone, Debug)]
pub struct RecordedRequest {
    /// The method.
    pub method: String,
    /// The path, with its query.
    pub path: String,
    /// The headers, with their names as the client sent them.
    pub headers: Vec<(String, String)>,
}

impl RecordedRequest {
    /// The `Authorization` header, when the request had one.
    #[must_use]
    pub fn authorization(&self) -> Option<&str> {
        self.header("authorization")
    }

    /// One header, by a case-insensitive name.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// The state of the fake registry.
#[derive(Debug, Default)]
pub struct State {
    /// The manifests, by the text after `/manifests/`.
    pub manifests: HashMap<String, StoredManifest>,
    /// The blobs, by digest.
    pub blobs: HashMap<String, BlobBody>,
    /// Every request that the server read.
    pub requests: Vec<RecordedRequest>,
    /// The number of times the server served each blob.
    pub blob_serves: HashMap<String, usize>,
    /// The number of times the server served each manifest.
    pub manifest_serves: HashMap<String, usize>,
    /// A status that overrides every blob answer.
    pub blob_status: Option<(u16, String)>,
    /// A status that overrides every manifest answer.
    pub manifest_status: Option<(u16, String)>,
    /// A redirect base for every blob answer, such as `http://127.0.0.2:9`.
    pub redirect_blobs_to: Option<String>,
    /// How the server asks for credentials.
    pub challenge: Challenge,
    /// The token that the token service returns.
    pub token_value: Option<String>,
    /// The `Authorization` header that the token service needs.
    pub token_basic: Option<String>,
    /// Whether a correct credential still gets a 401.
    pub always_unauthorized: bool,
}

/// A fake registry on a loopback address.
pub struct FakeRegistry {
    address: SocketAddr,
    /// The host name that [`Self::base_url`] uses, which can differ from the
    /// bound address so a test can play a second host on this machine.
    url_host: String,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl FakeRegistry {
    /// Starts the registry on `127.0.0.1`.
    ///
    /// # Panics
    /// When the listener cannot bind.
    #[must_use]
    pub fn start() -> Self {
        Self::start_on("127.0.0.1")
    }

    /// Starts the registry on `address`.
    ///
    /// # Panics
    /// When the listener cannot bind.
    #[must_use]
    pub fn start_on(address: &str) -> Self {
        Self::start_on_host(address, address)
    }

    /// Starts the registry on `bind`, and names it `url_host` in its base URL.
    ///
    /// # Panics
    /// When the listener cannot bind.
    #[must_use]
    pub fn start_on_host(bind: &str, url_host: &str) -> Self {
        let listener = TcpListener::bind(format!("{bind}:0"))
            .unwrap_or_else(|error| panic!("the fake registry binds to {bind}: {error}"));
        let address = listener.local_addr().expect("the fake registry has a port");
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let accept = {
            let state = Arc::clone(&state);
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("ewe-oci-registry".to_string())
                .spawn(move || {
                    for stream in listener.incoming() {
                        if stop.load(Ordering::SeqCst) {
                            break;
                        }
                        if let Ok(stream) = stream {
                            let state = Arc::clone(&state);
                            std::thread::spawn(move || serve(stream, &state));
                        }
                    }
                })
                .expect("the fake registry thread starts")
        };
        Self {
            address,
            url_host: url_host.to_string(),
            state,
            stop,
            accept: Some(accept),
        }
    }

    /// The address of this server.
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The base address, such as `http://127.0.0.1:5000`.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.url_host, self.address.port())
    }

    /// The state of the server.
    ///
    /// # Panics
    /// When another thread panicked while it held the lock.
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Adds one manifest under the reference text that a client asks for.
    pub fn add_manifest(&self, reference: &str, media_type: &str, body: Vec<u8>) {
        self.state().manifests.insert(
            reference.to_string(),
            StoredManifest {
                media_type: media_type.to_string(),
                body,
            },
        );
    }

    /// Adds one blob and returns its digest.
    #[must_use]
    pub fn add_blob(&self, bytes: Vec<u8>) -> String {
        let digest = format!("sha256:{}", sha256_hex(&bytes));
        self.add_blob_at(&digest, BlobBody::Bytes(bytes));
        digest
    }

    /// Adds one blob under a digest.
    pub fn add_blob_at(&self, digest: &str, body: BlobBody) {
        self.state().blobs.insert(digest.to_string(), body);
    }

    /// Every request that the server read.
    #[must_use]
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.state().requests.clone()
    }

    /// The number of requests whose path holds `needle`.
    #[must_use]
    pub fn count_path(&self, needle: &str) -> usize {
        self.state()
            .requests
            .iter()
            .filter(|request| request.path.contains(needle))
            .count()
    }

    /// The number of times the server served one blob.
    #[must_use]
    pub fn blob_serves(&self, digest: &str) -> usize {
        self.state().blob_serves.get(digest).copied().unwrap_or(0)
    }

    /// The number of times the server served one manifest.
    #[must_use]
    pub fn manifest_serves(&self, reference: &str) -> usize {
        self.state()
            .manifest_serves
            .get(reference)
            .copied()
            .unwrap_or(0)
    }
}

impl Drop for FakeRegistry {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

/// Reads one request and writes one answer.
fn serve(mut stream: TcpStream, state: &Arc<Mutex<State>>) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let Some(request) = read_request(&mut stream) else {
        return;
    };
    let (status, reason, headers, body) = {
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.requests.push(request.clone());
        answer(&mut state, &request)
    };
    write_response(&mut stream, status, reason, &headers, &body);
}

/// One answer of the fake registry.
type Answer = (u16, &'static str, Vec<(&'static str, String)>, BlobBody);

/// The answer of one request.
fn answer(state: &mut State, request: &RecordedRequest) -> Answer {
    let path = request.path.as_str();
    if let Some(reference) = path.split_once("/manifests/").map(|(_, after)| after) {
        return manifest_answer(state, request, reference);
    }
    if let Some(digest) = path.split_once("/blobs/").map(|(_, after)| after) {
        return blob_answer(state, request, digest);
    }
    if path.starts_with("/storage/") {
        let digest = path.trim_start_matches("/storage/");
        return match state.blobs.get(digest) {
            Some(body) => {
                *state.blob_serves.entry(digest.to_string()).or_insert(0) += 1;
                (200, "OK", Vec::new(), body.clone())
            }
            None => (404, "Not Found", Vec::new(), text_body("no blob")),
        };
    }
    if path.starts_with("/token") {
        return token_answer(state, request);
    }
    (404, "Not Found", Vec::new(), text_body("no route"))
}

/// The answer of one manifest request.
fn manifest_answer(state: &mut State, request: &RecordedRequest, reference: &str) -> Answer {
    if !authorized(state, request) {
        return challenge_answer(state, request);
    }
    if let Some(status) = &state.manifest_status {
        return (status.0, "Error", Vec::new(), text_body(&status.1));
    }
    let Some(manifest) = state.manifests.get(reference).cloned() else {
        return (404, "Not Found", Vec::new(), text_body("no manifest"));
    };
    *state
        .manifest_serves
        .entry(reference.to_string())
        .or_insert(0) += 1;
    let digest = format!("sha256:{}", sha256_hex(&manifest.body));
    (
        200,
        "OK",
        vec![
            ("Content-Type", manifest.media_type),
            ("Docker-Content-Digest", digest),
        ],
        BlobBody::Bytes(manifest.body),
    )
}

/// The answer of one blob request.
fn blob_answer(state: &mut State, request: &RecordedRequest, digest: &str) -> Answer {
    if !authorized(state, request) {
        return challenge_answer(state, request);
    }
    if let Some(status) = &state.blob_status {
        return (status.0, "Error", Vec::new(), text_body(&status.1));
    }
    if let Some(base) = &state.redirect_blobs_to {
        let location = format!("{base}/storage/{digest}");
        return (
            307,
            "Temporary Redirect",
            vec![("Location", location)],
            BlobBody::Bytes(Vec::new()),
        );
    }
    let Some(body) = state.blobs.get(digest).cloned() else {
        return (404, "Not Found", Vec::new(), text_body("no blob"));
    };
    *state.blob_serves.entry(digest.to_string()).or_insert(0) += 1;
    (200, "OK", Vec::new(), body)
}

/// The answer of the token service.
fn token_answer(state: &mut State, request: &RecordedRequest) -> Answer {
    if let Some(expected) = &state.token_basic {
        if request.authorization() != Some(expected.as_str()) {
            return (
                401,
                "Unauthorized",
                vec![(
                    "WWW-Authenticate",
                    "Basic realm=\"the test token service\"".to_string(),
                )],
                text_body("no credentials"),
            );
        }
    }
    let token = state
        .token_value
        .clone()
        .unwrap_or_else(|| TEST_TOKEN.to_string());
    let body = format!("{{\"token\":\"{token}\",\"expires_in\":300}}");
    (
        200,
        "OK",
        vec![("Content-Type", "application/json".to_string())],
        BlobBody::Bytes(body.into_bytes()),
    )
}

/// Whether a request has the credential that the challenge asks for.
fn authorized(state: &State, request: &RecordedRequest) -> bool {
    if state.always_unauthorized {
        return false;
    }
    match &state.challenge {
        Challenge::None => true,
        Challenge::Bearer { token } => {
            request.authorization() == Some(format!("Bearer {token}").as_str())
        }
        Challenge::Basic { authorization } => {
            request.authorization() == Some(authorization.as_str())
        }
    }
}

/// The 401 answer with the challenge of this server.
fn challenge_answer(state: &State, _request: &RecordedRequest) -> Answer {
    let challenge = match &state.challenge {
        Challenge::Bearer { .. } | Challenge::None => format!(
            "Bearer realm=\"{}/token\",service=\"the test registry\",scope=\"repository:test/image:pull\"",
            state.realm_base()
        ),
        Challenge::Basic { .. } => "Basic realm=\"the test registry\"".to_string(),
    };
    (
        401,
        "Unauthorized",
        vec![("WWW-Authenticate", challenge)],
        text_body("no token"),
    )
}

impl State {
    /// The base address for the realm, from the last request host.
    fn realm_base(&self) -> String {
        let Some(request) = self.requests.last() else {
            return "http://127.0.0.1".to_string();
        };
        let host = request.header("host").unwrap_or("127.0.0.1");
        format!("http://{host}")
    }
}

/// A small text body.
fn text_body(text: &str) -> BlobBody {
    BlobBody::Bytes(text.as_bytes().to_vec())
}

/// Reads one request head.
fn read_request(stream: &mut TcpStream) -> Option<RecordedRequest> {
    let mut head = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return None,
            Ok(read) => {
                head.extend_from_slice(&buffer[..read]);
                if head.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                if head.len() > 64 * 1024 {
                    return None;
                }
            }
        }
    }
    let text = String::from_utf8_lossy(&head);
    let mut lines = text.lines();
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    Some(RecordedRequest {
        method,
        path,
        headers,
    })
}

/// Writes one answer.
fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    headers: &[(&'static str, String)],
    body: &BlobBody,
) {
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    body.write_to(stream);
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// The SHA-256 of a chunk repeated `total` times as bytes.
#[must_use]
pub fn sha256_repeat(chunk: &[u8], total: u64) -> String {
    use sha2::{Digest as _, Sha256};

    let mut hasher = Sha256::new();
    let mut left = total;
    while left > 0 {
        let take = chunk.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        hasher.update(&chunk[..take]);
        left -= u64::try_from(take).unwrap_or(u64::MAX);
    }
    let mut text = String::with_capacity(64);
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        let _ = write!(text, "{byte:02x}");
    }
    text
}
