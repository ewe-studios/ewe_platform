//! TCP capture utility for recording raw HTTP responses.
//!
//! WHY: To debug HTTP chunked transfer encoding issues, we need to capture
//! the exact bytes sent by servers like GCP Discovery API.
//!
//! WHAT: A CLI subcommand that fetches a URL and saves the raw TCP response
//! (headers + body) to a file for later analysis and regression testing.
//!
//! HOW: Uses raw TCP connection to capture all bytes from the HTTP response
//! without any parsing - exactly what the server sent over the wire.

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::missing_errors_doc)]

use clap::{ArgMatches, Command};
use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

/// Register the `tcp_capture` subcommand.
pub fn register(cmd: Command) -> Command {
    cmd.subcommand(
        Command::new("tcp_capture")
            .about("Capture raw TCP/HTTP response from a URL")
            .arg(
                clap::Arg::new("url")
                    .help("URL to fetch (must be HTTP, not HTTPS)")
                    .required(true)
                    .index(1),
            )
            .arg(
                clap::Arg::new("output")
                    .long("output")
                    .short('o')
                    .help("Output file path for raw response")
                    .required(true)
                    .value_name("FILE"),
            )
            .arg(
                clap::Arg::new("timeout")
                    .long("timeout")
                    .help("Connection timeout in seconds")
                    .default_value("30")
                    .value_name("SECS"),
            )
            .arg(
                clap::Arg::new("debug")
                    .long("debug")
                    .action(clap::ArgAction::SetTrue)
                    .help("Enable debug logging"),
            ),
    )
}

/// Run the `tcp_capture` command.
pub fn run(matches: &ArgMatches) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let logging_level = if matches.get_flag("debug") {
        Level::DEBUG
    } else {
        Level::INFO
    };

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(logging_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let url = matches.get_one::<String>("url").unwrap();
    let output_path = matches.get_one::<String>("output").unwrap();
    let timeout_secs: u64 = matches
        .get_one::<String>("timeout")
        .unwrap()
        .parse()
        .unwrap_or(30);

    info!("Capturing raw HTTP response from: {}", url);
    info!("Output file: {}", output_path);

    // Parse URL (simple parsing for http://host:port/path)
    let url_trimmed = url.trim_start_matches("http://");
    let (host_port, path) = url_trimmed.split_once('/').unwrap_or((url_trimmed, ""));
    let path = format!("/{}", path);

    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (h.to_string(), p.parse::<u16>()?)
    } else {
        (host_port.to_string(), 80)
    };

    info!("Connecting to {}:{}...", host, port);

    // Connect via TCP
    let timeout = Duration::from_secs(timeout_secs);
    let mut stream = TcpStream::connect_timeout(&format!("{}:{}", host, port).parse()?, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    // Build HTTP/1.1 request
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        path, host
    );

    info!("Sending request...");
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    // Read ALL response bytes until connection closed
    info!("Reading response...");
    let mut raw_bytes = Vec::new();
    let mut buffer = vec![0u8; 8192];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                info!("Connection closed by server");
                break;
            }
            Ok(n) => {
                info!("Read {} bytes", n);
                raw_bytes.extend_from_slice(&buffer[..n]);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                info!("Read timeout - assuming complete");
                break;
            }
            Err(e) => {
                error!("Read error: {}", e);
                return Err(format!("Read error: {}", e).into());
            }
        }
    }

    info!("Total bytes captured: {}", raw_bytes.len());

    // Write raw bytes to output file
    let mut output = File::create(output_path)?;
    output.write_all(&raw_bytes)?;
    output.flush()?;

    info!("Raw TCP response written to: {}", output_path);

    // Also write analysis file with hex dump
    let analysis_path = format!("{}.analysis", output_path);
    let mut analysis = File::create(&analysis_path)?;

    writeln!(analysis, "=== TCP Capture Analysis ===")?;
    writeln!(analysis, "URL: {}", url)?;
    writeln!(analysis, "Total bytes: {}", raw_bytes.len())?;
    writeln!(analysis)?;

    // Find end of headers
    if let Some(header_end) = find_header_end(&raw_bytes) {
        writeln!(analysis, "=== HTTP HEADERS ===")?;
        let headers = &raw_bytes[..header_end];
        writeln!(analysis, "{}", String::from_utf8_lossy(headers))?;
        writeln!(analysis)?;

        // Check for chunked encoding
        let headers_str = String::from_utf8_lossy(headers);
        let is_chunked = headers_str
            .lines()
            .any(|l| l.to_lowercase().contains("transfer-encoding") && l.to_lowercase().contains("chunked"));

        writeln!(analysis, "=== BODY ===")?;
        writeln!(analysis, "Transfer-Encoding: {}", if is_chunked { "chunked" } else { "identity/content-length" })?;
        writeln!(analysis, "Body size: {} bytes", raw_bytes.len() - header_end)?;
        writeln!(analysis)?;

        // Hex dump of body (first 2KB)
        let body = &raw_bytes[header_end..];
        let dump_len = body.len().min(2048);
        writeln!(analysis, "=== HEX DUMP (first {} bytes) ===", dump_len)?;
        for (i, chunk) in body[..dump_len].chunks(16).enumerate() {
            let offset = header_end + i * 16;
            let hex_str: String = chunk
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let ascii_str: String = chunk
                .iter()
                .map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' })
                .collect();
            writeln!(analysis, "{:08x} {:<48} {}", offset, hex_str, ascii_str)?;
        }

        // Count CR/LF bytes
        let cr_count = body.iter().filter(|&&b| b == b'\r').count();
        let lf_count = body.iter().filter(|&&b| b == b'\n').count();
        writeln!(analysis)?;
        writeln!(analysis, "=== BYTE ANALYSIS ===")?;
        writeln!(analysis, "CR (0x0D) count: {}", cr_count)?;
        writeln!(analysis, "LF (0x0A) count: {}", lf_count)?;

        if cr_count > 0 {
            writeln!(analysis)?;
            writeln!(analysis, "=== CR BYTE POSITIONS ===")?;
            for (i, &b) in body.iter().enumerate() {
                if b == b'\r' {
                    writeln!(analysis, "CR at absolute position {} (body offset {})", header_end + i, i)?;
                }
            }
        }
    }

    info!("Analysis written to: {}", analysis_path);
    Ok(())
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    // Look for \r\n\r\n (CRLFCRLF) which marks end of HTTP headers
    for i in 3..data.len() {
        if data[i] == b'\n' && data[i - 1] == b'\r' && data[i - 2] == b'\n' && data[i - 3] == b'\r' {
            return Some(i + 1);
        }
    }
    // Also try LF-only (some servers use \n\n)
    for i in 1..data.len() {
        if data[i] == b'\n' && data[i - 1] == b'\n' {
            return Some(i + 1);
        }
    }
    None
}
