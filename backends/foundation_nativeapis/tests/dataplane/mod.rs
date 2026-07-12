//! Integration tests for the WireGuard overlay data plane (spec-55, feature 00).
//!
//! WHY: Prove that two smoltcp `NetStack`s can exchange application traffic **entirely
//! in userspace** — no kernel socket for the overlay range, no privileges — with the
//! `Tunn` boundary stood in for by an in-memory packet queue.
//!
//! WHAT: A TCP loopback round-trip across two bridged stacks, waker firing on data
//! availability, `poll` deadline behaviour, and a privileged (ignored) kernel-TUN test.
//!
//! HOW: We manually shuttle each stack's drained outbound IP packets into the other's
//! inbound queue (the "wire"), pumping `poll` on both until the handshake and transfer
//! complete.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use foundation_nativeapis::dataplane::netstack::OverlayStream;
use foundation_nativeapis::dataplane::{DataPlane, NetStack, NetStackConfig};
use tracing_test::traced_test;

const SERVER_IP: &str = "10.9.0.2";
const CLIENT_IP: &str = "10.9.0.1";
const SERVER_PORT: u16 = 8080;

fn stack(ip: &str) -> NetStack {
    NetStack::new(NetStackConfig::new(ip.parse().expect("ip"), 24))
}

fn server_addr() -> SocketAddr {
    format!("{SERVER_IP}:{SERVER_PORT}").parse().expect("addr")
}

/// One bidirectional exchange tick: poll both stacks, move each stack's outbound IP
/// packets to the other's inbound queue (the stand-in for `Tunn` + the UDP wire), then
/// poll again so freshly delivered packets are processed.
fn exchange(a: &mut NetStack, b: &mut NetStack) {
    let now = Instant::now();
    a.poll(now);
    b.poll(now);

    let mut buffer: Vec<Vec<u8>> = Vec::new();
    a.drain_outbound_ip(&mut |p| buffer.push(p.to_vec()));
    for packet in &buffer {
        b.inject_inbound_ip(packet);
    }
    buffer.clear();
    b.drain_outbound_ip(&mut |p| buffer.push(p.to_vec()));
    for packet in &buffer {
        a.inject_inbound_ip(packet);
    }

    a.poll(now);
    b.poll(now);
}

/// Write `data` in full across the overlay stream, pumping the wire between attempts.
fn write_all(stream: &OverlayStream, data: &[u8], a: &mut NetStack, b: &mut NetStack) {
    let mut sent = 0;
    for _ in 0..500 {
        if sent < data.len() {
            match stream.write(&data[sent..]) {
                Ok(n) => sent += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => panic!("overlay write failed: {e}"),
            }
        }
        exchange(a, b);
        if sent >= data.len() {
            break;
        }
    }
    assert_eq!(sent, data.len(), "not all bytes were sent");
    // Extra ticks so the last data segment is delivered and acknowledged.
    for _ in 0..20 {
        exchange(a, b);
    }
}

