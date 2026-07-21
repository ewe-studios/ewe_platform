//! `foundation_sshkit` — SSH orchestration toolkit.
//!
//! **WHAT:** Modeled on SSHKit (Basecamp's Ruby library). Provides `Host`,
//! `Command`, `CommandResult`, the `Backend` trait, `ConnectionPool`, and
//! `Runner` strategies.
//!
//! **HOW:** Uses `ssh2` (libssh2) as the backend, already proven in
//! `foundation_testbed`. The `Backend` trait keeps the transport swappable.

pub mod backends;
pub mod command;
pub mod host;
pub mod pool;
pub mod powershell;
pub mod runner;
pub mod shell;

pub use backends::Backend;
pub use command::{Command, CommandResult};
pub use host::Host;
pub use pool::ConnectionPool;
pub use runner::Runner;

pub use backends::ssh2::{ChannelStream, Dialer, Ssh2Backend};
pub use pool::connect_session;
