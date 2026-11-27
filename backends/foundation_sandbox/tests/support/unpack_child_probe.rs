//! The probe unpack child of issue #977.
//!
//! It behaves like `agent-slave --unpack-child` for the protocol only: it
//! writes the `ready` line, waits, then drains its standard input and writes
//! one `ok` result. It does not `chroot`, so the test needs no root.
//!
//! The wait is the point. It lets the writer fill the pipe, so a blocking
//! write holds its worker and a parking write releases it.

#[cfg(target_os = "linux")]
fn main() {
    use std::io::{Read as _, Write as _};
    use std::time::Duration;

    // The delay before the drain: long enough for the test to observe that a
    // queued task runs while every writer waits.
    const DRAIN_DELAY: Duration = Duration::from_millis(1_000);

    let mut stdout = std::io::stdout();
    if writeln!(stdout, "ready").is_err() || stdout.flush().is_err() {
        std::process::exit(1);
    }
    std::thread::sleep(DRAIN_DELAY);
    let mut stdin = std::io::stdin();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut bytes = 0_u64;
    loop {
        match stdin.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => bytes = bytes.saturating_add(u64::try_from(read).unwrap_or(u64::MAX)),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => std::process::exit(1),
        }
    }
    let outcome = format!(r#"{{"status":"ok","entries":1,"bytes":{bytes}}}"#);
    if writeln!(stdout, "{outcome}").is_err() || stdout.flush().is_err() {
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("the unpack child probe needs Linux");
    std::process::exit(1);
}
