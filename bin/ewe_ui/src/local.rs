//! Register a project's declared Rust task package.
use clap::{Arg, ArgAction, ArgMatches, Command};
use foundation_tasks::{ProcessCommand, TaskError, TaskSet};
use foundation_ui_project::application::{LocalArgument, LocalArgumentKind, LocalTasks};
use std::{collections::BTreeSet, ffi::OsString, path::Path};

fn name_ok(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn argument(argument: &LocalArgument) -> Result<Arg, TaskError> {
    if !name_ok(&argument.name) || argument.name == "help" || argument.description.trim().is_empty()
    {
        return Err(TaskError::operation(
            "A local argument needs a distinct lowercase name and a description",
        ));
    }
    let mut arg = Arg::new(argument.name.clone())
        .help(argument.description.clone())
        .required(argument.required);
    if argument.kind != LocalArgumentKind::Positional {
        arg = arg.long(argument.name.clone());
    }
    match argument.kind {
        LocalArgumentKind::Flag if argument.repeat || argument.required => Err(
            TaskError::operation("Local flags must be optional and accept no value"),
        ),
        LocalArgumentKind::Flag => Ok(arg.action(ArgAction::SetTrue)),
        _ => {
            let arg = arg.value_parser(clap::value_parser!(OsString));
            Ok(if argument.repeat {
                if argument.kind == LocalArgumentKind::Positional {
                    arg.num_args(1..)
                } else {
                    arg.action(ArgAction::Append)
                }
            } else {
                arg
            })
        }
    }
}

fn forward(arguments: &[LocalArgument], matches: &ArgMatches) -> Vec<OsString> {
    let mut out = Vec::new();
    for arg in arguments {
        if arg.kind == LocalArgumentKind::Flag {
            if matches.get_flag(&arg.name) {
                out.push(format!("--{}", arg.name).into());
            }
        } else if let Some(values) = matches.get_many::<OsString>(&arg.name) {
            for value in values {
                if arg.kind == LocalArgumentKind::Option {
                    out.push(format!("--{}", arg.name).into());
                }
                out.push(value.clone());
            }
        }
    }
    out
}

pub fn set(root: &Path, local: LocalTasks) -> Result<TaskSet, TaskError> {
    if !name_ok(&local.namespace) || local.commands.is_empty() {
        return Err(TaskError::operation(
            "A local task package needs its own namespace and commands",
        ));
    }
    let root = root.canonicalize().map_err(TaskError::operation)?;
    let manifest = root
        .join(local.manifest)
        .canonicalize()
        .map_err(TaskError::operation)?;
    if !manifest.starts_with(&root)
        || manifest.file_name().and_then(|s| s.to_str()) != Some("Cargo.toml")
    {
        return Err(TaskError::operation(
            "The task manifest must be a Cargo.toml inside the application",
        ));
    }
    let mut set = TaskSet::new("application task package").with_namespace(&local.namespace);
    for definition in local.commands {
        if !name_ok(&definition.name) || definition.description.trim().is_empty() {
            return Err(TaskError::operation(
                "A local command needs a lowercase name and a description",
            ));
        }
        let mut command =
            Command::new(definition.name.clone()).about(definition.description.clone());
        let mut names = BTreeSet::new();
        let mut optional_positional = false;
        let mut repeated_positional = false;
        for arg in &definition.arguments {
            if !names.insert(&arg.name) {
                return Err(TaskError::operation(format!(
                    "Duplicate local argument {}",
                    arg.name
                )));
            }
            if arg.kind == LocalArgumentKind::Positional {
                if repeated_positional {
                    return Err(TaskError::operation(
                        "Only the final positional argument can repeat",
                    ));
                }
                repeated_positional = arg.repeat;
                if optional_positional && arg.required {
                    return Err(TaskError::operation(
                        "Required positional arguments must precede optional arguments",
                    ));
                }
                if !arg.required {
                    optional_positional = true;
                }
            }
            command = command.arg(argument(arg)?);
        }
        let base = ProcessCommand::new("cargo")
            .current_dir(&root)
            .args(["run", "--quiet", "--manifest-path"])
            .arg(&manifest)
            .args(["--", "task", "run"])
            .arg(format!("{}:{}", local.namespace, definition.name));
        set.add_command(
            &definition.name,
            definition.description,
            command,
            move |ctx, matches| {
                let invocation = base.clone().args(forward(&definition.arguments, &matches));
                async move { ctx.exec(invocation).await }
            },
        )?;
    }
    Ok(set)
}
