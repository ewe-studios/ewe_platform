//! The support of the guest-artifact tests.
//!
//! The tests use a real local Hypertext Transfer Protocol (HTTP) server, a real
//! temporary directory and the real host file system. No test makes a request
//! to the network, except the two tests that the feature marks as network
//! tests.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_sandbox::native::artifacts::{ArtifactLimits, ArtifactStore};
use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::shared::host::HostFs;

/// Starts the shared valtron pool one time for this test binary.
pub fn pool() {
    crate::pool::pool();
}

/// Drives a future to its result on the test thread.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    crate::pool::block_on(future)
}

/// A temporary directory that removes itself.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// A new empty directory with `tag` in its name.
    ///
    /// # Panics
    /// When the directory cannot be made.
    #[must_use]
    pub fn new(tag: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let count = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("ewe-artifact-{tag}-{}-{count}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the test directory is made");
        Self { path }
    }

    /// The path of the directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// How a test server sends a body.
#[derive(Clone, Debug)]
pub enum ReplyBody {
    /// The bytes.
    Bytes(Vec<u8>),
    /// The chunk repeated until `total` bytes went out.
    Repeat {
        /// The declared and sent length.
        total: u64,
        /// One piece of the body.
        chunk: Vec<u8>,
    },
}

/// The answer of the test server.
#[derive(Clone, Debug)]
pub struct Reply {
    /// The status code.
    pub status: u16,
    /// The reason text.
    pub reason: &'static str,
    /// The body.
    pub body: ReplyBody,
    /// The `Content-Length` when it differs from the body that the server
    /// sends. It makes a cut body.
    pub declared: Option<u64>,
}

impl Reply {
    /// A 200 answer with the given bytes.
    #[must_use]
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            reason: "OK",
            body: ReplyBody::Bytes(body.into()),
            declared: None,
        }
    }

    /// A 200 answer with `total` bytes of `byte`.
    #[must_use]
    pub fn repeat(total: u64, byte: u8) -> Self {
        Self {
            status: 200,
            reason: "OK",
            body: ReplyBody::Repeat {
                total,
                chunk: vec![byte; 64 * 1024],
            },
            declared: None,
        }
    }

    /// The same answer with a declared length that differs from the sent body.
    /// The connection closes after the sent body, so the body is cut.
    #[must_use]
    pub fn declared(mut self, declared: u64) -> Self {
        self.declared = Some(declared);
        self
    }

    /// The body length that the server declares.
    fn length(&self) -> u64 {
        self.declared.unwrap_or(match &self.body {
            ReplyBody::Bytes(bytes) => bytes.len() as u64,
            ReplyBody::Repeat { total, .. } => *total,
        })
    }
}

/// A local HTTP server for the tests.
pub struct TestServer {
    addr: SocketAddr,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Serves every request with `handler`, which reads the request path.
    ///
    /// # Panics
    /// When the server cannot bind.
    pub fn start<F>(handler: F) -> Self
    where
        F: Fn(&str) -> Reply + Send + Sync + 'static,
    {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("the test server binds to the loopback");
        let addr = listener.local_addr().expect("the test server address");
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let handler = Arc::new(handler);
        let accept = {
            let requests = Arc::clone(&requests);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = stream {
                        let handler = Arc::clone(&handler);
                        let requests = Arc::clone(&requests);
                        std::thread::spawn(move || {
                            serve(stream, &*handler, &requests);
                        });
                    }
                }
            })
        };
        Self {
            addr,
            requests,
            stop,
            accept: Some(accept),
        }
    }

    /// The address of `path` on this server.
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    /// The number of requests that the server answered.
    #[must_use]
    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the blocking accept call.
        let _ = TcpStream::connect(self.addr);
        if let Some(accept) = self.accept.take() {
            let _ = accept.join();
        }
    }
}

