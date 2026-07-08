//! HTTP/2 streaming echo — all 4 RPC modes over raw h2c.
//! Each call opens a fresh connection (same pattern as unary_echo/server_streaming).
//!
//! Run with: `cargo run -p foundation_connectrpc --example h2_echo`

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use bytes::Bytes;
use foundation_netio::http2::connection::{H2Connection, H2Request, H2Response};
use foundation_netio::http2::frame::*;
use foundation_netio::http2::hpack;

fn start_server(addr: std::net::SocketAddr) -> thread::JoinHandle<()> {
    let listener = TcpListener::bind(addr).expect("bind");
    listener.set_nonblocking(true).ok();
    thread::spawn(move || loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = thread::spawn(move || serve(stream));
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    })
}

fn serve(stream: TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut c = H2Connection::new(stream, true);
    c.server_handshake()?;

    loop {
        c.flush()?;
        let (head, payload) = match c.read_frame() {
            Ok(v) => v,
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };
        if head.kind != Kind::Headers { continue; }
        let hf = HeadersFrame::parse(&head, &payload)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut dec = hpack::Decoder::new();
        let d = dec.decode(&hf.header_block).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut path = String::new();
        for (n, v) in &d { if n.as_ref() == b":path" { path = String::from_utf8_lossy(v).into(); } }
        let sid = head.stream_id;
        let es = hf.flags & headers_flags::END_STREAM != 0;
        println!("[server] stream={sid} path={path}");

        match path.as_str() {
            "/echo/Echo" => {
                c.send_response(sid, H2Response { status: 200, headers: vec![], body: Some(Bytes::from("ECHO")), end_stream: true })?; c.flush()?;
            }
            "/echo/Count" => {
                c.send_headers_response(sid, 200, &[], false)?;
                for i in 0..4 { c.send_data_frame(sid, format!("{i}").as_bytes(), i == 3)?; }
            }
            "/echo/Collect" => {
                let mut parts: Vec<String> = Vec::new();
                if !es { loop { match c.recv_data_frame()? { Some((_, d, e)) => { parts.push(String::from_utf8_lossy(&d).into()); if e { break; } } None => break, } } }
                c.send_response(sid, H2Response { status: 200, headers: vec![], body: Some(Bytes::from(format!("[{parts:?}]"))), end_stream: true })?; c.flush()?;
            }
            "/echo/Chat" => {
                c.send_headers_response(sid, 200, &[], false)?;
                let mut seq = 0;
                if !es { loop { match c.recv_data_frame()? { Some((_, d, e)) => { let t = String::from_utf8_lossy(&d); c.send_data_frame(sid, format!("r{seq}:{t}").as_bytes(), e)?; c.flush()?; seq += 1; if e { break; } } None => break, } } }
            }
            _ => {}
        }
    }
    Ok(())
}

fn client_connect(addr: std::net::SocketAddr) -> H2Connection<TcpStream> {
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).expect("connect");
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let mut c = H2Connection::new(stream, false);
    c.client_handshake().expect("handshake");
    c
}

fn main() {
    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let a = l.local_addr().expect("addr");
        drop(l);
        a
    };
    start_server(addr);
    thread::sleep(Duration::from_millis(100));
    println!("h2c server on {addr}");

    let auth = Bytes::from(format!("{addr}"));
    let req = |path: &str, es: bool| H2Request {
        method: Bytes::from_static(b"GET"), scheme: Bytes::from_static(b"http"),
        authority: auth.clone(), path: Bytes::copy_from_slice(path.as_bytes()),
        headers: vec![], body: None, end_stream: es,
    };

    // 1. unary
    println!("\n─── 1. unary ───");
    let mut c = client_connect(addr);
    c.send_request(req("/echo/Echo", true)).expect("send");
    let (_, r) = c.recv_response().expect("recv").expect("resp");
    let body = if !r.end_stream { match c.recv_data_frame().expect("recv data") { Some((_, d, _)) => String::from_utf8_lossy(&d).to_string(), None => String::new() } } else { String::new() };
    println!("[client] unary: {body:?}");
    assert_eq!(body, "ECHO");

    // 2. server-stream
    println!("\n─── 2. server-stream ───");
    let mut c = client_connect(addr);
    c.send_request(req("/echo/Count", true)).expect("send");
    c.recv_response().expect("recv headers");
    let mut chunks = Vec::new();
    loop { match c.recv_data_frame().expect("recv") { Some((_, d, e)) => { chunks.push(String::from_utf8_lossy(&d).to_string()); if e { break; } } None => break, } }
    println!("[client] server-stream: {chunks:?}");
    assert_eq!(chunks, vec!["0","1","2","3"]);

    // 3. client-stream
    println!("\n─── 3. client-stream ───");
    let mut c = client_connect(addr);
    let sid = c.send_request(req("/echo/Collect", false)).expect("send");
    c.send_data_frame(sid, b"a", false).unwrap(); c.send_data_frame(sid, b"b", false).unwrap(); c.send_data_frame(sid, b"c", true).unwrap();
    let (_, r) = c.recv_response().expect("recv").expect("resp");
    let body = if !r.end_stream { match c.recv_data_frame().expect("recv body") { Some((_, d, _)) => String::from_utf8_lossy(&d).to_string(), None => String::new() } } else { String::new() };
    println!("[client] client-stream: {body}");
    assert!(body.contains("a") && body.contains("b") && body.contains("c"));

    // 4. bidi-stream
    println!("\n─── 4. bidi-stream ───");
    let mut c = client_connect(addr);
    let sid = c.send_request(req("/echo/Chat", false)).expect("send");
    c.recv_response().expect("recv headers");
    for i in 0..3 {
        c.send_data_frame(sid, format!("m{i}").as_bytes(), i == 2).unwrap();
        match c.recv_data_frame().expect("recv echo") { Some((_, d, _)) => println!("[client]   echo: {}", String::from_utf8_lossy(&d)), None => break, }
    }

    println!("\n─── all 4 RPC modes verified ✓ ───");
}
