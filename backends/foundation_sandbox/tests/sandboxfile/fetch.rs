//! The FETCH rows of feature 14, section 7: I7, the lost hash and the
//! download limit, against a local HTTP server.

use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_sandbox::native::artifacts::sha256_hex;
use foundation_sandbox::native::images::build::executor::BuildStepLine;
use foundation_sandbox::native::images::build::BuildOptions;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::limits::BuildLimits;

use super::support::{base_name, one_step, Harness};

/// One answer of the test server.
#[derive(Clone)]
enum Answer {
    /// A 200 response with this body.
    Ok(Vec<u8>),
    /// A 302 response to this path.
    Redirect(String),
}

/// A local HTTP server with a fixed route table.
struct Server {
    address: SocketAddr,
    requests: Arc<AtomicUsize>,
}

impl Server {
    /// Starts the server on a free loopback port.
    fn start(routes: HashMap<String, Answer>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("the test server binds");
        let address = listener.local_addr().expect("the test server has a port");
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&requests);
        std::thread::Builder::new()
            .name("ewe-sandboxfile-http".to_string())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else {
                        continue;
                    };
                    counter.fetch_add(1, Ordering::SeqCst);
                    let mut buffer = [0_u8; 2048];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]);
                    let path = request
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("/")
                        .to_string();
                    let answer = routes.get(&path).cloned().unwrap_or_else(|| {
                        Answer::Ok(b"not found".to_vec())
                    });
                    let response = match answer {
                        Answer::Ok(body) => format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .into_bytes()
                        .into_iter()
                        .chain(body)
                        .collect::<Vec<u8>>(),
                        Answer::Redirect(target) => format!(
                            "HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .into_bytes(),
                    };
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                }
            })
            .expect("the test server thread starts");
        Self { address, requests }
    }

    /// The URL of one path on this server.
    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    /// How many requests the server answered.
    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

/// The build file of one FETCH step.
fn fetch_file(url: &str, sha256: &str) -> String {
    one_step(&format!("FETCH {url} sha256:{sha256} AS /tool"))
}

#[test]
fn build_fetch_hash() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new("fetch-hash");
    harness.base(&base_name());
    let body = b"tool-binary\n".to_vec();
    let sha = sha256_hex(&body);
    let big = b"too-large".to_vec();
    let big_sha = sha256_hex(&big);
    let mut routes = HashMap::new();
    routes.insert("/tool".to_string(), Answer::Ok(body.clone()));
    routes.insert("/big".to_string(), Answer::Ok(big));
    let server = Server::start(routes);

    // The right hash: the file enters the cache, and a second build downloads
    // nothing.
    let text = fetch_file(&server.url("/tool"), &sha);
    harness.build(&text).expect("the FETCH build succeeds");
    let cached = harness.cache.join("fetch").join(&sha);
    assert_eq!(
        std::fs::read(&cached).expect("the verified file is cached"),
        body
    );
    assert_eq!(server.requests(), 1);
    harness
        .build(&text)
        .expect("the second FETCH build succeeds");
    assert_eq!(server.requests(), 1, "a cache hit downloads nothing");

    // A changed body is refused, and nothing enters the cache.
    let wrong = "00".repeat(32);
    let text = fetch_file(&server.url("/tool"), &wrong);
    let (result, _progress) = harness.build_with_progress(&text);
    let error = result.expect_err("the hash does not match");
    assert!(matches!(
        error.current_context(),
        SandboxError::ArtifactHashMismatch { .. }
    ));
    assert_eq!(
        error.downcast_ref::<BuildStepLine>().map(|line| line.0),
        Some(2),
        "the error names the step line"
    );
    assert!(
        !harness.cache.join("fetch").join(&wrong).exists(),
        "the file with the wrong hash is not cached"
    );

    // A body larger than the fetch limit is refused.
    let limited = BuildOptions {
        limits: BuildLimits::default().with_max_fetch_bytes(4),
        ..BuildOptions::default()
    };
    let text = fetch_file(&server.url("/big"), &big_sha);
    let error = harness
        .build_options(&text, limited)
        .expect_err("the body is larger than the limit");
    assert!(matches!(
        error.current_context(),
        SandboxError::ArtifactTooLarge { .. }
    ));
}

#[test]
fn fetch_follows_a_redirect() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new("fetch-redirect");
    harness.base(&base_name());
    let body = b"redirected\n".to_vec();
    let sha = sha256_hex(&body);
    let mut routes = HashMap::new();
    routes.insert("/old".to_string(), Answer::Redirect("/new".to_string()));
    routes.insert("/new".to_string(), Answer::Ok(body));
    let server = Server::start(routes);
    harness
        .build(&fetch_file(&server.url("/old"), &sha))
        .expect("the redirect is followed");
    assert_eq!(server.requests(), 2, "the client asked twice");
}
