#![cfg(test)]

//! WebSocket server-side integration tests.
//!
//! Tests the foundation_core WebSocket server upgrade handling with real connections.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_core::netcap::RawStream;
use foundation_core::wire::simple_http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};
use foundation_core::wire::websocket::{WebSocketServerConnection, WebSocketUpgrade};
use foundation_core::wire::websocket::{WebSocketMessage, Opcode};
use tracing_test::traced_test;

/// Send raw HTTP response to stream
fn send_response(stream: &mut TcpStream, response: &[u8]) -> std::io::Result<()> {
    stream.write_all(response)?;
    stream.flush()?;
    Ok(())
}

/// Read HTTP response until end of headers (\r\n\r\n)
fn read_http_response_headers(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut response = Vec::new();
    let mut buffer = [0u8; 1];
    let mut consecutive_crlf = 0;

    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();

    loop {
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            break; // Connection closed
        }

        response.push(buffer[0]);

        // Check for \r\n\r\n pattern
        if buffer[0] == b'\n' {
            consecutive_crlf += 1;
            if consecutive_crlf >= 2 {
                break; // End of HTTP headers
            }
        } else if buffer[0] != b'\r' {
            consecutive_crlf = 0;
        }
    }

    Ok(String::from_utf8_lossy(&response).to_string())
}

/// Read HTTP response with timeout, collecting all available data
fn read_http_response(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut response = [0u8; 4096];
    let mut total_bytes = 0;

    loop {
        stream.set_read_timeout(Some(Duration::from_millis(100))).ok();
        match stream.read(&mut response[total_bytes..]) {
            Ok(0) => break, // Connection closed
            Ok(n) => {
                total_bytes += n;
                if total_bytes >= 4096 {
                    break;
                }
            }
            Err(_) => break, // Timeout or error
        }
    }

    Ok(String::from_utf8_lossy(&response[..total_bytes]).to_string())
}

/// Manually parse a simple HTTP request for testing
fn parse_simple_request(data: &[u8]) -> Option<SimpleIncomingRequest> {
    let request_str = String::from_utf8_lossy(data);
    let mut lines = request_str.lines();
    let request_line = lines.next()?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() < 3 {
        return None;
    }

    let method_str = parts[0];
    let path = parts[1];

    let method = match method_str {
        "GET" => SimpleMethod::GET,
        "POST" => SimpleMethod::POST,
        _ => SimpleMethod::GET,
    };

    // Parse headers
    let mut headers = std::collections::BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key_str = key.trim().to_uppercase();
            let value_str = value.trim().to_string();

            let header = match key_str.as_str() {
                "UPGRADE" => SimpleHeader::UPGRADE,
                "CONNECTION" => SimpleHeader::CONNECTION,
                "HOST" => SimpleHeader::HOST,
                "SEC-WEBSOCKET-KEY" => SimpleHeader::SEC_WEBSOCKET_KEY,
                "SEC-WEBSOCKET-VERSION" => SimpleHeader::SEC_WEBSOCKET_VERSION,
                "SEC-WEBSOCKET-PROTOCOL" => SimpleHeader::SEC_WEBSOCKET_PROTOCOL,
                "SEC-WEBSOCKET-ACCEPT" => SimpleHeader::SEC_WEBSOCKET_ACCEPT,
                _ => SimpleHeader::Custom(key_str),
            };

            headers.entry(header).or_insert_with(Vec::new).push(value_str);
        }
    }

    let mut builder = SimpleIncomingRequest::builder()
        .with_plain_url(format!("http://localhost{}", path))
        .with_method(method);

    for (key, values) in &headers {
        for value in values {
            builder = builder.add_header_raw(key.clone(), value);
        }
    }

    builder.build().ok()
}

// Test 1: Server detects WebSocket upgrade request
#[test]
#[traced_test]
fn test_server_detects_upgrade_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let request = parse_simple_request(&buffer[..bytes_read]).unwrap();
        let is_upgrade = WebSocketUpgrade::is_upgrade_request(&request);
        assert!(is_upgrade, "Server should detect WebSocket upgrade request");

        send_response(&mut stream, b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
    });

    thread::sleep(Duration::from_millis(50));
    let mut client = TcpStream::connect(&addr).unwrap();

    let upgrade_request = "GET /chat HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Upgrade: websocket\r\n\
                           Connection: Upgrade\r\n\
                           Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                           Sec-WebSocket-Version: 13\r\n\
                           \r\n";

    client.write_all(upgrade_request.as_bytes()).unwrap();
    client.flush().unwrap();

    let mut response = [0u8; 1024];
    let bytes_read = client.read(&mut response).unwrap();
    let response_str = String::from_utf8_lossy(&response[..bytes_read]);
    assert!(response_str.contains("200 OK"));

    server_handle.join().unwrap();
}

