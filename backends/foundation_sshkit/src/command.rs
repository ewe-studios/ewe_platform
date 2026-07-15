//! Shell command to execute on a remote host.

use std::path::PathBuf;
use std::time::Duration;

/// A shell command to execute on a remote host.
#[derive(Debug, Clone)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub working_dir: Option<PathBuf>,
    pub user: Option<String>,
    pub pty: bool,
    pub in_background: bool,
}

impl Command {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
            working_dir: None,
            user: None,
            pty: false,
            in_background: false,
        }
    }

    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    #[must_use]
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(|a| a.into()));
        self
    }

    #[must_use]
    pub fn env(mut self, key: &str, val: &str) -> Self {
        self.env.push((key.to_string(), val.to_string()));
        self
    }

    #[must_use]
    pub fn within(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    #[must_use]
    pub fn as_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    #[must_use]
    pub fn pty(mut self) -> Self {
        self.pty = true;
        self
    }

    #[must_use]
    pub fn in_background(mut self) -> Self {
        self.in_background = true;
        self
    }

    /// Wrap inside `bash -c "..."`.
    #[must_use]
    pub fn bash(self) -> String {
        format!("bash -c {:?}", self.to_shell_command())
    }

    /// Wrap inside `cmd /c "..."` (Windows).
    #[must_use]
    pub fn cmd(self) -> String {
        format!("cmd /c {:?}", self.to_shell_command())
    }

    /// Render to a shell command string.
    pub fn to_shell_command(&self) -> String {
        let mut parts = Vec::new();

        // Environment
        for (k, v) in &self.env {
            parts.push(format!("export {k}='{v}'"));
        }

        // Working directory
        if let Some(ref dir) = self.working_dir {
            parts.push(format!("cd '{}'", dir.display()));
        }

        // Main command
        let cmd = if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        };

        // User switch
        let cmd = if let Some(ref user) = self.user {
            format!("sudo -u {user} -- {cmd}")
        } else {
            cmd
        };

        // Background
        let cmd = if self.in_background {
            format!("nohup {cmd} > /dev/null 2>&1 &")
        } else {
            cmd
        };

        if parts.is_empty() {
            cmd
        } else {
            parts.push(cmd);
            parts.join(" && ")
        }
    }
}

/// Result of a command execution on a remote host.
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub runtime: Duration,
    pub host: String,
}

impl CommandResult {
    pub fn is_success(&self) -> bool { self.exit_code == 0 }
}