/// Reads one request and writes one answer.
fn serve(
    mut stream: TcpStream,
    handler: &(dyn Fn(&str) -> Reply + Send + Sync),
    requests: &AtomicUsize,
) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let mut head = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => {
                head.extend_from_slice(&buffer[..read]);
                if head.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                if head.len() > 64 * 1024 {
                    return;
                }
            }
        }
    }
    let text = String::from_utf8_lossy(&head);
    let Some(line) = text.lines().next() else {
        return;
    };
    let mut parts = line.split_whitespace();
    let _method = parts.next();
    let Some(path) = parts.next() else {
        return;
    };
    requests.fetch_add(1, Ordering::SeqCst);
    let reply = handler(path);
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reply.status,
        reply.reason,
        reply.length()
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let sent = match &reply.body {
        ReplyBody::Bytes(bytes) => stream.write_all(bytes).is_ok(),
        ReplyBody::Repeat { total, chunk } => {
            let mut left = *total;
            let mut ok = true;
            while left > 0 {
                let take = chunk.len().min(usize::try_from(left).unwrap_or(usize::MAX));
                if stream.write_all(&chunk[..take]).is_err() {
                    ok = false;
                    break;
                }
                left -= u64::try_from(take).unwrap_or(u64::MAX);
            }
            ok
        }
    };
    if sent {
        let _ = stream.flush();
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// A host file system that records the largest single write and the bytes
/// written.
#[derive(Clone, Debug, Default)]
pub struct RecordingFs {
    max_write: Arc<AtomicUsize>,
    written: Arc<AtomicU64>,
}

impl RecordingFs {
    /// An empty recorder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The largest buffer of one write call.
    #[must_use]
    pub fn max_write(&self) -> usize {
        self.max_write.load(Ordering::SeqCst)
    }

    /// The bytes written.
    #[must_use]
    pub fn written(&self) -> u64 {
        self.written.load(Ordering::SeqCst)
    }
}

impl HostFs for RecordingFs {
    fn create_dir_all(
        &self,
        path: &Path,
        mode: u32,
    ) -> foundation_sandbox::shared::error::SandboxResult<()> {
        NativeFs.create_dir_all(path, mode)
    }

    fn remove_dir_all(&self, path: &Path) -> foundation_sandbox::shared::error::SandboxResult<()> {
        NativeFs.remove_dir_all(path)
    }

    fn write(
        &self,
        path: &Path,
        data: &[u8],
    ) -> foundation_sandbox::shared::error::SandboxResult<()> {
        NativeFs.write(path, data)
    }

    fn read(
        &self,
        path: &Path,
        max: usize,
    ) -> foundation_sandbox::shared::error::SandboxResult<Vec<u8>> {
        NativeFs.read(path, max)
    }

    fn tail(
        &self,
        path: &Path,
        max: usize,
    ) -> foundation_sandbox::shared::error::SandboxResult<Vec<u8>> {
        NativeFs.tail(path, max)
    }

    fn exists(&self, path: &Path) -> bool {
        NativeFs.exists(path)
    }

    fn rename(
        &self,
        from: &Path,
        to: &Path,
    ) -> foundation_sandbox::shared::error::SandboxResult<()> {
        NativeFs.rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> foundation_sandbox::shared::error::SandboxResult<()> {
        NativeFs.remove_file(path)
    }

    fn read_stream(
        &self,
        path: &Path,
    ) -> foundation_sandbox::shared::error::SandboxResult<Box<dyn std::io::Read + Send>> {
        NativeFs.read_stream(path)
    }

    fn clone_file(
        &self,
        from: &Path,
        to: &Path,
    ) -> foundation_sandbox::shared::error::SandboxResult<()> {
        NativeFs.clone_file(from, to)
    }

    fn lock_exclusive(
        &self,
        path: &Path,
    ) -> foundation_sandbox::shared::error::SandboxResult<foundation_sandbox::shared::host::FileLock>
    {
        NativeFs.lock_exclusive(path)
    }

    fn create_file(
        &self,
        path: &Path,
    ) -> foundation_sandbox::shared::error::SandboxResult<Box<dyn Write + Send>> {
        let inner = NativeFs.create_file(path)?;
        Ok(Box::new(RecordingWriter {
            inner,
            max_write: Arc::clone(&self.max_write),
            written: Arc::clone(&self.written),
        }))
    }
}

/// A writer that records its largest call and its byte count.
struct RecordingWriter {
    inner: Box<dyn Write + Send>,
    max_write: Arc<AtomicUsize>,
    written: Arc<AtomicU64>,
}

impl Write for RecordingWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.max_write.fetch_max(buffer.len(), Ordering::SeqCst);
        self.written
            .fetch_add(buffer.len() as u64, Ordering::SeqCst);
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// A context with a simulated clock and a simulated effect environment.
#[must_use]
pub fn ctx(seed: u64) -> Ctx {
    pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(clock, seed);
    let env =
        foundation_core::env::simulated(&ctx, seed).expect("the simulated environment builds");
    ctx.store(env);
    ctx
}

/// A store over a new temporary cache directory.
#[must_use]
pub fn store(tag: &str, limits: ArtifactLimits) -> (ArtifactStore, TempDir) {
    let directory = TempDir::new(tag);
    let fs: Arc<dyn HostFs> = Arc::new(NativeFs);
    let store = ArtifactStore::new(ctx(77), fs, directory.path().join("cache")).with_limits(limits);
    (store, directory)
}

/// A store whose file system records its streamed writes.
#[must_use]
pub fn recording_store(
    tag: &str,
    limits: ArtifactLimits,
) -> (ArtifactStore, TempDir, Arc<RecordingFs>) {
    let directory = TempDir::new(tag);
    let recorder = Arc::new(RecordingFs::new());
    let fs: Arc<dyn HostFs> = Arc::new((*recorder).clone());
    let store = ArtifactStore::new(ctx(78), fs, directory.path().join("cache")).with_limits(limits);
    (store, directory, recorder)
}

/// The first 16 hexadecimal digits of a hash.
#[must_use]
pub fn prefix(hash: &str) -> &str {
    &hash[..16]
}

/// The relative paths of every entry below `root`.
#[must_use]
pub fn entries(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            found.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            );
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
    found
}

/// One entry of a hand-built archive. The name goes into the header without a
/// check, so a test can build an unsafe archive.
pub struct RawEntry<'a> {
    /// The path in the archive, as given.
    pub path: &'a str,
    /// The data of a regular file.
    pub data: &'a [u8],
    /// The link target of a symbolic link.
    pub link: Option<&'a str>,
}

