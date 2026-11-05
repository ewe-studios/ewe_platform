use derive_more::{Display, Error};
use foundation_errstacks::ErrorTrace;

use serde::{Deserialize, Serialize};

use serde_with::skip_serializing_none;

pub type Result<T> = std::result::Result<T, ErrorTrace<ConfigError>>;

#[derive(Debug, Display, Error)]
pub enum ConfigError {
    #[display("Configuration file is not found")]
    FileNotFound,

    #[display("Configuration file could not be read")]
    FailedReading,

    #[display("Configuration file format not supported")]
    UnknownFormat,

    #[display("Configuration is invalid due to: {_0}")]
    BadConfiguration(serde_json::Error),

    #[display("Could not serialize configuration due to: {_0}")]
    FailedDeserialization(serde_json::Error),
}

impl From<ConfigError> for ErrorTrace<ConfigError> {
    fn from(err: ConfigError) -> Self {
        ErrorTrace::new(err)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum CommandExpectation {
    #[serde(rename = "exit")]
    Exit,

    #[serde(rename = "skip")]
    Skip,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CommandDescription {
    pub command: Vec<String>,
    pub if_failed: Option<CommandExpectation>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DirWatcher {
    pub dir: String,
    pub recursive: bool,
    pub debounce: u16,
    pub after_change: Option<Vec<CommandDescription>>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FileWatcher {
    pub file: String,
    pub debounce: u16,
    pub after_change: Option<Vec<CommandDescription>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum Watcher {
    #[serde(rename = "file")]
    File(FileWatcher),

    #[serde(rename = "dir")]
    Dir(DirWatcher),
}

impl Watcher {
    #[must_use]
    pub fn debounce(&self) -> u16 {
        match self {
            Watcher::File(file) => file.debounce,
            Watcher::Dir(dir) => dir.debounce,
        }
    }

    #[must_use]
    pub fn commands(&self) -> Option<Vec<CommandDescription>> {
        match self {
            Watcher::File(file) => file.after_change.clone(),
            Watcher::Dir(dir) => dir.after_change.clone(),
        }
    }

    #[must_use]
    pub fn path(&self) -> String {
        match self {
            Watcher::File(file) => file.file.clone(),
            Watcher::Dir(dir) => dir.dir.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Config {
    pub watchers: Vec<Watcher>,
}

impl Config {
    /// Parses JSON string into Config.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::BadConfiguration` if JSON parsing fails.
    pub fn json(config_str: &str) -> Result<Config> {
        match serde_json::from_str(config_str) {
            Ok(config) => Result::Ok(config),
            Err(err) => Result::Err(ConfigError::BadConfiguration(err).into()),
        }
    }

    /// Serializes Config to JSON string.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError::FailedDeserialization` if serialization fails.
    pub fn as_json(&self) -> Result<String> {
        match serde_json::to_string(&self) {
            Ok(val) => Result::Ok(val),
            Err(err) => Result::Err(ConfigError::FailedDeserialization(err).into()),
        }
    }
}
