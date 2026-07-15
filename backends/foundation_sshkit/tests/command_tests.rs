//! Unit tests for Command type.

use foundation_sshkit::Command;

#[test]
fn test_simple_command() {
    let cmd = Command::new("ls").arg("-la");
    let shell = cmd.to_shell_command();
    assert_eq!(shell, "ls -la");
}

#[test]
fn test_command_with_env() {
    let cmd = Command::new("echo").arg("hello").env("FOO", "bar");
    let shell = cmd.to_shell_command();
    assert!(shell.contains("export FOO='bar'"));
    assert!(shell.contains("echo hello"));
}

#[test]
fn test_command_with_working_dir() {
    let cmd = Command::new("make").within("/tmp/build");
    let shell = cmd.to_shell_command();
    assert!(shell.contains("cd '/tmp/build'"));
    assert!(shell.contains("make"));
}

#[test]
fn test_command_as_user() {
    let cmd = Command::new("whoami").as_user("nobody");
    let shell = cmd.to_shell_command();
    assert!(shell.contains("sudo -u nobody"));
}

#[test]
fn test_command_in_background() {
    let cmd = Command::new("sleep").arg("60").in_background();
    let shell = cmd.to_shell_command();
    assert!(shell.contains("nohup"));
    assert!(shell.contains("> /dev/null 2>&1 &"));
}

#[test]
fn test_command_multiple_args() {
    let cmd = Command::new("docker").args(["build", "-t", "myapp:latest", "."]);
    let shell = cmd.to_shell_command();
    assert_eq!(shell, "docker build -t myapp:latest .");
}

#[test]
fn test_command_with_all_options() {
    let cmd = Command::new("cargo")
        .arg("build")
        .env("RUST_LOG", "debug")
        .within("/workspace")
        .as_user("builder")
        .pty();

    let shell = cmd.to_shell_command();
    assert!(shell.contains("export RUST_LOG='debug'"));
    assert!(shell.contains("cd '/workspace'"));
    assert!(shell.contains("sudo -u builder"));
    assert!(shell.contains("cargo build"));
    assert!(cmd.pty);
}

#[test]
fn test_command_result_is_success() {
    let result = foundation_sshkit::CommandResult {
        exit_code: 0,
        stdout: "ok".into(),
        stderr: String::new(),
        runtime: std::time::Duration::from_millis(100),
        host: "test".into(),
    };
    assert!(result.is_success());

    let result = foundation_sshkit::CommandResult {
        exit_code: 1,
        stdout: String::new(),
        stderr: "error".into(),
        runtime: std::time::Duration::from_millis(100),
        host: "test".into(),
    };
    assert!(!result.is_success());
}

#[test]
fn test_bash_wraps_in_bash_c() {
    let cmd = Command::new("echo").arg("hello");
    let wrapped = cmd.bash();
    assert!(wrapped.starts_with("bash -c"));
    assert!(wrapped.contains("echo hello"));
}

#[test]
fn test_cmd_wraps_in_cmd_c() {
    let cmd = Command::new("dir");
    let wrapped = cmd.cmd();
    assert!(wrapped.starts_with("cmd /c"));
    assert!(wrapped.contains("dir"));
}

#[test]
fn test_bash_with_env_and_cd() {
    let cmd = Command::new("make").env("CC", "clang").within("/build");
    let wrapped = cmd.bash();
    assert!(wrapped.contains("export CC='clang'"));
    assert!(wrapped.contains("cd '/build'"));
    assert!(wrapped.contains("make"));
    assert!(wrapped.starts_with("bash -c"));
}
