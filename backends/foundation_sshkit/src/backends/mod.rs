//! SSH transport backends — swappable implementation.

use std::path::Path;

use crate::command::{Command, CommandResult};
use crate::host::Host;

/// SSH transport backend.
pub trait Backend: Send + Sync {
    fn execute(&self, host: &Host, cmd: &Command) -> Result<CommandResult, String>;
    fn upload(&self, host: &Host, local: &Path, remote: &Path) -> Result<(), String>;
    fn download(&self, host: &Host, remote: &Path, local: &Path) -> Result<(), String>;
}

pub mod ssh2;

pub use ssh2::{ChannelStream, Dialer, Ssh2Backend};
