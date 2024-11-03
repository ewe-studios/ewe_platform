use core::time;
use ewe_devserver::{
    types::{Http1, ProxyRemoteConfig},
    HttpDevService, ProjectDefinition, ProxyType, VecStringExt,
};
use std::collections::HashMap;
use tokio::sync::broadcast;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ewe_trace::info!("Starting local binary");

    let binary_name = std::env::var("BINARY_NAME")
        .expect("should fetch BINARY_NAME from environment via .cargo/config.toml");

    let project_directory = std::env::var("PROJECT_DIRECTORY")
        .expect("should fetch PROJECT_DIRECTORY from environment via .cargo/config.toml");

    let source = ProxyRemoteConfig::new(String::from("0.0.0.0"), 3600);
    let destination = ProxyRemoteConfig::new(String::from("0.0.0.0"), 3200);

    let tunnel_config = ProxyType::Http1(Http1::new(source, destination, Some(HashMap::new())));

    let definition = ProjectDefinition {
        proxy: tunnel_config,
        crate_name: binary_name.clone(),
        workspace_root: project_directory.clone(),
        watch_directory: project_directory.clone(),
        wait_before_reload: time::Duration::from_millis(300), // magic number that works
        run_arguments: vec!["cargo", "run", "--bin", &binary_name].to_vec_string(),
        build_arguments: vec!["cargo", "build", "--bin", &binary_name].to_vec_string(),
        target_directory: String::from(format!("{}/target", project_directory.clone())),
    };

    let mut dev_service = HttpDevService::new(definition);

    let (_cancel_sender, cancel_receiver) = broadcast::channel::<()>(1);

    let waiter = dev_service
        .start(cancel_receiver)
        .await
        .expect("safely instantiated");

    waiter
        .await
        .expect("safely closed")
        .expect("should safely be cleanedup");
}
