//! `foundation_reactor` — the shared I/O readiness reactor.
//!
//! WHY: the reactor (the process-global selector and its readiness cache) must
//! sit *below* `foundation_netio` so that both the server and client transport
//! layers can park on readiness without a dependency cycle. Before this crate,
//! the reactor lived in `foundation_nativeapis`, which depends on
//! `foundation_db`, which depends on `foundation_netio` — so netio could never
//! reach it.
//!
//! WHAT: [`poll`] (the epoll/kqueue/io_uring selector), [`Ready`], the shared
//! [`Reactor`] and its [`SharedReadiness`] view, and [`FdReadiness`] (the
//! `EventReadiness` that can also wake a `std::task::Waker`).
//!
//! HOW: `foundation_nativeapis` and `foundation_iogate` re-export these paths
//! for backward compatibility. `foundation_netio` depends on this crate
//! directly. The graph stays acyclic because this crate depends only on
//! `foundation_core`, `libc`, `windows-sys`, and `tracing`.

pub mod poll;

#[cfg(unix)]
pub mod ready;

#[cfg(unix)]
pub mod token;

#[cfg(unix)]
pub mod reactor;

#[cfg(unix)]
pub mod fd_readiness;

#[cfg(all(target_os = "linux", feature = "uring"))]
pub mod send_pool;

pub use poll::{Backend, BackendPreference, Events, Interest, Poll, Registry, Token, Waker};

#[cfg(unix)]
pub use poll::SourceFd;

#[cfg(unix)]
pub use ready::Ready;

#[cfg(unix)]
pub use reactor::{AlreadyInitialised, Reactor, SharedReadiness};

#[cfg(unix)]
pub use fd_readiness::FdReadiness;

#[cfg(unix)]
pub use token::next_token;
