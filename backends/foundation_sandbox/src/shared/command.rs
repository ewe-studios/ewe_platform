//! One program run and its results.

use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};

use crate::shared::serde_helpers::{base64_bytes, duration_millis_option};

/// How a command receives its standard input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StdinMode {
    /// The input is `/dev/null`.
    Null,
    /// The caller writes the input through the handle.
    Piped,
    /// The input is these bytes, written once.
    Bytes(#[serde(with = "base64_bytes")] Vec<u8>),
}

/// One program to run in a sandbox.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    /// The argument vector. `argv[0]` is the program. No shell runs.
    pub argv: Vec<String>,
    /// The environment entries, added over the sandbox environment. A later
    /// key wins.
    pub env: Vec<(String, String)>,
    /// The working directory, or `None` for the sandbox work directory.
    pub cwd: Option<String>,
    /// How the program receives its input.
    pub stdin: StdinMode,
    /// The time limit, or `None` for no time limit.
    #[serde(with = "duration_millis_option", default)]
    pub timeout: Option<Duration>,
    /// The user of the program: `"uid"`, `"uid:gid"`, `"name"` or
    /// `"name:group"` (feature 12). `None` runs as root, or as the image user
    /// when the sandbox came from an image.
    #[serde(default)]
    pub user: Option<String>,
}

impl Command {
    /// A command from an argument vector.
    ///
    /// Every other field is empty, or [`StdinMode::Null`] for the input.
    #[must_use]
    pub fn new<I, S>(argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            argv: argv.into_iter().map(Into::into).collect(),
            env: Vec::new(),
            cwd: None,
            stdin: StdinMode::Null,
            timeout: None,
            user: None,
        }
    }

    /// The same command with this user.
    #[must_use]
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }
}

/// How a program ended.
///
/// Exactly one of the exit code and the signal is present. Make a value with
/// [`ExitStatus::code`] or [`ExitStatus::signal`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ExitStatus {
    code: Option<i32>,
    signal: Option<i32>,
}

impl ExitStatus {
    /// A program that exited with a code.
    #[must_use]
    pub fn code(code: i32) -> Self {
        Self {
            code: Some(code),
            signal: None,
        }
    }

    /// A program that a signal ended.
    #[must_use]
    pub fn signal(signal: i32) -> Self {
        Self {
            code: None,
            signal: Some(signal),
        }
    }

    /// The exit code, when the program exited with one.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.code
    }

    /// The signal, when a signal ended the program.
    #[must_use]
    pub fn terminating_signal(&self) -> Option<i32> {
        self.signal
    }
}

impl<'de> Deserialize<'de> for ExitStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            code: Option<i32>,
            signal: Option<i32>,
        }

        let wire = Wire::deserialize(deserializer)?;
        match (wire.code, wire.signal) {
            (Some(code), None) => Ok(Self::code(code)),
            (None, Some(signal)) => Ok(Self::signal(signal)),
            _ => Err(serde::de::Error::custom(
                "exactly one of code and signal must be present",
            )),
        }
    }
}

/// One event of a running program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecEvent {
    /// Bytes on standard output.
    Stdout(#[serde(with = "base64_bytes")] Vec<u8>),
    /// Bytes on standard error.
    Stderr(#[serde(with = "base64_bytes")] Vec<u8>),
    /// The program ended. This event is last.
    Exit(ExitStatus),
}

/// The captured result of one command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecOutput {
    /// The captured standard output.
    #[serde(with = "base64_bytes")]
    pub stdout: Vec<u8>,
    /// The captured standard error.
    #[serde(with = "base64_bytes")]
    pub stderr: Vec<u8>,
    /// How the program ended.
    pub status: ExitStatus,
    /// Whether standard output was cut at the capture limit.
    pub stdout_truncated: bool,
    /// Whether standard error was cut at the capture limit.
    pub stderr_truncated: bool,
}
