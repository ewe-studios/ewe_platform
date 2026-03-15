mod gen_model_descriptors;
mod generate;
mod local;
mod sandbox;
mod sandbox_app;
mod wasm_bins;
mod watchful;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() -> std::result::Result<(), BoxedError> {
    let commander = wasm_bins::register(gen_model_descriptors::register(sandbox::register(
        sandbox_app::register(watchful::register(local::register(generate::register(
            clap::Command::new("platform")
                .about("The Ewe platform toolset")
                .arg_required_else_help(true)
                .allow_external_subcommands(true),
        )))),
    )));

    let matches = commander.get_matches();
    match matches.subcommand() {
        Some(("local", arguments)) => local::run(arguments).await?,
        Some(("sandbox", arguments)) => sandbox::run(arguments).await?,
        Some(("sandbox_app", arguments)) => sandbox_app::run(arguments).await?,
        Some(("generate", arguments)) => generate::run(arguments)?,
        Some(("gen_model_descriptors", arguments)) => gen_model_descriptors::run(arguments)?,
        Some(("wasm_bins", arguments)) => wasm_bins::run(arguments)?,
        Some(("watch", arguments)) => watchful::run(arguments)?,
        _ => {}
    }

    Ok(())
}
