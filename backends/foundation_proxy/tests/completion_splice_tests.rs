//! io_uring proxy relay: two Completion sockets spliced via `splice_bidirectional` (F50).
//!
//! WHY: The individual pieces (connect_completion, SEND_ZC, reactor wait) are each
//! tested in isolation. This proves the assembled relay — bytes flow both ways
//! between two Completion-backed sockets through the production splice path.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use foundation_iogate::{init_reactor_for, CompletionSocket, ServerIo};
use foundation_nativeapis::native::fd::Reactor;
use foundation_nativeapis::Token;
use foundation_proxy::passthrough::splice_bidirectional;

#[test]
#[serial_test::serial]
fn completion_splice_relays_bytes_both_directions() {
    if init_reactor_for(ServerIo::Completion).is_err() {
        eprintln!("io_uring unavailable — skipping completion splice test");
        return;
    }

    let server_b = TcpListener::bind("127.0.0.1:0").expect("bind server B");
    let addr_b = server_b.local_addr().expect("addr B");
    let server_d = TcpListener::bind("127.0.0.1:0").expect("bind server D");
    let addr_d = server_d.local_addr().expect("addr D");

    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ok = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let d_done = Arc::clone(&done);
    let d_ok = Arc::clone(&ok);

    let splice_thread = thread::spawn(move || {
        let reactor = Reactor::get().expect("reactor in splice thread");
        let registry = reactor.registry();
        let (b, _) = server_b.accept().expect("accept B");
        b.set_nonblocking(true).ok();
        let (d, _) = server_d.accept().expect("accept D");
        d.set_nonblocking(true).ok();

        let socket_b =
            CompletionSocket::completion(b, registry, Token(2), None).expect("register B");
        let socket_d =
            CompletionSocket::completion(d, registry, Token(3), None).expect("register D");

        splice_bidirectional(socket_b, socket_d, true);
        d_ok.store(true, std::sync::atomic::Ordering::Release);
        d_done.store(true, std::sync::atomic::Ordering::Release);
    });

    // Give the splice thread time to accept and register.
    thread::sleep(Duration::from_millis(200));

    let mut a = TcpStream::connect(addr_b).expect("connect A");
    a.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut c = TcpStream::connect(addr_d).expect("connect C");
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();

    // Write A → B, splice B → D, read C.
    a.write_all(b"hello from A").expect("write A");
    let mut buf = [0u8; 64];
    let n = c.read(&mut buf).expect("read C");
    assert_eq!(&buf[..n], b"hello from A");

    // Write C → D, splice D → B, read A.
    c.write_all(b"response from C").expect("write C");
    let n = a.read(&mut buf).expect("read A");
    assert_eq!(&buf[..n], b"response from C");

    drop(a);
    drop(c);

    for _ in 0..50 {
        if done.load(std::sync::atomic::Ordering::Acquire) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(ok.load(std::sync::atomic::Ordering::Acquire), "splice completed cleanly");
    splice_thread.join().ok();
}
