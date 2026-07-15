//! `foundation_sshkit` — SSH orchestration toolkit.
//!
//! **WHAT:** Modeled on SSHKit (Basecamp's Ruby library). Provides `Host`,
//! `Command`, `CommandResult`, `Backend` trait (swappable backends: ssh2,
//! russh), `ConnectionPool`, and `Runner` strategies.
//!
//! **HOW:** Uses `ssh2` as the primary backend (already proven in
//! `foundation_testbed`). The `Backend` trait enables future backends.
//! Enable `russh-backend` feature for the pure-Rust russh alternative.

pub mod backends;
pub mod command;
pub mod host;
pub mod pool;
pub mod runner;

pub use backends::Backend;
pub use command::{Command, CommandResult};
pub use host::Host;
pub use pool::ConnectionPool;
pub use runner::Runner;

pub use backends::ssh2::{ChannelStream, Dialer, Ssh2Backend};
pub use pool::connect_session;

#[cfg(feature = "russh-backend")]
pub use backends::russh::RusshBackend;