/// Read exactly `len` bytes from the overlay stream, pumping the wire between attempts.
fn read_exact(stream: &OverlayStream, len: usize, a: &mut NetStack, b: &mut NetStack) -> Vec<u8> {
    let mut got = Vec::new();
    let mut scratch = [0u8; 2048];
    for _ in 0..500 {
        match stream.read(&mut scratch) {
            Ok(0) => break,
            Ok(n) => got.extend_from_slice(&scratch[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("overlay read failed: {e}"),
        }
        if got.len() >= len {
            break;
        }
        exchange(a, b);
    }
    got
}

#[test]
#[traced_test]
fn netstack_tcp_loopback_roundtrip() {
    let mut server = stack(SERVER_IP);
    let mut client = stack(CLIENT_IP);

    let mut listener = server.tcp_listen(server_addr()).expect("listen");
    let client_stream = client.tcp_connect(server_addr()).expect("connect");

    // Drive the three-way handshake and accept the server side.
    let mut server_stream = None;
    for _ in 0..300 {
        exchange(&mut client, &mut server);
        if server_stream.is_none() {
            if let Ok(accepted) = listener.accept() {
                server_stream = Some(accepted);
            }
        }
        if server_stream.is_some() && client_stream.may_send() {
            break;
        }
    }
    let server_stream = server_stream.expect("server accepted a connection");
    assert!(client_stream.is_active(), "client side established");
    assert_eq!(server_stream.peer_addr().ip().to_string(), CLIENT_IP);

    // client -> server
    let request = b"GET /overlay HTTP/1.0\r\n\r\n";
    write_all(&client_stream, request, &mut client, &mut server);
    let received = read_exact(&server_stream, request.len(), &mut server, &mut client);
    assert_eq!(&received, request, "server received the client's bytes intact");

    // server -> client (echo back reversed length so it differs)
    let response = b"HTTP/1.0 200 OK\r\ncontent-length: 2\r\n\r\nhi";
    write_all(&server_stream, response, &mut server, &mut client);
    let echoed = read_exact(&client_stream, response.len(), &mut client, &mut server);
    assert_eq!(&echoed, response, "client received the server's bytes intact");
}

#[test]
#[traced_test]
fn netstack_read_waker_fires_on_data() {
    let mut server = stack(SERVER_IP);
    let mut client = stack(CLIENT_IP);

    let mut listener = server.tcp_listen(server_addr()).expect("listen");
    let client_stream = client.tcp_connect(server_addr()).expect("connect");

    let mut server_stream = None;
    for _ in 0..300 {
        exchange(&mut client, &mut server);
        if server_stream.is_none() {
            if let Ok(accepted) = listener.accept() {
                server_stream = Some(accepted);
            }
        }
        if server_stream.is_some() && client_stream.may_send() {
            break;
        }
    }
    let server_stream = server_stream.expect("accepted");

    // Register a one-shot read waker on the server side; it must fire once data lands.
    let woken = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&woken);
    server_stream.set_read_waker(Arc::new(move || flag.store(true, Ordering::SeqCst)));

    assert!(!woken.load(Ordering::SeqCst), "waker not fired before data");

    write_all(&client_stream, b"ping", &mut client, &mut server);

    assert!(
        woken.load(Ordering::SeqCst),
        "read waker fired once data became available"
    );
}

#[test]
#[traced_test]
fn netstack_udp_loopback_datagram() {
    let mut server = stack(SERVER_IP);
    let mut client = stack(CLIENT_IP);

    let server_sock = server.udp_bind(server_addr()).expect("bind server udp");
    let client_addr: SocketAddr = format!("{CLIENT_IP}:5353").parse().expect("addr");
    let client_sock = client.udp_bind(client_addr).expect("bind client udp");

    // client -> server datagram
    let payload = b"overlay-datagram";
    client_sock
        .send_to(payload, server_addr())
        .expect("client send");

    let mut recv_buf = [0u8; 128];
    let mut received = None;
    for _ in 0..100 {
        exchange(&mut client, &mut server);
        match server_sock.recv_from(&mut recv_buf) {
            Ok((n, from)) => {
                received = Some((recv_buf[..n].to_vec(), from));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("server recv failed: {e}"),
        }
    }
    let (bytes, from) = received.expect("server received the datagram");
    assert_eq!(&bytes, payload, "datagram payload intact");
    assert_eq!(from.ip().to_string(), CLIENT_IP, "source address preserved");
    assert_eq!(from.port(), 5353, "source port preserved");

    // server -> client reply to the observed source
    let reply = b"ack";
    server_sock.send_to(reply, from).expect("server reply");
    let mut got = None;
    for _ in 0..100 {
        exchange(&mut server, &mut client);
        match client_sock.recv_from(&mut recv_buf) {
            Ok((n, _)) => {
                got = Some(recv_buf[..n].to_vec());
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("client recv failed: {e}"),
        }
    }
    assert_eq!(got.as_deref(), Some(reply.as_slice()), "client received reply");
}

#[test]
#[traced_test]
fn netstack_poll_reports_idle_when_no_sockets() {
    let mut idle = stack(CLIENT_IP);
    // With no sockets and no traffic there is nothing to schedule.
    assert_eq!(idle.poll(Instant::now()), None, "idle stack has no deadline");
    assert_eq!(idle.socket_count(), 0);
}

// ---------------------------------------------------------------------------
// Kernel TUN (privileged; ignored without CAP_NET_ADMIN)
// ---------------------------------------------------------------------------

#[cfg(all(feature = "tun", target_os = "linux"))]
#[test]
#[traced_test]
#[ignore = "requires CAP_NET_ADMIN / root to open /dev/net/tun"]
fn tun_device_opens_on_linux() {
    use foundation_nativeapis::dataplane::tun::{TunConfig, TunDevice};

    let device = TunDevice::open(&TunConfig::new("ewe-test0")).expect("open tun");
    assert_eq!(device.name(), "ewe-test0");
    // A freshly opened device has nothing to read yet.
    let mut buf = [0u8; 2048];
    match device.read(&mut buf) {
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
        other => panic!("expected WouldBlock on empty tun, got {other:?}"),
    }
}
