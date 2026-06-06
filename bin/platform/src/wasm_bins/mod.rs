type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(foundation_codegentools::cli::wasm_bins::command())
}

pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    foundation_codegentools::cli::wasm_bins::run(args)?;
    Ok(())
}
