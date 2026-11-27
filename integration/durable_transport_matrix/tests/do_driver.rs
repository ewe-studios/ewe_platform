//! The Durable Object driver test.
//!
//! It starts `wrangler dev --local` for the `do_sql` fixture on a free port,
//! sends each scenario to `GET /matrix?scenario=...` and compares the
//! observation log with the model oracle, then with the embedded driver.
//!
//! The test is off by default: it needs Node, `wrangler` and `worker-build`.
//! Run it with `cargo test -p durable_transport_matrix --features do-driver
//! --test do_driver`.

#![cfg(all(feature = "do-driver", unix))]

use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use durable_transport_matrix::{
    model_scenario, run_scenario, EmbeddedDriver, Observation, Scenario,
};
use foundation_context::Ctx;
use foundation_core::valtron::{block_on_future_scoped, initialize_pool};
use serde::Deserialize;
use serial_test::serial;

/// The reply of `GET /matrix`.
#[derive(Debug, Deserialize)]
struct MatrixReply {
    /// True when the scenario ran.
    ok: bool,
    /// The observation log, one entry per step.
    #[serde(default)]
    observations: Vec<Observation>,
    /// The route error, when `ok` is false.
    #[serde(default)]
    error: Option<String>,
}

/// One `wrangler dev` process. It is killed by its handle, never by name.
struct Wrangler {
    child: Child,
    port: u16,
}

impl Wrangler {
    /// Start `wrangler dev --local` on a free port and wait for readiness.
    fn start() -> Self {
        let port = free_port();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../backends/foundation_db/integration/do_sql");
        let child = Command::new("wrangler")
            .args([
                "dev",
                "--local",
                "--ip",
                "127.0.0.1",
                "--port",
                &port.to_string(),
            ])
            .current_dir(&fixture)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn wrangler");
        let mut wrangler = Self { child, port };
        wrangler.wait_ready();
        wrangler
    }

    /// Wait until the worker answers, or fail after ten minutes.
    fn wait_ready(&mut self) {
        // A bounded count, not a clock read: 1 200 tries at 500 ms.
        for _ in 0..1_200 {
            if http_get(self.port, "/").is_ok() {
                return;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!("wrangler exited before it was ready: {status}");
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        panic!("wrangler did not become ready within ten minutes");
    }

    /// The port the worker listens on.
    fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Wrangler {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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
#[serial(durable_transport_matrix)]
fn given_a_durable_object_when_it_runs_the_scenario_list_then_it_matches_the_model_and_embedded_driver(
) {
    let _guard = initialize_pool(42, Some(8));
    let wrangler = Wrangler::start();
    for scenario in Scenario::all() {
        let path = format!("/matrix?scenario={}", scenario.name());
        let (status, body) = http_get(wrangler.port(), &path).unwrap_or_else(|error| {
            panic!(
                "{}: the Durable Object request failed: {error}",
                scenario.name()
            )
        });
        assert_eq!(status, 200, "{}: HTTP status", scenario.name());
        let reply: MatrixReply = serde_json::from_str(&body)
            .unwrap_or_else(|error| panic!("{}: bad JSON reply: {error}\n{body}", scenario.name()));
        assert!(
            reply.ok,
            "{}: the Durable Object route failed: {:?}",
            scenario.name(),
            reply.error
        );

        let expected = block_on_future_scoped(model_scenario(scenario));
        let mut embedded = EmbeddedDriver::new(Ctx::host());
        let embedded_log = block_on_future_scoped(run_scenario(&mut embedded, scenario));

        assert_eq!(
            reply.observations,
            expected,
            "{}: the Durable Object driver must match the model",
            scenario.name()
        );
        assert_eq!(
            reply.observations,
            embedded_log,
            "{}: the Durable Object driver must match the embedded driver",
            scenario.name()
        );
    }
}
