//! Generate and operate an Ewe application.
mod operations;
mod wizard;
use clap::{Args, ColorChoice, Command};
use foundation_tasks::{task, TaskContext, TaskError, TaskResult, TaskRunner, TaskSet};
use foundation_ui_project::application::{Application, Host, Target};
use std::{io::IsTerminal, path::PathBuf};

const ABOUT: &str =
    "Create an Ewe application. Try: ewe_ui new --name FieldNotes --target web --output .";
#[derive(Args)]
struct New {
    /// Application name. Start with a letter and use letters, digits, or underscores.
    #[arg(long)]
    name: Option<String>,
    /// Select web, android, or ios. Repeat for a combined project.
    #[arg(long, value_enum)]
    target: Vec<Target>,
    /// Mobile application identifier, such as com.example.fieldnotes.
    #[arg(long)]
    app_id: Option<String>,
    /// Use a server connection or the on-device Rust core. The default is server.
    #[arg(long, value_enum)]
    host: Option<Host>,
    /// Parent directory for the application.
    #[arg(long, default_value = ".")]
    output: PathBuf,
}
#[derive(Args)]
struct Help {
    /// Print the complete plain-text command and task guide.
    #[arg(long)]
    llm: bool,
}
async fn new(_: TaskContext, args: New) -> TaskResult {
    let parent = args.output.clone();
    blocking::unblock(move || {
        let mobile = args.target.iter().any(|t| *t != Target::Web);
        let complete =
            args.name.is_some() && !args.target.is_empty() && (!mobile || args.app_id.is_some());
        let plan = if !complete && std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
        {
            foundation_repl::run_form(
                &mut wizard::Wizard::new(args),
                &foundation_repl::ReplTheme::default(),
            )
            .map_err(TaskError::operation)?
            .ok_or(TaskError::Cancelled)?
        } else {
            let name = args
                .name
                .ok_or_else(|| TaskError::operation("Missing --name"))?;
            if !mobile && args.host.is_some() {
                return Err(TaskError::operation("--host requires a mobile target"));
            }
            Application::new(
                name,
                args.target.into_iter().collect(),
                args.app_id,
                args.host.unwrap_or_default(),
            )
            .map_err(TaskError::operation)?
        };
        let path = plan.generate(&parent).map_err(TaskError::operation)?;
        println!("Created {}", path.display());
        Ok(())
    })
    .await
}
struct Guide(Command);
fn inventory(mut command: Command, path: &str) {
    command = command.color(ColorChoice::Never).bin_name(path.to_owned());
    println!("\n{}", command.render_long_help());
    for child in command.get_subcommands() {
        inventory(child.clone(), &format!("{path} {}", child.get_name()));
    }
}
async fn help(ctx: TaskContext, args: Help) -> TaskResult {
    let guide = ctx.require::<Guide>()?;
    if args.llm {
        println!("Create a web application: ewe_ui new --name FieldNotes --target web\nCreate Android: ewe_ui new --name FieldNotes --target android --app-id com.example.fieldnotes\nCreate iOS: ewe_ui new --name FieldNotes --target ios --app-id com.example.fieldnotes\nCombine all hosts: ewe_ui new --name FieldNotes --target web --target ios --target android --app-id com.example.fieldnotes\nGeneration writes one new directory. Existing destinations produce an error. Escape cancels with status 130. Correct invalid arguments before retrying. Publication requires a release tag, an integrity digest, and NPM_TOKEN.");
        inventory(guide.0.clone(), "ewe_ui");
    } else {
        println!("{}", guide.0.clone().render_long_help());
    }
    Ok(())
}

#[task]
async fn main(mut runner: TaskRunner) -> TaskResult {
    runner = runner.with_name("ewe_ui", ABOUT);
    let mut set = TaskSet::new("ewe_ui").with_namespace("app");
    set.add(
        "new",
        "Create an application with a wizard or complete arguments",
        new,
    )?;
    set.expose("new", &["new"])?;
    set.add(
        "help",
        "Read the application guide or print every command",
        help,
    )?;

    operations::register(&mut set)?;
    runner.context().store(operations::Directory(
        std::env::current_dir().map_err(TaskError::operation)?,
    ));
    runner
        .context()
        .store(foundation_ui_native::tasks::NativeBuildEnv::from_workspace());
    runner.register(operations::native()?)?;
    runner.register(set)?;
    runner.use_help_task("app:help")?;
    foundation_wasm_ui::tasks::install_host(&runner);
    runner.register(foundation_wasm_ui::tasks::set()?)?;
    runner.context().store(Guide(runner.root_command()));
    runner.run().await
}
