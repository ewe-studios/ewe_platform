//! `foundation_iogate` — the I/O gate between netio transports and the
//! nativeapis reactor.
//!
//! WHY: the io_uring completion read path (Decision 14 F4 / Feature 48) needs a
//! type that owns *both* `foundation_netio` (the `Connection` enum and its
//! `CompletionReadWrite` seam) and `foundation_nativeapis` (the `Reactor`,
//! `RegisteredFd`, and the completion buffer ring). netio cannot depend on
//! nativeapis directly: `nativeapis → foundation_db → foundation_netio` would
//! close a dependency cycle. iogate sits *above* both crates, so every edge
//! points one way — `iogate → {netio, nativeapis}` — and the cycle never forms.
//!
//! WHAT: [`ServerIo`] (the operator-facing I/O-mode switch, always available)
//! plus, on native targets, [`native::CompletionSocket`] (a completion-backed
//! byte source), its `CompletionReadWrite` impl, and the accept-path helper that
//! turns an accepted `TcpStream` into the right netio `Connection`.
//!
//! HOW: `shared/` is compiled on every target; `native/` carries the reactor
//! wiring and is gated to non-wasm; `wasm/` is an empty placeholder so the
//! module layout matches `foundation_http` and `foundation_netio`.

// ── Shared (all targets) ────────────────────────────────────────────────
pub mod shared;

// ── Native, unix-only (reactor + completion socket) ─────────────────────
// The reactor, io_uring, and `Connection::Completion` are all unix concerns;
// non-unix native targets get `ServerIo` only.
#[cfg(all(not(target_family = "wasm"), unix))]
pub mod native;

// ── Wasm placeholder ────────────────────────────────────────────────────
#[cfg(target_family = "wasm")]
pub mod wasm;

pub use shared::ServerIo;

#[cfg(all(not(target_family = "wasm"), unix))]
pub use native::{accept_connection, init_reactor_for, CompletionSocket};
