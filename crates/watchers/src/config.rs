use std::{error, result};

use thiserror::Error;

use serde::{Deserialize, Serialize};
use serde_json::{from_str, json, to_string};

pub type Result<T> = anyhow::Result<T, ConfigError>;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration is invalid due to: {0}")]
    BadConfiguration(serde_json::Error),

    #[error("Could not serialize configuration due to: {0}")]
    FailedDeserialization(serde_json::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DirWatcher {
    pub dir: String,
    pub recursive: bool,
    pub debounce: i16,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FileWatcher {
    pub file: String,
    pub debounce: i16,
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

    use crate::config::{ConfigError, DirWatcher};

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

        let parsed_config = Config::json(config_json).unwrap();

        let expected = Config {
            watchers: vec![Watcher::Dir(DirWatcher {
                dir: String::from("./crates/watchers/src"),
                recursive: true,
                debounce: 800,
            })],
        };

        assert_eq!(expected, parsed_config);
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

        let parsed_config = Config::json(config_json).unwrap();

        let expected = Config {
            watchers: vec![Watcher::File(FileWatcher {
                file: String::from("./crates/watchers/src/lib.rs"),
                debounce: 800,
            })],
        };

        assert_eq!(expected, parsed_config);
    }
}
