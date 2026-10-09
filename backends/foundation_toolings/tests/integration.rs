// Integration tests for foundation_toolings.

use foundation_toolings::builder::{CargoBuilder, ProjectBuilder};
use foundation_toolings::watcher::FileChange;
use foundation_toolings::VecStringExt;
use foundation_toolings::{ProjectDefinition, ProxyRemoteConfig, ProxyType, Http1, Tunnel};
use foundation_toolings::runner::DurationSleeper;
use foundation_toolings::proxy::{TunnelProxyTrait, copy_bidirectional};

use std::path::PathBuf;
use std::time::Duration;

// -- VecStringExt

#[test]
fn test_vec_string_ext() {
    let v = vec!["hello", "world"];
    let result = v.to_vec_string();
    assert_eq!(result, vec!["hello".to_string(), "world".to_string()]);
}

// -- FileChange

#[test]
fn test_file_change_rust() {
    let path = PathBuf::from("src/main.rs");
    let change = FileChange::from(path);
    assert!(matches!(change, FileChange::Rust(_)));
}

#[test]
fn test_file_change_js() {
    let path = PathBuf::from("frontend/app.js");
    let change = FileChange::from(path);
    assert!(matches!(change, FileChange::Javascript(_)));
}

#[test]
fn test_file_change_ts() {
    let path = PathBuf::from("src/index.ts");
    let change = FileChange::from(path);
    assert!(matches!(change, FileChange::Typescript(_)));
}

#[test]
fn test_file_change_any() {
    let path = PathBuf::from("data.json");
    let change = FileChange::from(path);
    assert!(matches!(change, FileChange::Any(_)));
}

// -- CargoBuilder should_build

#[test]
fn test_cargo_builder_should_build_rust() {
    let builder = CargoBuilder {
        workspace_root: ".".into(),
        crate_name: "test".into(),
        build_args: vec![],
        skip_check: false,
    };
    assert!(builder.should_build(&FileChange::Rust(PathBuf::from("src/main.rs"))));
}

#[test]
fn test_cargo_builder_should_not_build_js() {
    let builder = CargoBuilder {
        workspace_root: ".".into(),
        crate_name: "test".into(),
        build_args: vec![],
        skip_check: false,
    };
    assert!(!builder.should_build(&FileChange::Javascript(PathBuf::from("app.js"))));
}

// -- ProjectDefinition + ProxyType

#[test]
fn test_project_definition() {
    let dest = ProxyRemoteConfig::new("0.0.0.0".into(), 3000);
    let source = ProxyRemoteConfig::new("0.0.0.0".into(), 3600);
    let proxy = ProxyType::Http1(Http1::new(source, dest));

    let def = ProjectDefinition {
        proxy,
        target_directory: "./target".into(),
        workspace_root: ".".into(),
        crate_name: "myapp".into(),
        skip_rust_checks: true,
        stop_on_failure: false,
        reload_directories: vec!["./src".into()],
        build_directories: vec!["./src".into()],
        build_arguments: vec!["cargo".into(), "build".into()],
        run_arguments: vec!["cargo".into(), "run".into()],
        wait_before_reload: Duration::from_millis(300),
    };

    assert!(def.skip_rust_checks);
    assert_eq!(def.crate_name, "myapp");
}

#[test]
fn test_tunnel_type() {
    let source = ProxyRemoteConfig::new("0.0.0.0".into(), 8080);
    let dest = ProxyRemoteConfig::new("0.0.0.0".into(), 3000);
    let tunnel = Tunnel::new(source.clone(), dest.clone());

    assert_eq!(tunnel.source.addr, "0.0.0.0");
    assert_eq!(tunnel.source.port, 8080);
}

// -- DurationSleeper

#[test]
fn test_duration_sleeper() {
    let mut sleeper = DurationSleeper::new();
    assert!(!sleeper.is_done());

    sleeper.start(Duration::from_millis(50));
    assert!(!sleeper.is_done());

    std::thread::sleep(Duration::from_millis(100));
    assert!(sleeper.is_done());
}

// -- copy_bidirectional (quick smoke)

#[test]
fn test_copy_bidirectional() {
    use std::io::Cursor;

    let mut a = Cursor::new(vec![1, 2, 3, 4]);
    let mut b = Cursor::new(Vec::new());

    copy_bidirectional(&mut a, &mut b);
    // After copy, b should have received a's data (one direction at least)
    assert!(!b.get_ref().is_empty() || a.position() > 0);
}
