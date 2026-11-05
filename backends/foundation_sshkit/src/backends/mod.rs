//! SSH transport backends — swappable implementation.

use std::path::Path;

use crate::command::{Command, CommandResult};
use crate::error::SshKitResult;
use crate::host::Host;

/// SSH transport backend.
pub trait Backend: Send + Sync {
    fn execute(&self, host: &Host, cmd: &Command) -> SshKitResult<CommandResult>;
    fn upload(&self, host: &Host, local: &Path, remote: &Path) -> SshKitResult<()>;
    fn download(&self, host: &Host, remote: &Path, local: &Path) -> SshKitResult<()>;
}

pub mod ssh2;

pub use ssh2::{ChannelStream, Dialer, Ssh2Backend};
