//! The control socket serves each connection as a future, not a thread (spec 70
//! feature 65, E4).
//!
//! WHY: it started one OS thread per connection and slept 50 ms between accept
//! tries.
//!
//! WHAT: 50 clients connect and send half a request line, so 50 connections are
//! open at once. The test counts the process threads, then finishes every
//! request and checks each answer. Linux only (`/proc/self/task`).

#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_proxy::config::ServiceConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};

fn thread_count() -> usize {
    std::fs::read_dir("/proc/self/task")
        .expect("read /proc/self/task")
        .count()
}

#[valtron_test(seed = 68, threads = 1, timeout = 60000)]
fn control_socket_does_not_spawn_threads() {
    let sock = std::env::temp_dir()
        .join(format!("ewe_proxy_ctl_threads_{}.sock", std::process::id()))
        .to_string_lossy()
        .into_owned();
    let proxy = ProxyServer::start(
        ProxyConfig::new("test.local", "127.0.0.1")
            .bind("127.0.0.1:0")
            .control_socket(&sock)
            .service(ServiceConfig::new("echo", "test.local").backend("http://127.0.0.1:19101")),
        foundation_core::valtron::Ctx::host(),
    )
    .block()
    .expect("start proxy");

    // Warm up: one full request, so lazy threads exist before the baseline.
    let mut warm = UnixStream::connect(&sock).expect("connect");
    writeln!(warm, r#"{{"command":"status"}}"#).expect("write");
    let mut line = String::new();
    BufReader::new(warm).read_line(&mut line).expect("read");
    assert!(line.contains("\"ok\":true"), "warm-up answer: {line}");
    std::thread::sleep(Duration::from_millis(200));

    let before = thread_count();
    let mut open: Vec<UnixStream> = (0..50)
        .map(|_| {
            let mut s = UnixStream::connect(&sock).expect("connect");
            s.set_read_timeout(Some(Duration::from_secs(10))).ok();
            s.write_all(br#"{"command":"#).expect("half a request");
            s
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let during = thread_count();

    for s in &mut open {
        s.write_all(b"\"status\"}\n").expect("rest of request");
    }
    for s in open {
        let mut line = String::new();
        BufReader::new(s).read_line(&mut line).expect("read answer");
        assert!(line.contains("\"ok\":true"), "answer: {line}");
    }
    proxy.shutdown();

    assert!(
        during <= before + 3,
        "50 open control connections grew the process from {before} to {during} threads"
    );
}