// Test 2: Server builds correct 101 response
#[test]
#[traced_test]
fn test_server_builds_101_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let request = parse_simple_request(&buffer[..bytes_read]).unwrap();
        let (response_bytes, accept_key) = WebSocketUpgrade::accept(&request, None).unwrap();

        assert_eq!(accept_key, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");

        for chunk in &response_bytes {
            send_response(&mut stream, chunk).unwrap();
        }
    });

    thread::sleep(Duration::from_millis(50));
    let mut client = TcpStream::connect(&addr).unwrap();

    let upgrade_request = "GET /chat HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Upgrade: websocket\r\n\
                           Connection: Upgrade\r\n\
                           Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                           Sec-WebSocket-Version: 13\r\n\
                           \r\n";

    client.write_all(upgrade_request.as_bytes()).unwrap();
    client.flush().unwrap();

    let mut response = [0u8; 4096];
    let mut total_bytes = 0;

    // Read until server closes connection or timeout
    loop {
        client.set_read_timeout(Some(Duration::from_millis(100))).ok();
        match client.read(&mut response[total_bytes..]) {
            Ok(0) => break, // Connection closed
            Ok(n) => {
                total_bytes += n;
                if total_bytes >= 4096 {
                    break;
                }
            }
            Err(_) => break, // Timeout or error
        }
    }

    let response_str = String::from_utf8_lossy(&response[..total_bytes]);
    eprintln!("Client received {} bytes: {}", total_bytes, response_str);

    assert!(response_str.contains("101"));
    assert!(response_str.contains("Switching Protocols"));
    assert!(response_str.contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));

    server_handle.join().unwrap();
}

// Test 3: Server accepts with subprotocol
#[test]
#[traced_test]
fn test_server_accepts_with_subprotocol() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let request = parse_simple_request(&buffer[..bytes_read]).unwrap();
        let (response_bytes, _) = WebSocketUpgrade::accept(&request, Some("chat")).unwrap();

        // Send all chunks at once, then close connection to signal end of response
        for chunk in &response_bytes {
            send_response(&mut stream, chunk).unwrap();
        }
        // Explicitly close the connection to signal end of handshake
        drop(stream);
    });

    thread::sleep(Duration::from_millis(50));
    let mut client = TcpStream::connect(&addr).unwrap();

    let upgrade_request = "GET /chat HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Upgrade: websocket\r\n\
                           Connection: Upgrade\r\n\
                           Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                           Sec-WebSocket-Version: 13\r\n\
                           Sec-WebSocket-Protocol: chat, superchat\r\n\
                           \r\n";

    client.write_all(upgrade_request.as_bytes()).unwrap();
    client.flush().unwrap();

    let response_str = read_http_response(&mut client).unwrap();
    eprintln!("Client received {} bytes: [{}]", response_str.len(), response_str);

    let response_upper = response_str.to_uppercase();
    assert!(response_upper.contains("SEC-WEBSOCKET-PROTOCOL"), "Response should contain Sec-WebSocket-Protocol header. Got: {}", response_str);
    assert!(response_upper.contains("CHAT"), "Response should contain chat protocol. Got: {}", response_str);

    server_handle.join().unwrap();
}

