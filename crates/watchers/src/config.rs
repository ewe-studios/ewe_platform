use thiserror::Error;

use serde::{Deserialize, Serialize};

use serde_with::skip_serializing_none;

pub type Result<T> = anyhow::Result<T, ConfigError>;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file is not found")]
    FileNotFound,

    #[error("Configuration file could not be read")]
    FailedReading,

    #[error("Configuration file format not supported")]
    UnknownFormat,

    #[error("Configuration is invalid due to: {0}")]
    BadConfiguration(serde_json::Error),

    #[error("Could not serialize configuration due to: {0}")]
    FailedDeserialization(serde_json::Error),
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
            Err(err) => Result::Err(ConfigError::BadConfiguration(err)),
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
            Err(err) => Result::Err(ConfigError::FailedDeserialization(err)),
        }
    }
}
