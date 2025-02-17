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
        clap::Command::new("local")
            .about("runs a local dev proxy server that builds and reloads your project")
            .arg(
                clap::Arg::new("service_addr")
                    .long("service_addr")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String))
                    .default_value("0.0.0.0"),
            )
            .arg(
                clap::Arg::new("service_port")
                    .long("service_port")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(usize))
                    .default_value("3000"),
            )
            .arg(
                clap::Arg::new("proxy_addr")
                    .long("proxy_addr")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String))
                    .default_value("0.0.0.0"),
            )
            .arg(
                clap::Arg::new("proxy_port")
                    .long("proxy_port")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(usize))
                    .default_value("3600"),
            )
            .arg(
                clap::Arg::new("project_directory")
                    .long("project_directory")
                    .required(true)
                    .action(clap::ArgAction::Set)
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
    let project_name = args
        .get_one::<String>("project_name")
        .expect("should have project_name address");

    let project_directory = args
        .get_one::<String>("project_directory")
        .expect("should have project_directory address");

    let binary_name_ref = args.get_one::<String>("binary_name");

    let binary_name = if binary_name_ref.is_none() {
        project_name.clone().to_owned()
    } else {
        binary_name_ref.unwrap().to_owned()
    };

    let service_addr = args
        .get_one::<String>("service_addr")
        .expect("should have source address");

    let service_port = args
        .get_one::<usize>("service_port")
        .expect("should have source port");

    let proxy_addr = args
        .get_one::<String>("proxy_addr")
        .expect("should have destination address");

    let proxy_port = args
        .get_one::<usize>("proxy_port")
        .expect("should have destination port");

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    ewe_trace::info!("Starting local binary");

    let destination = ProxyRemoteConfig::new(service_addr.clone(), service_port.clone());
    let source = ProxyRemoteConfig::new(proxy_addr.clone(), proxy_port.clone());

    let tunnel_config = ProxyType::Http1(Http1::new(source, destination, Some(HashMap::new())));

    let definition = ProjectDefinition {
        proxy: tunnel_config,
        crate_name: project_name.clone(),
        workspace_root: project_directory.clone(),
        watch_directories: vec![project_directory.clone()],
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
