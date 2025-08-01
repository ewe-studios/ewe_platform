use core::time;
use ewe_devserver::{
    types::{Http1, ProxyRemoteConfig},
    HttpDevService, ProjectDefinition, ProxyType, VecStringExt,
};
use std::collections::HashMap;
use tokio::sync::broadcast;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
        clap::Command::new("sandbox")
            .about("runs a build and proxy server for the sandbox application providing hot-reloading via SSE scripts")
            .arg(
                clap::Arg::new("service_addr")
                    .long("service_addr")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String))
                    .help("The address we will run the sandbox proxy on")
                    .default_value("0.0.0.0"),
            )
            .arg(
                clap::Arg::new("service_port")
                    .long("service_port")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(usize))
                    .help("The port we will run the sandbox proxy on")
                    .default_value("3000"),
            )
            .arg(
                clap::Arg::new("proxy_addr")
                    .long("proxy_addr")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String))
                    .help("The sandbox http server address the actual sandbox http server is running on")
                    .default_value("0.0.0.0"),
            )
            .arg(
                clap::Arg::new("proxy_port")
                    .long("proxy_port")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(usize))
                    .help("The sandbox http server port the actual sandbox http server is running on")
                    .default_value("3080"),
            )
            .arg(
                clap::Arg::new("skip_rust_checks")
                    .long("skip_rust_checks")
                    .value_parser(clap::value_parser!(bool))
                    .default_value("true")
                    .default_missing_value("false")
                    .help("When enabled will skip executing cargo check to improve rebuilding speed (default: true)")
            )
            .arg(
                clap::Arg::new("stop_on_failure")
                    .long("stop_on_failure")
                    .value_parser(clap::value_parser!(bool))
                    .default_value("false")
                    .default_missing_value("true")
                    .help("When enabled kill the rebuilding server when there is an error (default: false)")
            )
    )
}

pub async fn run(args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    let project_directory = std::env::var("EWE_PLATFORM_DIR")?;
    let assets_directory = format!("{}/assets", project_directory.clone());
    let backends_directory = format!("{}/backends", project_directory.clone());
    let crates_directory = format!("{}/crates", project_directory.clone());
    let demos_directory = format!("{}/demos", project_directory.clone());
    let binary_directory = format!("{}/bin", project_directory.clone());
    let templates_directory = format!("{}/templates", project_directory.clone());
    let examples_directory = format!("{}/examples", project_directory.clone());

    let project_name: String = "ewe_platform".into();
    let binary_name: String = "ewe_platform".into();
    let sub_command: String = "sandbox_app".into();

    let service_addr = args
        .get_one::<String>("service_addr")
        .expect("should have service address");

    let service_port = args
        .get_one::<usize>("service_port")
        .expect("should have service port");

    let proxy_addr = args
        .get_one::<String>("proxy_addr")
        .expect("should have destination address");

    let proxy_port = args
        .get_one::<usize>("proxy_port")
        .expect("should have destination port");

    let skip_rust_checks = args
        .get_one::<bool>("skip_rust_checks")
        .expect("should have skip_rust_checks set");

    let stop_on_failure = args
        .get_one::<bool>("stop_on_failure")
        .expect("should have stop_on_failure set");

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ewe_trace::info!("Starting local binary skip_rust_checks={skip_rust_checks}, stop_on_failure={stop_on_failure}");

    let source = ProxyRemoteConfig::new(service_addr.clone(), *service_port);
    let destination = ProxyRemoteConfig::new(proxy_addr.clone(), *proxy_port);

    let tunnel_config = ProxyType::Http1(Http1::new(source, destination, Some(HashMap::new())));

    let definition = ProjectDefinition {
        skip_rust_checks: *skip_rust_checks,
        stop_on_failure: *stop_on_failure,
        proxy: tunnel_config,
        crate_name: project_name.clone(),
        workspace_root: project_directory.clone(),
        reload_directories: vec![
            assets_directory,
            backends_directory.clone(),
            demos_directory.clone(),
            binary_directory.clone(),
            crates_directory.clone(),
            templates_directory.clone(),
            examples_directory.clone(),
        ],
        build_directories: vec![
            backends_directory.clone(),
            demos_directory.clone(),
            binary_directory.clone(),
            crates_directory.clone(),
            templates_directory.clone(),
            examples_directory.clone(),
        ],
        wait_before_reload: time::Duration::from_millis(300), // magic number that works
        target_directory: format!("{}/target", project_directory.clone()),
        build_arguments: vec!["cargo", "build", "--bin", binary_name.as_str()].to_vec_string(),
        run_arguments: vec![
            "cargo",
            "run",
            "--bin",
            binary_name.as_str(),
            sub_command.as_str(),
        ]
        .to_vec_string(),
    };

    let mut dev_service = HttpDevService::new(definition);

    let (cancel_sender, cancel_receiver) = broadcast::channel::<()>(1);

    ctrlc::set_handler(move || {
        cancel_sender.send(()).expect("should send signal");
    })
    .expect("Error setting Ctrl-C handler");

    let waiter = dev_service
        .start(cancel_receiver)
        .await
        .expect("safely instantiated");

    waiter
        .await
        .expect("safely closed")
        .expect("should safely be cleanedup");

    Ok(())
}
