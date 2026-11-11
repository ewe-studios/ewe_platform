use core::time;

use foundation_toolings::{Http1, ProjectDefinition, ProxyRemoteConfig, ProxyType, VecStringExt};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    tracing::info!("Starting local binary");

    let binary_name = std::env::var("BINARY_NAME")
        .expect("should fetch BINARY_NAME from environment via .cargo/config.toml");

    let project_directory = std::env::var("PROJECT_DIRECTORY")
        .expect("should fetch PROJECT_DIRECTORY from environment via .cargo/config.toml");

    let source = ProxyRemoteConfig::new(String::from("0.0.0.0"), 3600);
    let destination = ProxyRemoteConfig::new(String::from("0.0.0.0"), 3200);

    let tunnel_config = ProxyType::Http1(Http1::new(source, destination));

    let definition = ProjectDefinition {
        proxy: tunnel_config,
        crate_name: binary_name.clone(),
        workspace_root: project_directory.clone(),
        reload_directories: vec![project_directory.clone()],
        build_directories: vec![project_directory.clone()],
        wait_before_reload: time::Duration::from_millis(300), // magic number that works
        run_arguments: vec!["cargo", "run", "--bin", binary_name.as_str()].to_vec_string(),
        build_arguments: vec!["cargo", "build", "--bin", binary_name.as_str()].to_vec_string(),
        target_directory: format!("{}/target", project_directory),
        skip_rust_checks: false,
        stop_on_failure: false,
    };

    foundation_toolings::run_dev_server(definition).expect("dev server ran cleanly");
}
