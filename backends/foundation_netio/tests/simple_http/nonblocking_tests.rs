//! `IdleDeadline` and `PendingBytes` unit tests (moved from `src/http/tasks/nonblocking.rs`).

use std::io::Write;
use std::time::Duration;

use foundation_netio::http::nonblocking::{IdleDeadline, IoStep, PendingBytes};

/// A writer that accepts `budget` bytes, then reports `WouldBlock`.
struct Throttled {
    accepted: Vec<u8>,
    budget: usize,
}

impl Write for Throttled {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.budget == 0 {
            return Err(std::io::ErrorKind::WouldBlock.into());
        }
        let n = buf.len().min(self.budget).min(3);
        self.accepted.extend_from_slice(&buf[..n]);
        self.budget -= n;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn pending_bytes_resume_exactly_where_the_socket_stopped() {
    let mut out = Throttled {
        accepted: Vec::new(),
        budget: 5,
    };
    let mut pending = PendingBytes::new(b"hello world".to_vec());

    let (moved, step) = pending.write_to(&mut out).expect("first write");
    assert_eq!((moved, step), (5, IoStep::Blocked));
    assert!(!pending.is_empty());

    out.budget = 100;
    let (moved, step) = pending.write_to(&mut out).expect("second write");
    assert_eq!((moved, step), (6, IoStep::Done));
    assert_eq!(out.accepted, b"hello world", "no byte lost or duplicated");
}

#[test]
fn idle_deadline_rearms_after_progress() {
    let mut idle = IdleDeadline::new(Duration::from_millis(0));
    assert!(!idle.expired(), "not armed yet");
    let first = idle.arm();
    assert_eq!(idle.arm(), first, "arming is idempotent");
    assert!(idle.expired(), "zero budget expires at once");
    idle.progress();
    assert!(!idle.expired(), "progress clears the deadline");
}
