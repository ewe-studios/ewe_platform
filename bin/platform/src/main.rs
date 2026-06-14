#![allow(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::type_complexity)]

mod gen_api;
mod generate;
mod local;
mod models;
// mod sandbox;
// mod sandbox_app;
mod schema;
mod tcp_capture;
mod testbed;
mod wasm_bins;
mod wasm_bundle;
// mod watchful; // temporarily disabled (ewe_watchers excluded from workspace)

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() -> std::result::Result<(), BoxedError> {
    let _guard = foundation_core::valtron::initialize_pool(100, None);

    let mut commander = clap::Command::new("platform")
        .about("The Ewe platform toolset")
        .arg_required_else_help(true)
        .allow_external_subcommands(true);

    commander = models::register(commander);
    commander = generate::register(commander);
    commander = tcp_capture::register(commander);
    commander = local::register(commander);
    // commander = watchful::register(commander); // disabled with ewe_watchers
    // commander = sandbox_app::register(commander);
    // commander = sandbox::register(commander);
    commander = gen_api::register(commander);
    commander = schema::register(commander);
    commander = wasm_bins::register(commander);
    commander = wasm_bundle::register(commander);
    commander = testbed::register(commander);

    let matches = commander.get_matches();
    match matches.subcommand() {
        Some(("local", arguments)) => local::run(arguments)?,
        // Some(("sandbox", arguments)) => sandbox::run(arguments).await?,
        // Some(("sandbox_app", arguments)) => sandbox_app::run(arguments).await?,
        Some(("generate", arguments)) => generate::run(arguments)?,
        Some(("gen_api", arguments)) => gen_api::run(arguments)?,
        Some(("schema", arguments)) => schema::run(arguments)?,
        Some(("wasm_bins", arguments)) => wasm_bins::run(arguments)?,
        Some(("wasm-bundle", arguments)) => wasm_bundle::run(arguments)?,
        // Some(("watch", arguments)) => watchful::run(arguments)?,
        Some(("tcp_capture", arguments)) => tcp_capture::run(arguments)?,
        Some(("gen_model_descriptors", arguments)) => models::run(arguments)?,
        Some(("testbed", arguments)) => testbed::run(arguments)?,
        _ => {}
    }

    Ok(())
}
