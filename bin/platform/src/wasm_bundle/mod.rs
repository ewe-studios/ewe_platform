type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(foundation_wasm_ui::cli::command())
}

pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    foundation_wasm_ui::cli::run(args)?;
    Ok(())
}
