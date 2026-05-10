//! Testbed subcommand — delegates to foundation_testbed library CLI.

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(foundation_testbed::cli::command())
}

pub fn run(args: &clap::ArgMatches) -> Result<(), BoxedError> {
    // Args are already the testbed subcommand matches, pass directly
    foundation_testbed::cli::run(args)?;
    Ok(())
}