// Test 4: Server connection can send and receive frames
#[test]
#[traced_test]
fn test_server_connection_send_recv() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let request = parse_simple_request(&buffer[..bytes_read]).unwrap();
        let (response_bytes, _) = WebSocketUpgrade::accept(&request, None).unwrap();

        // Send all response bytes at once
        for chunk in &response_bytes {
            stream.write_all(chunk).unwrap();
        }
        stream.flush().unwrap();

        // Set longer timeout for frame read
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

        // Read WebSocket frame header
        let mut frame_header = [0u8; 2];
        let header_read = stream.read_exact(&mut frame_header);
        if header_read.is_ok() {
            let masked = (frame_header[1] & 0x80) != 0;
            let payload_len = (frame_header[1] & 0x7F) as usize;

            // Read mask if present
            let mut mask = [0u8; 4];
            if masked {
                stream.read_exact(&mut mask).ok();
            }

            // Read payload
            let mut payload = vec![0u8; payload_len];
            stream.read_exact(&mut payload).ok();

            // Unmask if needed
            if masked {
                for (i, byte) in payload.iter_mut().enumerate() {
                    *byte ^= mask[i % 4];
                }
            }

            // Echo back (server doesn't mask)
            // Clear the mask bit (0x80) since server frames are not masked
            let mut response = vec![frame_header[0], frame_header[1] & 0x7F];
            response.extend_from_slice(&payload);
            stream.write_all(&response).ok();
            stream.flush().ok();
        }

        // Keep stream alive
        std::thread::sleep(Duration::from_millis(100));
    });

    thread::sleep(Duration::from_millis(50));
    let mut client = TcpStream::connect(&addr).unwrap();

    // Send upgrade request
    let upgrade_request = "GET /chat HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Upgrade: websocket\r\n\
                           Connection: Upgrade\r\n\
                           Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                           Sec-WebSocket-Version: 13\r\n\
                           \r\n";

    client.write_all(upgrade_request.as_bytes()).unwrap();
    client.flush().unwrap();

    // Read handshake - read until \r\n\r\n
    let _ = read_http_response_headers(&mut client).unwrap();

    // Send masked WebSocket frame (client MUST mask)
    let mask = [0x01, 0x02, 0x03, 0x04];
    let payload = b"Hello";
    let mut frame = vec![0x81, 0x80 | 0x05]; // FIN=1, text, masked, len=5
    frame.extend_from_slice(&mask);
    let mut masked_payload = payload.to_vec();
    for (i, byte) in masked_payload.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
    frame.extend_from_slice(&masked_payload);

    client.write_all(&frame).unwrap();
    client.flush().unwrap();

    // Read echo
    let mut echo = [0u8; 1024];
    let echo_read = client.read(&mut echo).unwrap();
    assert!(echo_read >= 7);
    assert_eq!(echo[0], 0x81);
    assert_eq!(echo[1], 0x05);
    assert_eq!(&echo[2..7], b"Hello");

    server_handle.join().unwrap();
}

// Test 5: Server sends text message
#[test]
#[traced_test]
fn test_server_sends_text_message() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let request = parse_simple_request(&buffer[..bytes_read]).unwrap();
        let (response_bytes, _) = WebSocketUpgrade::accept(&request, None).unwrap();

        // Send all response bytes
        for chunk in &response_bytes {
            stream.write_all(chunk).unwrap();
        }
        stream.flush().unwrap();

        // Wrap stream for WebSocket communication
        let raw_stream = RawStream::from_tcp(stream.try_clone().unwrap()).unwrap();
        let shared_stream = SharedByteBufferStream::ref_cell(raw_stream);
        let mut conn = WebSocketServerConnection::new(shared_stream);

        // Send text message (server doesn't mask)
        conn.send(WebSocketMessage::Text("Hello from server!".to_string())).unwrap();

        // Keep stream alive long enough for client to read
        std::thread::sleep(Duration::from_millis(100));
    });

    thread::sleep(Duration::from_millis(50));
    let mut client = TcpStream::connect(&addr).unwrap();

    // Send upgrade
    let upgrade_request = "GET /chat HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Upgrade: websocket\r\n\
                           Connection: Upgrade\r\n\
                           Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                           Sec-WebSocket-Version: 13\r\n\
                           \r\n";

    client.write_all(upgrade_request.as_bytes()).unwrap();
    client.flush().unwrap();

    // Read handshake - read until \r\n\r\n
    let _ = read_http_response_headers(&mut client).unwrap();

    // Read frame header
    let mut frame_header = [0u8; 2];
    client.read_exact(&mut frame_header).unwrap();

    assert_eq!(frame_header[0], 0x81);
    let payload_len = (frame_header[1] & 0x7F) as usize;
    assert_eq!(payload_len, 18);

    let mut payload = vec![0u8; payload_len];
    client.read_exact(&mut payload).unwrap();

    assert_eq!(&payload, b"Hello from server!");

    server_handle.join().unwrap();
}

