use foundation_macros::EmbedDirectoryAs;
use foundation_packager::cli::generate;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(EmbedDirectoryAs, Default)]
#[source = "$OUT_DIR/templates/"]
struct ProjectTemplates;

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(generate::command())
}

pub fn run(args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    generate::run_with_templates::<ProjectTemplates>(args)
}
