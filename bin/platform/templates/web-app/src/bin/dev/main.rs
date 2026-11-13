use core::time;

use foundation_config::value_from_path;
use foundation_core::extensions::serde_ext::DynamicValueExt;
use foundation_toolings::{Http1, ProjectDefinition, ProxyRemoteConfig, ProxyType, VecStringExt};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    tracing::info!("Starting local binary");

    let operating_environment_name = std::env::var("ENVIRONMENT")
        .expect("should fetch ENVIRONMENT from environment via .cargo/config.toml");

    let configuration_file_name = format!("{}.toml", operating_environment_name);

    let binary_name = std::env::var("BINARY_NAME")
        .expect("should fetch BINARY_NAME from environment via .cargo/config.toml");

    let project_directory = std::env::var("PROJECT_DIRECTORY")
        .expect("should fetch PROJECT_DIRECTORY from environment via .cargo/config.toml");

    let config_directory = std::env::var("CONFIG_DIR")
        .expect("should fetch CONFIG_DIR from environment via .cargo/config.toml");

    let config_value = value_from_path(format!("{config_directory}/{configuration_file_name}"))
        .expect("should load config");

    let dev_address = config_value
        .d_get::<String>("/dev/addr")
        .expect("should have dev.addr");
    let dev_port = config_value
        .d_get::<usize>("/dev/port")
        .expect("should have dev.port");

    let app_address = config_value
        .d_get::<String>("/app/addr")
        .expect("should have app.addr");
    let app_port = config_value
        .d_get::<usize>("/app/port")
        .expect("should have app.port");

    let source = ProxyRemoteConfig::new(dev_address, dev_port);
    let destination = ProxyRemoteConfig::new(app_address, app_port);

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
