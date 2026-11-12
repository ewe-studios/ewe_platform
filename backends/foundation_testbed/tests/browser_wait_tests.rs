//! The browser runner pauses on the injected clock (GitHub issue 474).
//!
//! The runner waits 3 s for module scripts and then polls the `#output`
//! element every 500 ms until a 30 s deadline. It read the deadline from the
//! `Ctx` clock but paused with `std::thread::sleep`. With a simulated clock the
//! real sleeps never moved the clock, so the runner could not finish. Now the
//! pauses follow the clock, and a test steps them with `SimulatedClock`.
//!
//! The test needs a working Chromium binary. It skips when the browser cannot
//! launch or attach a page before the poll loop starts.

#![cfg(feature = "wasm")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_testbed::wasm::browser::run;
use foundation_testbed::wasm::cli::Browser;

/// Serve a page with no `#output` element until `stop` is set.
fn serve_blank_page(stop: Arc<AtomicBool>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.set_nonblocking(true).expect("non-blocking accept");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let _ = stream.set_nonblocking(false);
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let body = "<!doctype html><title>blank</title><p>no output here</p>";
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    port
}

#[test]
fn the_runner_ends_when_the_simulated_clock_passes_the_deadline() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone());
    let stop = Arc::new(AtomicBool::new(false));
    let port = serve_blank_page(stop.clone());

    // Step the clock one second every 100 ms of real time. The launch waits
    // on the clock too, so a browser that cannot start still ends the launch
    // on its 20 s clock deadline instead of a stuck run.
    let stepper_stop = stop.clone();
    let stepper_clock = clock.clone();
    let stepper = std::thread::spawn(move || {
        while !stepper_stop.load(Ordering::Relaxed) {
            stepper_clock.advance(Duration::from_secs(1));
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    let url = format!("http://127.0.0.1:{port}/index.html");
    let started = Instant::now();
    let result = run(&url, &Browser::Chrome, true, &ctx);
    let real = started.elapsed();
    stop.store(true, Ordering::Relaxed);
    let _ = stepper.join();

    let err = result.expect_err("the page never writes a test result");
    let message = err.to_string();
    if !message.contains("did not produce results") {
        // The browser failed before the poll loop: no usable Chromium here.
        eprintln!("skipped: the browser did not reach the poll loop ({message})");
        return;
    }
    // The 3 s settle and the 30 s deadline are clock time. The stepper moves
    // the clock 10 s per real second, so the real time stays below 33 s.
    assert!(
        real < Duration::from_secs(25),
        "the runner must not wait in real time: {real:?}"
    );
}
