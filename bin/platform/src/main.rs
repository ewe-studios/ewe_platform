#![allow(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::type_complexity)]

use foundation_codegentools::cli::gen_api;
mod generate;
mod local;
mod models;
mod schema;
mod tcp_capture;
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
    commander = gen_api::register(commander);
    commander = schema::register(commander);
    commander = wasm_bins::register(commander);
    commander = wasm_bundle::register(commander);

    let matches = commander.get_matches();
    match matches.subcommand() {
        Some(("local", arguments)) => local::run(arguments)?,
        Some(("generate", arguments)) => generate::run(arguments)?,
        Some(("gen_api", arguments)) => gen_api::run(arguments)?,
        Some(("schema", arguments)) => schema::run(arguments)?,
        Some(("wasm_bins", arguments)) => wasm_bins::run(arguments)?,
        Some(("wasm-bundle", arguments)) => wasm_bundle::run(arguments)?,
        // Some(("watch", arguments)) => watchful::run(arguments)?,
        Some(("tcp_capture", arguments)) => tcp_capture::run(arguments)?,
        Some(("gen_model_descriptors", arguments)) => models::run(arguments)?,
        _ => {}
    }

    Ok(())
}
