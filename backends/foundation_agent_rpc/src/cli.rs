//! The `agent-rpc` command line: the `keys` subcommands.
//!
//! WHY: the operator seals the clear-text key files of the first version and
//! rotates keys before or after a host start. Both actions need a process
//! entry point, not only a library call.
//!
//! WHAT: [`Command`], [`KeysArgs`], [`parse`] and [`run`].
//!
//! HOW: the parser takes the arguments after the program name. The binary
//! `agent-rpc` builds a host [`Ctx`] and blocks on [`run`].

use std::path::PathBuf;

use foundation_context::Ctx;

use crate::keys::{self, KekConfig, KeysCommand, KeysReport};

/// A parsed `agent-rpc` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `agent-rpc keys ...`.
    Keys(KeysArgs),
}

/// The arguments of `agent-rpc keys ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeysArgs {
    /// The subcommand.
    pub command: KeysCommand,
    /// The state directory that holds the key records.
    pub state_dir: PathBuf,
    /// A key encryption key (KEK) in this file. `None`: the default source
    /// order of the native host.
    pub kek_file: Option<PathBuf>,
    /// The KEKs that can still open old blobs. `rotate-kek` only.
    pub previous_kek_files: Vec<PathBuf>,
}

/// Why the arguments do not form a command.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
#[display("{_0}")]
pub struct CliError(#[error(not(source))] String);

impl CliError {
    fn usage() -> Self {
        Self(
            "usage: agent-rpc keys <migrate|rotate|rotate-kek> --state-dir <dir> \
             [--kek-file <file>] [--previous-kek-file <file>]..."
                .to_string(),
        )
    }
}

/// Parse the arguments after the program name.
///
/// # Errors
/// [`CliError`] for an unknown command or a missing value.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command, CliError> {
    let mut args = args.into_iter();
    let first = args.next().ok_or_else(CliError::usage)?;
    if first != "keys" {
        return Err(CliError(format!(
            "unknown command {first:?}; {}",
            CliError::usage()
        )));
    }
    let subcommand = args.next().ok_or_else(CliError::usage)?;
    let command = match subcommand.as_str() {
        "migrate" => KeysCommand::Migrate,
        "rotate" => KeysCommand::RotateKey,
        "rotate-kek" => KeysCommand::RotateKek,
        other => {
            return Err(CliError(format!(
                "unknown keys subcommand {other:?}; {}",
                CliError::usage()
            )))
        }
    };

    let mut state_dir = None;
    let mut kek_file = None;
    let mut previous_kek_files = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state-dir" => state_dir = Some(PathBuf::from(value(&mut args, "--state-dir")?)),
            "--kek-file" => kek_file = Some(PathBuf::from(value(&mut args, "--kek-file")?)),
            "--previous-kek-file" => {
                previous_kek_files.push(PathBuf::from(value(&mut args, "--previous-kek-file")?));
            }
            other => {
                return Err(CliError(format!(
                    "unknown argument {other:?}; {}",
                    CliError::usage()
                )))
            }
        }
    }
    let state_dir = state_dir
        .ok_or_else(|| CliError(format!("--state-dir is required; {}", CliError::usage())))?;
    Ok(Command::Keys(KeysArgs {
        command,
        state_dir,
        kek_file,
        previous_kek_files,
    }))
}

fn value<I: Iterator<Item = String>>(args: &mut I, flag: &str) -> Result<String, CliError> {
    args.next()
        .ok_or_else(|| CliError(format!("{flag} needs a value")))
}

/// Run a parsed command. The report is for the terminal.
///
/// # Errors
/// [`foundation_auth::keystore::KeyStoreError`] variants.
pub async fn run(
    command: &Command,
    ctx: &Ctx,
) -> Result<KeysReport, foundation_auth::keystore::KeyStoreError> {
    match command {
        Command::Keys(args) => run_keys(args, ctx).await,
    }
}

/// Run the `keys` subcommand.
///
/// # Errors
/// [`foundation_auth::keystore::KeyStoreError`] variants.
pub async fn run_keys(
    args: &KeysArgs,
    ctx: &Ctx,
) -> Result<KeysReport, foundation_auth::keystore::KeyStoreError> {
    let current = args
        .kek_file
        .clone()
        .map(|path| KekConfig::SecretFile { path });
    match args.command {
        KeysCommand::Migrate => keys::migrate(&args.state_dir, current.as_ref(), ctx).await,
        KeysCommand::RotateKey => {
            keys::rotate_signing_key(&args.state_dir, current.as_ref(), ctx).await
        }
        KeysCommand::RotateKek => {
            let previous: Vec<KekConfig> = args
                .previous_kek_files
                .iter()
                .cloned()
                .map(|path| KekConfig::SecretFile { path })
                .collect();
            keys::rotate_kek(&args.state_dir, current.as_ref(), &previous, ctx).await
        }
    }
}
