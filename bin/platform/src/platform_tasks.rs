use clap::{ArgMatches, Command};
use foundation_tasks::{TaskContext, TaskError, TaskSet};

use crate::BoxedError;

type DirectHandler = fn(TaskContext, &ArgMatches) -> Result<(), BoxedError>;
type GroupHandler = fn(&str, &ArgMatches) -> Result<(), BoxedError>;

pub fn set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("ewe_platform");
    let commands: [(&str, &str, fn(Command) -> Command, DirectHandler); 4] = [
        (
            "generate",
            "generate",
            crate::generate::register,
            |_, args| crate::generate::run(args),
        ),
        ("local", "local", crate::local::register, |ctx, args| {
            crate::local::run(args, ctx.context())
        }),
        (
            "tcp-capture",
            "tcp_capture",
            crate::tcp_capture::register,
            |_, args| crate::tcp_capture::run(args),
        ),
        (
            "gen-model-descriptors",
            "gen_model_descriptors",
            crate::models::register,
            |ctx, args| crate::models::run(args, ctx.context()),
        ),
    ];
    for (name, path, command, handler) in commands {
        let root = command(Command::new("platform"));
        let definition = root
            .find_subcommand(path)
            .expect("platform command registered")
            .clone();
        add(&mut set, name, &[path], definition, handler)?;
    }
    Ok(set)
}

pub fn codegen() -> Result<TaskSet, TaskError> {
    use foundation_codegentools::cli::{gen_api, schema};

    let mut set = group(
        "foundation_codegentools",
        "codegen",
        gen_api::command(),
        "gen_api",
        &[
            ("generate", "generate"),
            ("analyze", "analyze"),
            ("normalize", "normalize"),
        ],
        gen_api::run_subcommand,
    )?;
    add(
        &mut set,
        "schema",
        &["schema"],
        schema::command(),
        |_, args| schema::run(args),
    )?;
    Ok(set)
}

pub fn wasm_bins() -> Result<TaskSet, TaskError> {
    use foundation_wasm::cli;
    group(
        "foundation_wasm",
        "wasm-bins",
        cli::command(),
        "wasm_bins",
        &[("list", "list"), ("generate", "generate")],
        cli::run_subcommand,
    )
}

pub fn wasm_bundle() -> Result<TaskSet, TaskError> {
    use foundation_wasm_ui::cli;
    group(
        "foundation_wasm_ui",
        "wasm-bundle",
        cli::command(),
        "wasm-bundle",
        &[("build", "build"), ("plan", "plan")],
        cli::run_subcommand,
    )
}

fn group(
    owner: &'static str,
    namespace: &'static str,
    command: Command,
    path: &'static str,
    leaves: &[(&'static str, &'static str)],
    handler: GroupHandler,
) -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new(owner).with_namespace(namespace);
    set.describe_group(path, summary(&command));
    for &(name, leaf) in leaves {
        let definition = command
            .find_subcommand(leaf)
            .expect("package command registered")
            .clone();
        add(&mut set, name, &[path, leaf], definition, move |_, args| {
            handler(leaf, args)
        })?;
    }
    Ok(set)
}

fn summary(command: &Command) -> String {
    match command.get_about() {
        Some(text) => text.to_string(),
        None => "Run a platform command".to_owned(),
    }
}

fn add(
    set: &mut TaskSet,
    name: &'static str,
    direct: &[&'static str],
    command: Command,
    handler: impl Fn(TaskContext, &ArgMatches) -> Result<(), BoxedError> + Send + Sync + 'static,
) -> Result<(), TaskError> {
    let handler = std::sync::Arc::new(handler);
    set.add_command(name, summary(&command), command, move |ctx, args| {
        let handler = handler.clone();
        async move {
            blocking::unblock(move || handler(ctx, &args))
                .await
                .map_err(TaskError::operation)
        }
    })?;
    set.expose(name, direct)
}
