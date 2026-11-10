//! gRPC health probe leak test — a silent backend must fail the probe promptly
//! and leak nothing.
//!
//! WHY: the old probe drove `grpc_health_check` on a throwaway thread and bounded
//! it with an `mpsc::Receiver::recv_timeout`. When the backstop tripped, the inner
//! future and the fire-and-forget H2 pump were abandoned: every probe kept one
//! open socket (and, in the stuck-timer case, one thread) until the peer's own
//! idle timeout. The fix bounds the H2 pump itself with a read timeout, so the
//! probe runs inline on the probe thread and the connection closes on its own.
//!
//! WHAT: a raw `TcpListener` writes one non-ACK h2 SETTINGS frame (so the client
//! handshake completes) and then stays silent. Twenty probes must each return
//! `false` in about the probe timeout — not the 10 s handshake timeout — and the
//! process must return to its baseline thread and file-descriptor counts.
//!
//! The baseline follows one warm-up probe. The first probe starts the two
//! process-wide helper threads that the probe path creates on first use: the
//! `valtron-timer` wheel that `sleep_async` needs, and the shared reactor's
//! `reactor-drain` thread when the H2 pump parks on fd readiness. They run
//! until the process exits by design, so they are not per-probe leaks. The
//! warm-up puts them in the baseline, and the check then covers only the growth
//! that the 20 measured probes add.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::valtron_test;
use foundation_proxy::health::probe_grpc_health;

/// One empty, non-ACK h2 SETTINGS frame: 3-byte length (0), type 4 (SETTINGS),
/// flags 0, stream id 0. This is all the client handshake needs before it moves
/// on to sending the request.
const SILENT_SETTINGS: [u8; 9] = [0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00];

/// Count open file descriptors from `/proc/self/fd` (Linux only).
#[cfg(target_os = "linux")]
fn fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd").map_or(0, std::iter::Iterator::count)
}

/// Read the `Threads:` line of `/proc/self/status` (Linux only).
#[cfg(target_os = "linux")]
fn thread_count() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("Threads:")
                    .and_then(|rest| rest.trim().parse::<usize>().ok())
            })
        })
        .unwrap_or(0)
}

#[valtron_test(seed = 63, threads = 8, timeout = 90_000)]
fn health_probe_of_silent_backend_times_out_and_leaks_nothing() {
    let timeout = Duration::from_millis(300);

    // A silent backend: it completes the client h2 handshake, then never answers.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind silent backend");
    listener
        .set_nonblocking(true)
        .expect("non-blocking listener");
    let port = listener.local_addr().expect("backend addr").port();
    let shutdown = Arc::new(AtomicBool::new(false));
    let accept_shutdown = Arc::clone(&shutdown);
    let accept_thread = std::thread::spawn(move || {
        loop {
            if accept_shutdown.load(Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    // The accepted socket inherits non-blocking from the
                    // listener on some platforms. Make it blocking so the
                    // read below waits for the client instead of failing fast.
                    let _ = stream.set_nonblocking(false);
                    // Complete the handshake, then read-and-discard until the
                    // client closes the connection (at its read timeout).
                    let _ = stream.write_all(&SILENT_SETTINGS);
                    let _ = stream.flush();
                    let mut buf = [0u8; 1024];
                    loop {
                        match stream.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {}
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => break,
            }
        }
    });

    let base_url = format!("grpc://127.0.0.1:{port}");

    // Warm up the probe path before the baseline. The first probe starts the
    // process-wide helper threads that the probe path creates on first use: the
    // `valtron-timer` thread that `sleep_async` needs, and — when the H2 pump
    // parks on fd readiness — the shared reactor's `reactor-drain` thread. Both
    // live until the process exits by design, so they are not per-probe leaks.
    // A probe before the baseline makes them part of it; any thread that the 20
    // measured probes add is then a real leak.
    #[cfg(target_os = "linux")]
    let fds_before_warmup = fd_count();
    assert!(
        !probe_grpc_health(&base_url, timeout, &foundation_core::valtron::Ctx::host()),
        "a silent backend must fail its health probe"
    );
    // The warm-up pump closes its socket just after the probe returns. Wait for
    // that descriptor to go, so a transient one is not in the baseline.
    #[cfg(target_os = "linux")]
    {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && fd_count() > fds_before_warmup {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // The accept thread and the helper threads are now part of the baseline
    // (both are constant for the test).
    #[cfg(target_os = "linux")]
    let (base_threads, base_fds) = (thread_count(), fd_count());

    let mut all_bounded = true;
    for _ in 0..20 {
        let start = Instant::now();
        let ok = probe_grpc_health(&base_url, timeout, &foundation_core::valtron::Ctx::host());
        let elapsed = start.elapsed();
        assert!(!ok, "a silent backend must fail its health probe");
        // The probe must finish in about `timeout`, not the 10 s handshake budget.
        let floor = timeout.saturating_sub(Duration::from_millis(50));
        let ceiling = timeout + Duration::from_secs(2);
        if elapsed < floor || elapsed > ceiling {
            all_bounded = false;
        }
    }
    assert!(
        all_bounded,
        "each probe must complete in about the probe timeout"
    );

    // The pump tasks close their sockets a moment after each probe returns. Wait
    // for the process to settle back to baseline before judging the leak.
    #[cfg(target_os = "linux")]
    {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline
            && (fd_count() > base_fds + 1 || thread_count() > base_threads + 1)
        {
            std::thread::sleep(Duration::from_millis(50));
        }
        let (final_threads, final_fds) = (thread_count(), fd_count());
        assert!(
            final_threads <= base_threads + 1,
            "thread count leaked: {base_threads} before, {final_threads} after"
        );
        assert!(
            final_fds <= base_fds + 1,
            "file-descriptor count leaked: {base_fds} before, {final_fds} after"
        );
    }

    shutdown.store(true, Ordering::SeqCst);
    let _ = accept_thread.join();
}
