use tracing::Level;
use tracing_subscriber::FmtSubscriber;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
        clap::Command::new("watch")
            .about("runs a watcher for re-running configured commands in your watcher.json file")
            .arg(
                clap::Arg::new("debounce")
                    .short('d')
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(u64))
                    .default_value("800"),
            )
            .arg(
                clap::Arg::new("config")
                    .short('c')
                    .long("config_file")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(std::path::PathBuf))
                    .default_value("./watcher.json"),
            )
            .arg_required_else_help(true),
    )
}

pub fn run(args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    let config_file = args
        .get_one::<std::path::PathBuf>("config")
        .expect("should have config file");

    let debounce_ms = args
        .get_one::<u64>("debounce")
        .expect("should have debounce value")
        .to_owned();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let current_directory = std::env::current_dir().unwrap();
    let target_config = current_directory.join(config_file);

    let watcher = ewe_watchers::watcher::Watchers::new();
    let config_watcher = ewe_watchers::watcher::ConfigWatcher::new(
        target_config.as_path().into(),
        watcher,
        debounce_ms,
    );

    config_watcher
        .listen()
        .expect("should have started watcher and config watching");

    Ok(())
}
