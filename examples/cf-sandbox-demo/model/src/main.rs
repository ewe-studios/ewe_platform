//! The model server of the demo sandbox container (spec 92 feature 06).
//!
//! The model is deterministic and needs no network, no key and no graphics
//! processor (GPU). It answers one prompt with one short line. The demo uses it
//! to prove the third model path: a model that runs in the sandbox's own
//! container and is reached over the container port.
//!
//! Routes:
//!
//! - `POST /answer` — the prompt is the body; the answer is `text/plain`.
//! - `GET /health` — `ok`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

/// The provider name that the demo shows for this path.
const PROVIDER: &str = "container-model";
/// The largest prompt body that the server reads.
const MAX_BODY_BYTES: usize = 64 * 1024;
/// The longest part of the prompt that the answer repeats.
const MAX_ECHO_BYTES: usize = 160;
/// The default listen address.
const DEFAULT_LISTEN: &str = "0.0.0.0:8081";

fn main() -> ExitCode {
    let listen = listen_address();
    let listener = match TcpListener::bind(&listen) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("demo-model: cannot listen on {listen}: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("demo-model: listening on {listen}");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || serve(stream));
            }
            Err(error) => eprintln!("demo-model: the accept failed: {error}"),
        }
    }
    ExitCode::SUCCESS
}

/// The `--listen <address>` value, or the default.
fn listen_address() -> String {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--listen" {
            if let Some(address) = arguments.next() {
                return address;
            }
        }
    }
    DEFAULT_LISTEN.to_string()
}

/// Serve one HTTP/1.1 request and close the connection.
fn serve(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Some((method, path, body)) = read_request(&mut stream) else {
        return;
    };
    let (status, text) = match (method.as_str(), path.as_str()) {
        ("GET", "/health") => (200, "ok".to_string()),
        ("POST", "/answer") => (200, answer(&body)),
        _ => (404, "not found".to_string()),
    };
    let _ = write_response(&mut stream, status, &text);
}

/// Read the request line, the headers and the bounded body.
fn read_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(position) = find_headers_end(&buffer) {
            break position;
        }
        if buffer.len() > MAX_BODY_BYTES {
            return None;
        }
    };
    let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();

    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0)
        .min(MAX_BODY_BYTES);

    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    let end = (body_start + content_length).min(buffer.len());
    let body = String::from_utf8_lossy(&buffer[body_start..end]).into_owned();
    Some((method, path, body))
}

/// The byte offset of the empty line that ends the header block.
fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// The deterministic answer to one prompt.
fn answer(prompt: &str) -> String {
    let prompt = prompt.trim();
    let mut echo: String = prompt.chars().take(MAX_ECHO_BYTES).collect();
    if echo.is_empty() {
        echo = "(empty prompt)".to_string();
    }
    format!("{PROVIDER} answered: {echo}\n")
}

/// Write one response with a known length.
fn write_response(stream: &mut TcpStream, status: u16, text: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Unknown",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         content-type: text/plain; charset=utf-8\r\n\
         content-length: {}\r\n\
         connection: close\r\n\r\n",
        text.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(text.as_bytes())?;
    stream.flush()
}
