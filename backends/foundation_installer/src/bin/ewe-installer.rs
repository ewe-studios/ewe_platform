//! The `ewe-installer` command-line interface.
//!
//! `apply` runs a manifest, `plan` says what `apply` would do, and
//! `render-bootstrap` prints the release bootstrap script.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use foundation_core::valtron::valtron;
use foundation_installer::{
    BootstrapInput, BootstrapScript, EntryOutcome, InstallContext, InstallError, Installer,
    InstallerConfig, Manifest, PlannedStep,
};

/// Exit code for a bad manifest or a lock error.
const EXIT_USAGE: u8 = 2;
/// Exit code for a run in which one or more entries failed.
const EXIT_ENTRY_FAILED: u8 = 1;

#[derive(Debug, Parser)]
#[command(
    name = "ewe-installer",
    about = "Install and start the entries of a spec 78 manifest"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Apply every entry of a manifest and print the report.
    Apply(ManifestArgs),
    /// Print what `apply` would do. It changes nothing.
    Plan(ManifestArgs),
    /// Print a bootstrap script for a `BootstrapInput` JSON file.
    RenderBootstrap(RenderArgs),
}

#[derive(Debug, clap::Args)]
struct ManifestArgs {
    /// The manifest file.
    #[arg(long)]
    manifest: PathBuf,
    /// Override the install record directory.
    #[arg(long)]
    state_dir: Option<PathBuf>,
    /// Override the downloaded-file directory.
    #[arg(long)]
    artifact_dir: Option<PathBuf>,
    /// Override the root of the version directories.
    #[arg(long)]
    install_root: Option<PathBuf>,
    /// Override the systemd unit directory.
    #[arg(long)]
    systemd_unit_dir: Option<PathBuf>,
}

impl ManifestArgs {
    fn config(&self) -> InstallerConfig {
        let mut config = InstallerConfig::default();
        if let Some(value) = &self.state_dir {
            config.state_dir.clone_from(value);
        }
        if let Some(value) = &self.artifact_dir {
            config.artifact_dir.clone_from(value);
        }
        if let Some(value) = &self.install_root {
            config.install_root.clone_from(value);
        }
        if let Some(value) = &self.systemd_unit_dir {
            config.systemd_unit_dir.clone_from(value);
        }
        config
    }
}

#[derive(Debug, clap::Args)]
struct RenderArgs {
    /// The `BootstrapInput` JSON file.
    #[arg(long)]
    input: PathBuf,
}

#[valtron]
async fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Apply(args) => apply(args).await,
        Command::Plan(args) => plan(args).await,
        Command::RenderBootstrap(args) => render_bootstrap(&args),
    };
    std::process::exit(i32::from(code));
}

async fn apply(args: ManifestArgs) -> u8 {
    let config = args.config();
    let manifest = match Manifest::parse_file(&args.manifest, config.max_manifest_bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("ewe-installer: {error}");
            return EXIT_USAGE;
        }
    };
    let ctx = match InstallContext::host(config) {
        Ok(ctx) => ctx,
        Err(error) => {
            eprintln!("ewe-installer: {error}");
            return EXIT_USAGE;
        }
    };
    let installer = Installer::with_defaults(ctx.config().clone());
    match installer.run(&manifest, &ctx).await {
        Ok(report) => {
            for entry in &report.entries {
                println!("{}", show_outcome(entry));
            }
            if report.has_failures() {
                EXIT_ENTRY_FAILED
            } else {
                0
            }
        }
        Err(error) => {
            eprintln!("ewe-installer: {error}");
            EXIT_USAGE
        }
    }
}

async fn plan(args: ManifestArgs) -> u8 {
    let config = args.config();
    let manifest = match Manifest::parse_file(&args.manifest, config.max_manifest_bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("ewe-installer: {error}");
            return EXIT_USAGE;
        }
    };
    let ctx = match InstallContext::host(config) {
        Ok(ctx) => ctx,
        Err(error) => {
            eprintln!("ewe-installer: {error}");
            return EXIT_USAGE;
        }
    };
    let installer = Installer::with_defaults(ctx.config().clone());
    match installer.plan(&manifest, &ctx).await {
        Ok(steps) => {
            for step in steps {
                println!("{}", show_step(&step));
            }
            0
        }
        Err(error) => {
            eprintln!("ewe-installer: {error}");
            EXIT_USAGE
        }
    }
}

fn render_bootstrap(args: &RenderArgs) -> u8 {
    let input = match read_bootstrap_input(&args.input) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("ewe-installer: {error}");
            return EXIT_USAGE;
        }
    };
    match BootstrapScript::render(&input) {
        Ok(script) => {
            print!("{script}");
            0
        }
        Err(error) => {
            eprintln!("ewe-installer: {error}");
            EXIT_USAGE
        }
    }
}

fn read_bootstrap_input(path: &std::path::Path) -> Result<BootstrapInput, InstallError> {
    let metadata = std::fs::metadata(path).map_err(|err| InstallError::ManifestIo {
        path: path.display().to_string(),
        reason: err.to_string(),
    })?;
    let limit = InstallerConfig::default().max_manifest_bytes;
    if metadata.len() > limit {
        return Err(InstallError::ManifestTooLarge {
            size: metadata.len(),
            max: limit,
        });
    }
    let text = std::fs::read_to_string(path).map_err(|err| InstallError::ManifestIo {
        path: path.display().to_string(),
        reason: err.to_string(),
    })?;
    serde_json::from_str(&text).map_err(|err| {
        InstallError::Parse(vec![foundation_installer::FieldError::new(
            "/",
            format!("the bootstrap input is not valid JSON: {err}"),
        )])
    })
}

fn show_outcome(outcome: &EntryOutcome) -> String {
    match outcome {
        EntryOutcome::Applied {
            name,
            version,
            deployed,
        } => format!("applied {name} {version} ({})", show_deployed(deployed)),
        EntryOutcome::Unchanged { name, version } => {
            format!("unchanged {name} {version}")
        }
        EntryOutcome::Failed { name, error, kept } => match kept {
            Some(kept) => format!("failed {name}: {error} (kept {kept})"),
            None => format!("failed {name}: {error}"),
        },
    }
}

fn show_deployed(deployed: &foundation_installer::Deployed) -> String {
    match deployed {
        foundation_installer::Deployed::Systemd { unit, .. } => format!("systemd unit {unit}"),
        foundation_installer::Deployed::Container { name, .. } => {
            format!("container {name}")
        }
        foundation_installer::Deployed::Installed { artifact } => {
            format!("installed {}", artifact.class())
        }
    }
}

fn show_step(step: &PlannedStep) -> String {
    match step {
        PlannedStep::Install { name, version } => format!("install {name} {version}"),
        PlannedStep::Upgrade { name, from, to } => format!("upgrade {name} {from} -> {to}"),
        PlannedStep::Unchanged { name } => format!("unchanged {name}"),
    }
}
