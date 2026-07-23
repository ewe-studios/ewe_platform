use std::path::PathBuf;
use std::process::Command;

use tracing::Level;
use tracing_subscriber::FmtSubscriber;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
        clap::Command::new("gen_model_descriptors")
            .about("Fetch upstream model catalogs and regenerate backends/foundation_ai/src/models/providers/")
            .arg(
                clap::Arg::new("debug")
                    .long("debug")
                    .default_value("false")
                    .action(clap::ArgAction::SetTrue)
                    .help("Enables debug logs (default: false)")
            ),
    )
}

pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    let logging_level = if args.get_flag("debug") {
        Level::DEBUG
    } else {
        Level::INFO
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(logging_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let result = crate::models::generator::generate_model_descriptors()?;

    let providers_dir = PathBuf::from("backends/foundation_ai/src/models/providers");
    std::fs::create_dir_all(&providers_dir)?;

    let mod_path = providers_dir.join("mod.rs");
    std::fs::write(&mod_path, &result.mod_source)?;

    let mut all_paths = vec![mod_path];

    for (filename, source) in &result.provider_files {
        let path = providers_dir.join(filename);
        std::fs::write(&path, source)?;
        all_paths.push(path);
    }

    for path in &all_paths {
        match Command::new("rustfmt").arg(path).status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                tracing::error!("rustfmt exited with {status} for {}", path.display());
            }
            Err(e) => {
                tracing::error!("Failed to run rustfmt on {}: {e}", path.display());
            }
        }
    }

    tracing::info!(
        "Generated {} provider files in {}",
        result.provider_files.len(),
        providers_dir.display()
    );
    tracing::info!("Total tool-capable models: {}", result.total_models);
    tracing::info!("Reasoning-capable models: {}", result.reasoning_count);
    for (provider, count) in &result.provider_counts {
        tracing::info!("  {provider}: {count} models");
    }

    Ok(())
}
