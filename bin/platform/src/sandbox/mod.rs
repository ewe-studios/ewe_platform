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
                clap::Arg::new("source_addr")
                    .long("source_addr")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String))
                    .help("The address we will run the sandbox proxy on")
                    .default_value("0.0.0.0"),
            )
            .arg(
                clap::Arg::new("source_port")
                    .long("source_port")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(usize))
                    .help("The port we will run the sandbox proxy on")
                    .default_value("3000"),
            )
            .arg(
                clap::Arg::new("destination_addr")
                    .long("destination_addr")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String))
                    .help("The sandbox http server address the actual sandbox http server is running on")
                    .default_value("0.0.0.0"),
            )
            .arg(
                clap::Arg::new("destination_port")
                    .long("destination_port")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(usize))
                    .help("The sandbox http server port the actual sandbox http server is running on")
                    .default_value("3050"),
            )
            .arg(
                clap::Arg::new("project_directory")
                    .long("project_directory")
                    .required(true)
                    .action(clap::ArgAction::Set)
                    .help("The directory where the ")
                    .value_parser(clap::value_parser!(String)),
            )
            .arg(
                clap::Arg::new("project_name")
                    .long("project_name")
                    .required(true)
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String)),
            )
            .arg(
                clap::Arg::new("binary_name")
                    .long("binary_name")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String)),
            )
            .arg_required_else_help(true),
    )
}

pub async fn run(args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    let current_directory = std::env::current_dir()?;

    let project_name: String = "sandbox_app".into();

    let project_directory = args
        .get_one::<String>("project_directory")
        .expect("should have project_directory address");

    let binary_name_ref = args.get_one::<String>("binary_name");

    let binary_name = if binary_name_ref.is_none() {
        project_name.clone().to_owned()
    } else {
        binary_name_ref.unwrap().to_owned()
    };

    let source_addr = args
        .get_one::<String>("source_addr")
        .expect("should have source address");

    let source_port = args
        .get_one::<usize>("source_port")
        .expect("should have source port");

    let destination_addr = args
        .get_one::<String>("destination_addr")
        .expect("should have destination address");

    let destination_port = args
        .get_one::<usize>("destination_port")
        .expect("should have destination port");

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ewe_trace::info!("Starting local binary");

    let destination = ProxyRemoteConfig::new(source_addr.clone(), source_port.clone());
    let source = ProxyRemoteConfig::new(destination_addr.clone(), destination_port.clone());

    let tunnel_config = ProxyType::Http1(Http1::new(source, destination, Some(HashMap::new())));

    let definition = ProjectDefinition {
        proxy: tunnel_config,
        crate_name: project_name.clone(),
        workspace_root: project_directory.clone(),
        watch_directory: project_directory.clone(),
        wait_before_reload: time::Duration::from_millis(300), // magic number that works
        target_directory: String::from(format!("{}/target", project_directory.clone())),
        run_arguments: vec!["cargo", "run", "--bin", binary_name.as_str()].to_vec_string(),
        build_arguments: vec!["cargo", "build", "--bin", binary_name.as_str()].to_vec_string(),
    };

    let mut dev_service = HttpDevService::new(definition);

    let (_cancel_sender, cancel_receiver) = broadcast::channel::<()>(1);

    // TODO: implement signal handling

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
