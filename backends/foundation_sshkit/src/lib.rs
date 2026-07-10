//! `foundation_sshkit` — SSH orchestration toolkit.
//!
//! **WHY:** The workspace uses SSH in multiple places (testbed guest communication,
//! cloud deployment provisioning, bollard remote transport). Each manages
//! connections independently. This crate provides a unified SSH abstraction
//! with connection pooling, host parsing, command execution, and runner strategies.
//!
//! **WHAT:** Modeled on SSHKit (Basecamp's Ruby library). Provides `Host`,
//! `Command`, `CommandResult`, `Backend` trait (swappable backends: ssh2,
//! russh), `ConnectionPool`, and `Runner` strategies (parallel, sequential,
//! grouped).
//!
//! **HOW:** Uses `ssh2` as the primary backend (already proven in
//! `foundation_testbed`). The `Backend` trait enables future backends.

pub mod backend;
pub mod command;
pub mod host;
pub mod pool;
pub mod runner;

pub use backend::{Backend, Ssh2Backend};
pub use command::{Command, CommandResult};
pub use host::Host;
pub use pool::ConnectionPool;
pub use runner::Runner;
