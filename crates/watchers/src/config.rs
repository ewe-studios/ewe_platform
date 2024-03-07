use std::{error, result};

use thiserror::Error;

use serde::{Deserialize, Serialize};
use serde_json::{from_str, json, to_string};
use serde_with::skip_serializing_none;

pub type Result<T> = anyhow::Result<T, ConfigError>;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration is invalid due to: {0}")]
    BadConfiguration(serde_json::Error),

    #[error("Could not serialize configuration due to: {0}")]
    FailedDeserialization(serde_json::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum CommandAllocation {
    #[serde(rename = "sequential")]
    Sequential,

    #[serde(rename = "concurrent")]
    Concurrent,
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
    pub alloc: Option<CommandAllocation>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DirWatcher {
    pub dir: String,
    pub recursive: bool,
    pub debounce: i16,
    pub after_change: Option<Vec<CommandDescription>>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FileWatcher {
    pub file: String,
    pub debounce: i16,
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Config {
    pub watchers: Vec<Watcher>,
}

impl Config {
    pub fn json(config_str: &str) -> Result<Config> {
        match serde_json::from_str(config_str) {
            Ok(config) => Result::Ok(config),
            Err(err) => Result::Err(ConfigError::BadConfiguration(err)),
        }
    }

    pub fn as_json(&self) -> Result<String> {
        match serde_json::to_string(&self) {
            Ok(val) => Result::Ok(val),
            Err(err) => Result::Err(ConfigError::FailedDeserialization(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{from_str, to_string};

    use crate::config::{
        CommandAllocation, CommandDescription, CommandExpectation, ConfigError, DirWatcher,
    };

    use super::{Config, FileWatcher, Watcher};

    #[test]
    fn test_watcher_config_parsing_should_fail_if_configuration_is_invalid() {
        let candidates = vec![
            r#"
		{
            "watchers": [
                {
					"dir": "./crates/watchers/src",
                    "recursive": true,
                }
            ]
        }
        "#,
            r#"
		{
            "watchers": [
                {
					"file": "./crates/watchers/src",
                }
            ]
        }
        "#,
            r#"
		{
            "watchers": [
                {
					"debounce": 800,
                }
            ]
        }
        "#,
            r#"
		{
            "watchers": [
                {
					"debounce": "./crates/watchers/src",
                }
            ]
        }
        "#,
            r#"
		{
            "watchers": [
                {
					"file_name": "./crates/watchers/src",
                }
            ]
        }
        "#,
        ];

        let results: Vec<Result<Config, ConfigError>> = candidates
            .iter()
            .map(move |config| Config::json(config))
            .collect();

        for r in results {
            assert!(r.is_err());
            assert!(!matches!(r, Err(ConfigError::FailedDeserialization(_))));
        }
    }

    #[test]
    fn test_config_with_dir_watcher() {
        let config_json = r#"
		{
            "watchers": [
                {
					"dir": "./crates/watchers/src",
                    "recursive": true,
                    "debounce": 800
                }
            ]
        }
        "#;

        let expected = Config {
            watchers: vec![Watcher::Dir(DirWatcher {
                dir: String::from("./crates/watchers/src"),
                recursive: true,
                debounce: 800,
                after_change: None,
            })],
        };

        let parsed_config = Config::json(config_json).unwrap();
        assert_eq!(
            expected,
            parsed_config,
            "We expected json like: {}",
            serde_json::to_string(&expected).unwrap()
        );
    }

    #[test]
    fn test_config_with_file_watcher() {
        let config_json = r#"
		{
            "watchers": [
                {
					"file": "./crates/watchers/src/lib.rs",
                    "debounce": 800
                }
            ]
        }
        "#;

        let expected = Config {
            watchers: vec![Watcher::File(FileWatcher {
                file: String::from("./crates/watchers/src/lib.rs"),
                debounce: 800,
                after_change: None,
            })],
        };

        let parsed_config = Config::json(config_json).unwrap();
        assert_eq!(
            expected,
            parsed_config,
            "We expected json like: {}",
            serde_json::to_string(&expected).unwrap()
        );
    }

    #[test]
    fn test_config_with_file_watcher_with_react_commands() {
        let config_json = r#"
		{
            "watchers": [
                {
					"file": "./crates/watchers/src/lib.rs",
                    "debounce": 800,
                    "after_change": [
                        {
                            "command": ["rust", "build"],
                            "if_failed": "exit",
                            "alloc": "sequential"
                        },
                        {
                            "command": ["rust", "check"],
                            "if_failed": "exit",
                            "alloc": "sequential"
                        },
                        {
                            "command": ["rust", "test"],
                            "if_failed": "exit",
                            "alloc": "sequential"
                        }
                    ]
                }
            ]
        }
        "#;

        let expected = Config {
            watchers: vec![Watcher::File(FileWatcher {
                file: String::from("./crates/watchers/src/lib.rs"),
                debounce: 800,
                after_change: Some(vec![
                    CommandDescription {
                        command: vec![String::from("rust"), String::from("build")],
                        if_failed: Some(CommandExpectation::Exit),
                        alloc: Some(CommandAllocation::Sequential),
                    },
                    CommandDescription {
                        command: vec![String::from("rust"), String::from("check")],
                        if_failed: Some(CommandExpectation::Exit),
                        alloc: Some(CommandAllocation::Sequential),
                    },
                    CommandDescription {
                        command: vec![String::from("rust"), String::from("test")],
                        if_failed: Some(CommandExpectation::Exit),
                        alloc: Some(CommandAllocation::Sequential),
                    },
                ]),
            })],
        };

        let parsed_config = Config::json(config_json).unwrap();
        assert_eq!(
            expected,
            parsed_config,
            "We expected json like: {}",
            serde_json::to_string(&expected).unwrap()
        );
    }
}