// Test 6: Server handles close frame
#[test]
#[traced_test]
fn test_server_handles_close_frame() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let request = parse_simple_request(&buffer[..bytes_read]).unwrap();
        let (response_bytes, _) = WebSocketUpgrade::accept(&request, None).unwrap();

        for chunk in &response_bytes {
            send_response(&mut stream, chunk).unwrap();
        }

        // Read close frame from client
        let mut frame_header = [0u8; 2];
        if stream.read_exact(&mut frame_header).is_ok() {
            // Read payload (close frame has code + optional reason)
            let payload_len = (frame_header[1] & 0x7F) as usize;
            let mut payload = vec![0u8; payload_len];
            stream.read_exact(&mut payload).ok();

            // Send close response (unmasked)
            let mut response = vec![0x88, payload_len as u8];
            response.extend_from_slice(&payload);
            stream.write_all(&response).ok();
            stream.flush().ok();
        }
    });

    thread::sleep(Duration::from_millis(50));
    let mut client = TcpStream::connect(&addr).unwrap();

    // Send upgrade
    let upgrade_request = "GET /chat HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Upgrade: websocket\r\n\
                           Connection: Upgrade\r\n\
                           Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                           Sec-WebSocket-Version: 13\r\n\
                           \r\n";

    client.write_all(upgrade_request.as_bytes()).unwrap();
    client.flush().unwrap();

    // Read handshake
    let _ = read_http_response(&mut client).unwrap();

    // Send close frame (masked for client)
    let mask = [0x01, 0x02, 0x03, 0x04];
    let close_payload = [0x03, 0xE8, b'b', b'y', b'e']; // 1000, "bye"
    let mut masked_payload = close_payload.to_vec();
    for (i, byte) in masked_payload.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }

    let mut frame = vec![0x88, 0x80 | 0x05]; // FIN=1, close, masked, len=5
    frame.extend_from_slice(&mask);
    frame.extend_from_slice(&masked_payload);

    client.write_all(&frame).unwrap();
    client.flush().unwrap();

    // Read close response
    let mut close_response = [0u8; 1024];
    let close_read = client.read(&mut close_response).ok();
    assert!(close_read.unwrap() >= 2);
    assert_eq!(close_response[0], 0x88);

    server_handle.join().unwrap();
}

// Test 7: Server rejects wrong method
#[test]
#[traced_test]
fn test_server_rejects_wrong_method() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let request = parse_simple_request(&buffer[..bytes_read]).unwrap();
        let is_upgrade = WebSocketUpgrade::is_upgrade_request(&request);
        assert!(!is_upgrade, "POST should not be detected as upgrade");

        send_response(&mut stream, b"HTTP/1.1 400 Bad Request\r\n\r\n").unwrap();
    });

    thread::sleep(Duration::from_millis(50));
    let mut client = TcpStream::connect(&addr).unwrap();

    let post_request = "POST /chat HTTP/1.1\r\n\
                        Host: localhost\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\
                        Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                        Sec-WebSocket-Version: 13\r\n\
                        \r\n";

    client.write_all(post_request.as_bytes()).unwrap();
    client.flush().unwrap();

    let mut response = [0u8; 1024];
    let bytes_read = client.read(&mut response).unwrap();
    let response_str = String::from_utf8_lossy(&response[..bytes_read]);

    assert!(response_str.contains("400"));

    server_handle.join().unwrap();
}

// Test 8: Server rejects missing key
#[test]
#[traced_test]
fn test_server_rejects_missing_key() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());

    let server_handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();

        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let request = parse_simple_request(&buffer[..bytes_read]).unwrap();
        let is_upgrade = WebSocketUpgrade::is_upgrade_request(&request);
        assert!(!is_upgrade, "Request without key should not be upgrade");

        let result = WebSocketUpgrade::accept(&request, None);
        assert!(result.is_err());

        send_response(&mut stream, b"HTTP/1.1 400 Bad Request\r\n\r\n").unwrap();
    });

    thread::sleep(Duration::from_millis(50));
    let mut client = TcpStream::connect(&addr).unwrap();

    let upgrade_request = "GET /chat HTTP/1.1\r\n\
                           Host: localhost\r\n\
                           Upgrade: websocket\r\n\
                           Connection: Upgrade\r\n\
                           Sec-WebSocket-Version: 13\r\n\
                           \r\n";

    client.write_all(upgrade_request.as_bytes()).unwrap();
    client.flush().unwrap();

    let mut response = [0u8; 1024];
    let bytes_read = client.read(&mut response).unwrap();
    let response_str = String::from_utf8_lossy(&response[..bytes_read]);

    assert!(response_str.contains("400"));

    server_handle.join().unwrap();
}