impl<'a> RawEntry<'a> {
    /// A regular file.
    #[must_use]
    pub fn file(path: &'a str, data: &'a [u8]) -> Self {
        Self {
            path,
            data,
            link: None,
        }
    }

    /// A symbolic link.
    #[must_use]
    pub fn symlink(path: &'a str, link: &'a str) -> Self {
        Self {
            path,
            data: &[],
            link: Some(link),
        }
    }
}

/// Builds a `.tar.gz` archive with the given raw entries.
///
/// The tar crate refuses an unsafe path, so the test writes the name bytes into
/// the header itself.
#[must_use]
pub fn raw_tarball(entries: &[RawEntry<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        let mut header = tar::Header::new_gnu();
        let old = header.as_old_mut();
        let name = entry.path.as_bytes();
        old.name[..name.len()].copy_from_slice(name);
        if let Some(link) = entry.link {
            let link = link.as_bytes();
            old.linkname[..link.len()].copy_from_slice(link);
            header.set_entry_type(tar::EntryType::Symlink);
        } else {
            header.set_entry_type(tar::EntryType::Regular);
        }
        header.set_size(entry.data.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(entry.data);
        let padding = (512 - entry.data.len() % 512) % 512;
        out.extend(std::iter::repeat_n(0u8, padding));
    }
    out.extend(std::iter::repeat_n(0u8, 1024));
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&out).expect("the tar stream is written");
    encoder.finish().expect("the gzip stream ends")
}

/// Whether the host can download `bytes` from `url` within five seconds.
///
/// The two network tests use this probe. A link that is present but too slow to
/// finish the test in a reasonable time gives the same skip message as a missing
/// link. The probe itself runs on a thread, so a stalled body cannot hold the
/// test past the window.
#[must_use]
pub fn network_fast_enough(url: &str, bytes: u64) -> bool {
    crate::pool::pool();
    let url = url.to_string();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(prefix_arrives(&url, bytes));
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or(false)
}

/// Downloads `bytes` of `url` and says whether the whole prefix arrived.
fn prefix_arrives(url: &str, bytes: u64) -> bool {
    use foundation_core::valtron::{execute, StreamIteratorExt};
    use foundation_netio::shared::client::request_task::HttpExchange;
    use foundation_netio::shared::http::SimpleHeader;
    use foundation_netio::HttpClientBuilder;
    use foundation_netio::PreparedRequestBuilder;
    use futures::StreamExt;

    let client = HttpClientBuilder::new().build();
    let range = format!("bytes=0-{}", bytes.saturating_sub(1));
    let Ok(builder) = PreparedRequestBuilder::get(url) else {
        return false;
    };
    let request = builder.header(SimpleHeader::RANGE, range).build();
    let Ok(driven) = execute(client.open_exchange(request), None) else {
        return false;
    };
    let mut stream = Box::pin(driven.into_next_stream());
    let mut total = 0u64;
    while total < bytes {
        match futures::executor::block_on(stream.next()) {
            Some(HttpExchange::BodyChunk(chunk)) => total += chunk.len() as u64,
            Some(_) | None => break,
        }
    }
    total >= bytes
}
