#![allow(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::needless_continue)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::type_complexity)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::unnecessary_wraps)]

mod gen_model_descriptors;
mod gen_provider_specs;
mod gen_resource_types;
mod generate;
mod local;
mod sandbox;
mod sandbox_app;
mod wasm_bins;
mod watchful;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[tokio::main]
async fn main() -> std::result::Result<(), BoxedError> {
    let commander = wasm_bins::register(
        gen_model_descriptors::register(
            gen_resource_types::register(
                sandbox::register(
                    sandbox_app::register(
                        watchful::register(
                            local::register(
                                gen_provider_specs::register(
                                    clap::Command::new("platform")
                                        .about("The Ewe platform toolset")
                                        .arg_required_else_help(true)
                                        .allow_external_subcommands(true),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    );

    let matches = commander.get_matches();
    match matches.subcommand() {
        Some(("local", arguments)) => local::run(arguments).await?,
        Some(("sandbox", arguments)) => sandbox::run(arguments).await?,
        Some(("sandbox_app", arguments)) => sandbox_app::run(arguments).await?,
        Some(("generate", arguments)) => generate::run(arguments)?,
        Some(("gen_model_descriptors", arguments)) => gen_model_descriptors::run(arguments)?,
        Some(("gen_provider_specs", arguments)) => gen_provider_specs::run(arguments)?,
        Some(("gen_resource_types", arguments)) => gen_resource_types::run(arguments)?,
        Some(("wasm_bins", arguments)) => wasm_bins::run(arguments)?,
        Some(("watch", arguments)) => watchful::run(arguments)?,
        _ => {}
    }

    Ok(())
}
