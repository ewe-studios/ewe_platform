//! Config parsing tests moved out of `src/config.rs` (spec 70 feature 59,
//! rule 7). Every item the tests use is public.

use ewe_watchers::config::{
    CommandDescription, CommandExpectation, Config, ConfigError, DirWatcher, FileWatcher, Watcher,
};

#[test]
fn test_watcher_config_parsing_should_fail_if_configuration_is_invalid() {
    let candidates = [
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

    let results: Vec<ewe_watchers::config::Result<Config>> = candidates
        .iter()
        .map(move |config| Config::json(config))
        .collect();

    for r in results {
        assert!(r.is_err());
        assert!(!matches!(
            r,
            Err(trace) if matches!(trace.current_context(), ConfigError::FailedDeserialization(_))
        ));
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
fn test_config_with_dir_watcher_with_react_commands() {
    let config_json = r#"
		{
            "watchers": [
                {
					"dir": "./crates/watchers/src",
                    "recursive": true,
                    "debounce": 800,
                    "after_change": [
                        {
                            "command": ["rust", "build"],
                            "if_failed": "exit"
                        },
                        {
                            "command": ["rust", "check"],
                            "if_failed": "exit"
                        },
                        {
                            "command": ["rust", "test"],
                            "if_failed": "exit"
                        }
                    ]
                }
            ]
        }
        "#;

    let expected = Config {
        watchers: vec![Watcher::Dir(DirWatcher {
            dir: String::from("./crates/watchers/src"),
            recursive: true,
            debounce: 800,
            after_change: Some(vec![
                CommandDescription {
                    command: vec![String::from("rust"), String::from("build")],
                    if_failed: Some(CommandExpectation::Exit),
                },
                CommandDescription {
                    command: vec![String::from("rust"), String::from("check")],
                    if_failed: Some(CommandExpectation::Exit),
                },
                CommandDescription {
                    command: vec![String::from("rust"), String::from("test")],
                    if_failed: Some(CommandExpectation::Exit),
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
                            "if_failed": "exit"
                        },
                        {
                            "command": ["rust", "check"],
                            "if_failed": "exit"
                        },
                        {
                            "command": ["rust", "test"],
                            "if_failed": "exit"
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
                },
                CommandDescription {
                    command: vec![String::from("rust"), String::from("check")],
                    if_failed: Some(CommandExpectation::Exit),
                },
                CommandDescription {
                    command: vec![String::from("rust"), String::from("test")],
                    if_failed: Some(CommandExpectation::Exit),
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
