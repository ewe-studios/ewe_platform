//! Integration tests for the command system and REPL builder.
//!
//! These test the observable behavior without needing a real terminal.

use foundation_shell_repl::{CommandRegistry, CommandResult, ReplConfig};

// ── Command registry tests ──────────────────────────────────────────────

#[test]
fn builtin_help_returns_text() {
    let reg = CommandRegistry::new();
    let result = reg.try_handle("/help").unwrap();
    match result {
        CommandResult::Output(text) => {
            assert!(text.contains("/help"));
            assert!(text.contains("/clear"));
            assert!(text.contains("/exit"));
            assert!(text.contains("/version"));
        }
        _ => panic!("help should return Output"),
    }
}

#[test]
fn builtin_version_returns_version() {
    let reg = CommandRegistry::new();
    let result = reg.try_handle("/version").unwrap();
    match result {
        CommandResult::Output(text) => {
            assert!(text.contains("foundation_shell_repl"));
        }
        _ => panic!("version should return Output"),
    }
}

#[test]
fn builtin_exit_returns_exit() {
    let reg = CommandRegistry::new();
    let result = reg.try_handle("/exit").unwrap();
    assert!(matches!(result, CommandResult::Exit));
}

#[test]
fn builtin_clear_returns_terminal() {
    let reg = CommandRegistry::new();
    let result = reg.try_handle("/clear").unwrap();
    assert!(matches!(result, CommandResult::Terminal(_)));
}

#[test]
fn unknown_command_returns_error() {
    let reg = CommandRegistry::new();
    let result = reg.try_handle("/unknown").unwrap();
    match result {
        CommandResult::Output(text) => {
            assert!(text.contains("Unknown command"));
            assert!(text.contains("unknown"));
        }
        _ => panic!("unknown command should return Output"),
    }
}

#[test]
fn non_command_line_returns_none() {
    let reg = CommandRegistry::new();
    assert!(reg.try_handle("hello world").is_none());
}

#[test]
fn custom_command_works() {
    let mut reg = CommandRegistry::new();
    reg.register("greet", |cmd| {
        let name = cmd.strip_prefix("/greet ").unwrap_or("world");
        format!("Hello, {name}!")
    });

    let result = reg.try_handle("/greet Alice").unwrap();
    match result {
        CommandResult::Output(text) => {
            assert_eq!(text, "Hello, Alice!");
        }
        _ => panic!("custom command should return Output"),
    }
}

#[test]
fn command_case_insensitive() {
    let reg = CommandRegistry::new();
    let result = reg.try_handle("/HELP").unwrap();
    assert!(matches!(result, CommandResult::Output(_)));
}

#[test]
fn command_strips_args() {
    let reg = CommandRegistry::new();
    // /exit with extra text should still exit
    let result = reg.try_handle("/exit now").unwrap();
    assert!(matches!(result, CommandResult::Exit));
}

// ── Config tests ────────────────────────────────────────────────────────

#[test]
fn default_config_has_defaults() {
    let cfg = ReplConfig::default();
    assert_eq!(cfg.prompt, ">>> ");
    assert_eq!(cfg.continuation_prompt, "... ");
    assert!(cfg.banner.is_none());
    assert!(cfg.goodbye.is_none());
    assert!(cfg.max_input_length.is_some());
}

#[test]
fn max_input_length_default_is_64k() {
    let cfg = ReplConfig::default();
    assert_eq!(cfg.max_input_length, Some(64 * 1024));
}
