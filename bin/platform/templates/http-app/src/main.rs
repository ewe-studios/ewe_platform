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

    let binary_name = String::from("wello");

    let project_directory = String::from("{{PROJECT_DIRECTORY}}");
    let demo_directory = String::from(format!(
        "{}/examples/web/{}",
        &project_directory, binary_name,
    ));

    let destination = ProxyRemoteConfig::new(String::from("0.0.0.0"), 3200);
    let source = ProxyRemoteConfig::new(String::from("0.0.0.0"), 3600);

    let tunnel_config = ProxyType::Http1(Http1::new(source, destination, Some(HashMap::new())));

    let definition = ProjectDefinition {
        proxy: tunnel_config,
        watch_directory: demo_directory,
        crate_name: binary_name,
        workspace_root: project_directory.clone(),
        wait_before_reload: time::Duration::from_millis(300), // magic number that works
        target_directory: String::from(format!("{}/target", project_directory)),
        build_arguments: vec!["cargo", "build", "--bin", "wello"].to_vec_string(),
        run_arguments: vec!["cargo", "run", "--bin", "wello"].to_vec_string(),
    };

    let mut dev_service = HttpDevService::new(definition);

    let (_cancel_sender, cancel_receiver) = broadcast::channel::<()>(1);

    // TODO: implement signal interrupt listener to send signal with _cancel_sender

    let waiter = dev_service
        .start(cancel_receiver)
        .await
        .expect("safely instantiated");

    waiter
        .await
        .expect("safely closed")
        .expect("should safely be cleanedup");
}
