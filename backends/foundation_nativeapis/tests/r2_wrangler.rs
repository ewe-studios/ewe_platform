//! The R2 block-store test on a real local R2 binding.
//!
//! It starts `wrangler dev --local` for the `integration/r2_blocks` fixture on a
//! free port, calls `GET /r2-blocks`, and checks the route's JSON result. The
//! fixture asserts the conditional-put semantics with the bare R2 etag, runs a
//! block put, an idempotent retry, an exact range read, and the SQL virtual file
//! system (VFS) flush and vacuumed read.
//!
//! The test is off by default: it needs Node, `wrangler` and `worker-build`.
//! Run it with `cargo test -p foundation_nativeapis --features r2-wrangler
//! --test r2_wrangler`.

#![cfg(all(feature = "r2-wrangler", unix))]

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde::Deserialize;

/// The reply of `GET /r2-blocks`.
#[derive(Debug, Deserialize)]
struct Reply {
    /// True when every scenario passed.
    ok: bool,
    /// The route error, when `ok` is false.
    #[serde(default)]
    error: Option<String>,
    /// True when the conditional put honoured the bare etag.
    #[serde(default)]
    conditional_put: bool,
    /// True when the repeated put returned success.
    #[serde(default)]
    retry_same_bytes: bool,
    /// The exact byte range the R2 store returned.
    #[serde(default)]
    range: Option<String>,
    /// The bytes the vacuumed SQL VFS read returned from R2.
    #[serde(default)]
    sql_read: Option<String>,
}

/// One `wrangler dev` process. It is killed by its handle, never by name.
struct Wrangler {
    child: Child,
    port: u16,
    log_path: std::path::PathBuf,
}

impl Wrangler {
    /// Start `wrangler dev --local` on a free port and wait for readiness.
    fn start() -> Self {
        let port = free_port();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("integration/r2_blocks");
        let log_path = std::env::temp_dir().join(format!("nativeapis-r2-wrangler-{port}.log"));
        let child = spawn_wrangler(&fixture, port, &log_path);
        let mut wrangler = Self {
            child,
            port,
            log_path,
        };
        wrangler.wait_ready();
        wrangler
    }

    /// Wait until the worker answers, or fail after 240 s with the log tail.
    fn wait_ready(&mut self) {
        // A bounded count, not a clock read: 480 tries at 500 ms.
        for _ in 0..480 {
            if http_get(self.port, "/").is_ok() {
                return;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!(
                    "wrangler exited before it was ready: {status}\n{}",
                    self.log_tail()
                );
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        panic!(
            "wrangler did not become ready within 240 s\n{}",
            self.log_tail()
        );
    }

    /// The port the worker listens on.
    fn port(&self) -> u16 {
        self.port
    }

    /// The last part of the wrangler log, for a failure message.
    fn log_tail(&self) -> String {
        std::fs::read_to_string(&self.log_path).map_or_else(
            |_| String::from("(no wrangler log)"),
            |text| {
                let mut start = text.len().saturating_sub(20_000);
                while start < text.len() && !text.is_char_boundary(start) {
                    start += 1;
                }
                text[start..].to_string()
            },
        )
    }
}

impl Drop for Wrangler {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.log_path);
    }
}

/// Resolve the wrangler command in the order `WRANGLER_BIN`, `npx` with the
/// pinned version, then `wrangler` on `PATH`.
fn spawn_wrangler(fixture: &Path, port: u16, log_path: &Path) -> Child {
    if let Ok(bin) = std::env::var("WRANGLER_BIN") {
        if !bin.is_empty() {
            return launch(&bin, &[], fixture, port, log_path)
                .unwrap_or_else(|error| panic!("start WRANGLER_BIN: {error}"));
        }
    }
    let version = std::env::var("WRANGLER_VERSION").unwrap_or_else(|_| String::from("4.156.2"));
    let prefix = ["--yes".to_string(), format!("wrangler@{version}")];
    match launch("npx", &prefix, fixture, port, log_path) {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            launch("wrangler", &[], fixture, port, log_path).unwrap_or_else(|error| {
                let tail = std::fs::read_to_string(log_path).unwrap_or_default();
                panic!("start wrangler: {error}\n{tail}")
            })
        }
        Err(error) => panic!("start npx wrangler: {error}"),
    }
}

/// Launch one wrangler program with its output written to the log file.
///
/// The output goes to a file, not a pipe: `npx` starts wrangler, which starts
/// workerd, and the descendants keep the pipe's write end open after a kill.
fn launch(
    program: &str,
    prefix: &[String],
    fixture: &Path,
    port: u16,
    log_path: &Path,
) -> io::Result<Child> {
    let log = std::fs::File::create(log_path)?;
    let stderr = log.try_clone()?;
    Command::new(program)
        .args(prefix)
        .args([
            "dev",
            "--local",
            "--ip",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .current_dir(fixture)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr))
        .spawn()
}

/// Pick a free loopback port.
fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a free port");
    listener.local_addr().expect("local address").port()
}

/// Send one HTTP/1.1 GET and return the status and body.
fn http_get(port: u16, path: &str) -> io::Result<(u16, String)> {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(180)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    parse_http(&raw)
}

/// Parse one HTTP/1.1 response. Handle a chunked body.
fn parse_http(raw: &[u8]) -> io::Result<(u16, String)> {
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no header end"))?;
    let head = std::str::from_utf8(&raw[..split])
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let lines: Vec<&str> = head.split("\r\n").collect();
    let status = lines
        .first()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no status code"))?;
    let body = &raw[split + 4..];
    let chunked = header_value(&lines, "transfer-encoding")
        .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"));
    let bytes = if chunked {
        decode_chunked(body)?
    } else {
        body.to_vec()
    };
    let text = String::from_utf8(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok((status, text))
}

/// Read one header value, ignoring the case of the name.
fn header_value<'a>(lines: &[&'a str], name: &str) -> Option<&'a str> {
    lines.iter().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// Decode a chunked body.
fn decode_chunked(mut rest: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let end = rest
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad chunk size"))?;
        let size_text = std::str::from_utf8(&rest[..end])
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let size_text = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        rest = &rest[end + 2..];
        if size == 0 {
            return Ok(out);
        }
        if rest.len() < size + 2 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "short chunk"));
        }
        out.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
}

#[test]
#[foundation_macros::timeout(900_000)]
#[serial_test::serial]
fn r2_block_store_round_trips_under_local_r2() {
    let wrangler = Wrangler::start();
    let (status, body) = http_get(wrangler.port(), "/r2-blocks")
        .unwrap_or_else(|error| panic!("the R2 block route request failed: {error}"));
    assert_eq!(status, 200, "the R2 block route failed: {body}");
    let reply: Reply = serde_json::from_str(&body)
        .unwrap_or_else(|error| panic!("the R2 block reply is not JSON: {error}\n{body}"));
    assert!(reply.ok, "the R2 block route reported: {:?}", reply.error);
    assert!(
        reply.conditional_put,
        "the conditional put did not honour the bare etag"
    );
    assert!(reply.retry_same_bytes, "the repeated put failed");
    assert_eq!(reply.range.as_deref(), Some("3456"));
    assert_eq!(reply.sql_read.as_deref(), Some("world"));
}
