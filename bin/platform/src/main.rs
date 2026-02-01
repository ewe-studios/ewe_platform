mod generate;
mod local;
mod sandbox;
mod sandbox_app;
mod watchful;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() -> std::result::Result<(), BoxedError> {
    let commander = sandbox::register(sandbox_app::register(watchful::register(local::register(
        generate::register(
            clap::Command::new("platform")
                .about("The Ewe platform toolset")
                .arg_required_else_help(true)
                .allow_external_subcommands(true),
        ),
    ))));

    let matches = commander.get_matches();
    match matches.subcommand() {
        Some(("local", arguments)) => local::run(arguments).await?,
        Some(("sandbox", arguments)) => sandbox::run(arguments).await?,
        Some(("sandbox_app", arguments)) => sandbox_app::run(arguments).await?,
        Some(("generate", arguments)) => generate::run(arguments)?,
        Some(("watch", arguments)) => watchful::run(arguments)?,
        _ => {}
    }

    Ok(())
}
