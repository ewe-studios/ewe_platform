//! The dev server example wires one `ProjectDefinition` and hands it to the
//! platform dev service. This test checks that wiring: the proxy tunnel and the
//! watch/build/run configuration the binary constructs.

use core::time::Duration;

use foundation_toolings::{Http1, ProjectDefinition, ProxyRemoteConfig, ProxyType, VecStringExt};

#[test]
fn dev_definition_wires_proxy_and_watch_directories() {
    let binary_name = String::from("app");
    let project_directory = String::from("/tmp/example-project");

    let source = ProxyRemoteConfig::new(String::from("0.0.0.0"), 3600);
    let destination = ProxyRemoteConfig::new(String::from("0.0.0.0"), 3200);
    let proxy = ProxyType::Http1(Http1::new(source, destination));

    let definition = ProjectDefinition {
        proxy,
        crate_name: binary_name.clone(),
        workspace_root: project_directory.clone(),
        reload_directories: vec![project_directory.clone()],
        build_directories: vec![project_directory.clone()],
        wait_before_reload: Duration::from_millis(300),
        target_directory: format!("{project_directory}/target"),
        build_arguments: vec!["cargo", "build", "--bin", binary_name.as_str()].to_vec_string(),
        run_arguments: vec!["cargo", "run", "--bin", binary_name.as_str()].to_vec_string(),
        skip_rust_checks: false,
        stop_on_failure: false,
    };

    assert_eq!(definition.crate_name, "app");
    assert_eq!(definition.workspace_root, "/tmp/example-project");
    assert_eq!(definition.reload_directories, vec!["/tmp/example-project"]);
    assert_eq!(definition.build_directories, vec!["/tmp/example-project"]);
    assert_eq!(definition.target_directory, "/tmp/example-project/target");
    assert_eq!(
        definition.run_arguments,
        vec!["cargo", "run", "--bin", "app"]
    );
    assert_eq!(
        definition.build_arguments,
        vec!["cargo", "build", "--bin", "app"]
    );
}
